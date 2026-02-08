use crate::bangumi_client::BangumiClient;
use crate::db::DbPool;
use crate::file_organizer::FileOrganizer;
use crate::models::*;
use crate::nfo_generator;
use crate::schema::{bangumi_episodes, bangumi_mapping, bangumi_subjects, sync_tasks};
use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use chrono::NaiveDate;
use diesel::prelude::*;
use serde::Serialize;
use shared::ViewerSyncRequest;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

pub async fn health_check() -> (StatusCode, Json<HealthResponse>) {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "healthy".to_string(),
            service: "jellyfin-viewer".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }),
    )
}

pub async fn sync(State(state): State<AppState>, Json(req): Json<ViewerSyncRequest>) -> StatusCode {
    tracing::info!(
        "Received sync request: download_id={}, anime={} S{:02}E{:02}",
        req.download_id,
        req.anime_title,
        req.series_no,
        req.episode_no
    );

    // Record sync task and get task_id
    let task_id = if let Ok(mut conn) = state.db.get() {
        let new_task = NewSyncTask {
            download_id: req.download_id,
            core_series_id: req.series_id,
            episode_no: req.episode_no,
            source_path: req.file_path.clone(),
            status: "processing".to_string(),
        };
        diesel::insert_into(sync_tasks::table)
            .values(&new_task)
            .returning(sync_tasks::task_id)
            .get_result::<i32>(&mut conn)
            .ok()
    } else {
        None
    };

    // Spawn async processing
    let organizer = state.organizer.clone();
    let db = state.db.clone();
    let bangumi = state.bangumi.clone();
    tokio::spawn(async move {
        process_sync(organizer, db, bangumi, req, task_id).await;
    });

    StatusCode::ACCEPTED
}

async fn process_sync(
    organizer: Arc<FileOrganizer>,
    db: DbPool,
    bangumi: Arc<BangumiClient>,
    req: ViewerSyncRequest,
    task_id: Option<i32>,
) {
    let result = do_sync(&organizer, &db, &bangumi, &req).await;

    // Update sync_task record by task_id
    if let (Some(tid), Ok(mut conn)) = (task_id, db.get()) {
        let now = chrono::Utc::now().naive_utc();
        match &result {
            Ok(target_path) => {
                let _ = diesel::update(sync_tasks::table.filter(sync_tasks::task_id.eq(tid)))
                    .set((
                        sync_tasks::status.eq("completed"),
                        sync_tasks::target_path.eq(target_path),
                        sync_tasks::completed_at.eq(Some(now)),
                    ))
                    .execute(&mut conn);
            }
            Err(e) => {
                let _ = diesel::update(sync_tasks::table.filter(sync_tasks::task_id.eq(tid)))
                    .set((
                        sync_tasks::status.eq("failed"),
                        sync_tasks::error_message.eq(Some(e.to_string())),
                        sync_tasks::completed_at.eq(Some(now)),
                    ))
                    .execute(&mut conn);
            }
        }
    }

    // Callback to Core
    let (status, target_path, error_message) = match result {
        Ok(path) => ("synced".to_string(), Some(path), None),
        Err(e) => ("failed".to_string(), None, Some(e.to_string())),
    };

    let callback = shared::ViewerSyncCallback {
        download_id: req.download_id,
        status,
        target_path,
        error_message,
    };

    let client = reqwest::Client::new();
    if let Err(e) = client
        .post(&req.callback_url)
        .json(&callback)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        tracing::error!(
            "Failed to callback Core for download {}: {}",
            req.download_id,
            e
        );
    }
}

async fn do_sync(
    organizer: &FileOrganizer,
    db: &DbPool,
    bangumi: &BangumiClient,
    req: &ViewerSyncRequest,
) -> anyhow::Result<String> {
    // 1. Move the file
    let source = std::path::Path::new(&req.file_path);
    let target_path = organizer
        .organize_episode(
            &req.anime_title,
            req.series_no as u32,
            req.episode_no as u32,
            source,
        )
        .await?;

    // 2. Fetch bangumi metadata (best-effort)
    if let Err(e) = fetch_and_generate_metadata(
        db,
        bangumi,
        organizer,
        req.series_id,
        &req.anime_title,
        req.series_no,
        req.episode_no,
        &target_path,
    )
    .await
    {
        tracing::warn!(
            "Metadata fetch failed for download {} (non-fatal): {}",
            req.download_id,
            e
        );
        // Non-fatal: file is already moved, that's the success criteria
    }

    Ok(target_path.display().to_string())
}

async fn fetch_and_generate_metadata(
    db: &DbPool,
    bangumi: &BangumiClient,
    organizer: &FileOrganizer,
    series_id: i32,
    anime_title: &str,
    series_no: i32,
    episode_no: i32,
    target_path: &std::path::Path,
) -> anyhow::Result<()> {
    let mut conn = db.get().map_err(|e| anyhow::anyhow!("{}", e))?;

    // Check if we already have a mapping
    let mapping = bangumi_mapping::table
        .filter(bangumi_mapping::core_series_id.eq(series_id))
        .first::<BangumiMapping>(&mut conn)
        .optional()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let bangumi_id = if let Some(m) = mapping {
        m.bangumi_id
    } else {
        // Search bangumi.tv
        let found_id = bangumi.search_anime(anime_title).await?;
        match found_id {
            Some(id) => {
                // Fetch and cache subject
                let subject = bangumi.get_subject(id).await?;
                cache_subject(&mut conn, &subject)?;

                // Fetch and cache episodes
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let episodes = bangumi.get_episodes(id).await?;
                cache_episodes(&mut conn, id, &episodes)?;

                // Create mapping
                let new_mapping = NewBangumiMapping {
                    core_series_id: series_id,
                    bangumi_id: id,
                    title_cache: Some(anime_title.to_string()),
                    source: "auto_search".to_string(),
                };
                diesel::insert_into(bangumi_mapping::table)
                    .values(&new_mapping)
                    .execute(&mut conn)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                id
            }
            None => {
                tracing::warn!("No bangumi.tv match found for '{}'", anime_title);
                return Ok(()); // Non-fatal
            }
        }
    };

    // Load cached subject
    let subject = bangumi_subjects::table
        .filter(bangumi_subjects::bangumi_id.eq(bangumi_id))
        .first::<BangumiSubject>(&mut conn)
        .optional()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if let Some(subject) = subject {
        // Generate tvshow.nfo + poster.jpg in anime root directory
        let anime_dir = organizer
            .get_library_dir()
            .join(FileOrganizer::sanitize_filename(anime_title));

        let subject_detail = to_subject_detail(&subject);
        nfo_generator::generate_tvshow_nfo(&anime_dir, &subject_detail).await?;

        // Download poster if not exists
        if let Some(cover_url) = &subject.cover_url {
            let poster_path = anime_dir.join("poster.jpg");
            if !poster_path.exists() {
                if let Err(e) = bangumi.download_image(cover_url, &poster_path).await {
                    tracing::warn!("Failed to download poster: {}", e);
                }
            }
        }
    }

    // Generate episode NFO
    let episode = bangumi_episodes::table
        .filter(bangumi_episodes::bangumi_id.eq(bangumi_id))
        .filter(bangumi_episodes::episode_no.eq(episode_no))
        .first::<BangumiEpisode>(&mut conn)
        .optional()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if let Some(ep) = episode {
        let ep_item = to_episode_item(&ep);
        nfo_generator::generate_episode_nfo(target_path, &ep_item, series_no).await?;
    }

    Ok(())
}

fn cache_subject(
    conn: &mut diesel::PgConnection,
    subject: &crate::bangumi_client::SubjectDetail,
) -> anyhow::Result<()> {
    let air_date = subject
        .date
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

    let new_subject = NewBangumiSubject {
        bangumi_id: subject.id,
        title: subject.name.clone(),
        title_cn: subject.name_cn.clone(),
        summary: subject.summary.clone(),
        rating: subject.rating.as_ref().and_then(|r| r.score),
        cover_url: subject.images.as_ref().and_then(|i| i.large.clone()),
        air_date,
        episode_count: subject.total_episodes,
        raw_json: None,
    };

    diesel::insert_into(bangumi_subjects::table)
        .values(&new_subject)
        .on_conflict(bangumi_subjects::bangumi_id)
        .do_nothing()
        .execute(conn)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

fn cache_episodes(
    conn: &mut diesel::PgConnection,
    bangumi_id: i32,
    episodes: &[crate::bangumi_client::EpisodeItem],
) -> anyhow::Result<()> {
    for ep in episodes {
        let air_date = ep
            .airdate
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let new_ep = NewBangumiEpisode {
            bangumi_ep_id: ep.id,
            bangumi_id,
            episode_no: ep.ep.unwrap_or(ep.sort),
            title: ep.name.clone(),
            title_cn: ep.name_cn.clone(),
            air_date,
            summary: ep.desc.clone(),
        };

        diesel::insert_into(bangumi_episodes::table)
            .values(&new_ep)
            .on_conflict(bangumi_episodes::bangumi_ep_id)
            .do_nothing()
            .execute(conn)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    Ok(())
}

/// Convert DB model to bangumi_client type for NFO generation
fn to_subject_detail(s: &BangumiSubject) -> crate::bangumi_client::SubjectDetail {
    crate::bangumi_client::SubjectDetail {
        id: s.bangumi_id,
        name: s.title.clone(),
        name_cn: s.title_cn.clone(),
        summary: s.summary.clone(),
        date: s.air_date.map(|d| d.format("%Y-%m-%d").to_string()),
        images: s
            .cover_url
            .as_ref()
            .map(|url| crate::bangumi_client::SubjectImages {
                large: Some(url.clone()),
                common: None,
                medium: None,
                small: None,
            }),
        rating: s.rating.map(|score| crate::bangumi_client::SubjectRating {
            score: Some(score),
            total: None,
        }),
        total_episodes: s.episode_count,
    }
}

fn to_episode_item(ep: &BangumiEpisode) -> crate::bangumi_client::EpisodeItem {
    crate::bangumi_client::EpisodeItem {
        id: ep.bangumi_ep_id,
        ep: Some(ep.episode_no),
        sort: ep.episode_no,
        name: ep.title.clone(),
        name_cn: ep.title_cn.clone(),
        airdate: ep.air_date.map(|d| d.format("%Y-%m-%d").to_string()),
        desc: ep.summary.clone(),
    }
}
