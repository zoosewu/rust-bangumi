use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::json;

use crate::dto::{AnimeLinkRequest, AnimeLinkResponse, AnimeLinkRichResponse, ConflictingLinkResponse, DownloadInfo};
use crate::models::{AnimeLink, Anime, AnimeWork, Download, NewAnimeLink, SubtitleGroup};
use crate::schema::{anime_links, anime_works, animes, downloads, raw_anime_items, subscriptions, subtitle_groups};
use crate::state::AppState;

/// Create a new anime link
#[utoipa::path(
    post,
    path = "/api/core/links",
    tag = "Links",
    request_body = AnimeLinkRequest,
    responses(
        (status = 201, description = "Created successfully", body = AnimeLinkResponse),
        (status = 500, description = "Database error")
    )
)]
pub async fn create_anime_link(
    State(state): State<AppState>,
    Json(payload): Json<AnimeLinkRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let now = Utc::now().naive_utc();
    let new_link = NewAnimeLink {
        anime_id: payload.series_id,
        group_id: payload.group_id,
        episode_no: payload.episode_no,
        title: payload.title,
        url: payload.url.clone(),
        source_hash: payload.source_hash,
        filtered_flag: false,
        created_at: now,
        raw_item_id: None,
        download_type: crate::services::download_type_detector::detect_download_type(&payload.url)
            .map(|dt| dt.to_string()),
        conflict_flag: false,
        link_status: "active".to_string(),
    };

    match state.repos.anime_link.create(new_link).await {
        Ok(link) => {
            // Trigger conflict detection
            match state.conflict_detection.detect_and_mark_conflicts().await {
                Ok(result) => {
                    if !result.auto_dispatch_link_ids.is_empty() {
                        if let Err(e) = state.dispatch_service.dispatch_new_links(result.auto_dispatch_link_ids).await {
                            tracing::warn!("Auto-dispatch after conflict detection failed: {}", e);
                        }
                    }
                }
                Err(e) => tracing::warn!("Conflict detection failed: {}", e),
            }

            tracing::info!("Created anime link: {}", link.link_id);
            let response = AnimeLinkResponse {
                link_id: link.link_id,
                series_id: link.anime_id,
                group_id: link.group_id,
                episode_no: link.episode_no,
                title: link.title,
                url: link.url,
                source_hash: link.source_hash,
                created_at: link.created_at,
            };
            (StatusCode::CREATED, Json(json!(response)))
        }
        Err(e) => {
            tracing::error!("Failed to create anime link: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to create anime link: {}", e)
                })),
            )
        }
    }
}

/// Get anime links by series_id — returns ALL links (filtered + unfiltered) with group_name and download status
#[utoipa::path(
    get,
    path = "/api/core/links/{series_id}",
    tag = "Links",
    params(("series_id" = i32, Path, description = "Anime series ID")),
    responses(
        (status = 200, description = "Success", body = Vec<AnimeLinkRichResponse>),
        (status = 500, description = "Database error")
    )
)]
pub async fn get_anime_links(
    State(state): State<AppState>,
    Path(series_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string(), "links": [] })),
            );
        }
    };

    // Load all links with subtitle group name (LEFT JOIN download)
    let links_with_groups: Vec<(AnimeLink, SubtitleGroup)> = match anime_links::table
        .inner_join(subtitle_groups::table.on(subtitle_groups::group_id.eq(anime_links::group_id)))
        .filter(anime_links::anime_id.eq(series_id))
        .select((AnimeLink::as_select(), SubtitleGroup::as_select()))
        .order((anime_links::filtered_flag.asc(), anime_links::episode_no.asc()))
        .load::<(AnimeLink, SubtitleGroup)>(&mut conn)
    {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string(), "links": [] })),
            );
        }
    };

    // Build conflict group map: (group_id, episode_no) -> Vec<link_id> for conflicted links
    let mut conflict_groups: HashMap<(i32, i32), Vec<i32>> = HashMap::new();
    for (link, _) in &links_with_groups {
        if link.conflict_flag {
            conflict_groups
                .entry((link.group_id, link.episode_no))
                .or_default()
                .push(link.link_id);
        }
    }

    let mut results = Vec::new();
    for (link, group) in &links_with_groups {
        let download_info: Option<DownloadInfo> = downloads::table
            .filter(downloads::link_id.eq(link.link_id))
            .order(downloads::updated_at.desc())
            .first::<Download>(&mut conn)
            .optional()
            .ok()
            .flatten()
            .map(|d| DownloadInfo {
                download_id: d.download_id,
                status: d.status,
                progress: d.progress,
                torrent_hash: d.torrent_hash,
            });

        let conflicting_link_ids = if link.conflict_flag {
            conflict_groups
                .get(&(link.group_id, link.episode_no))
                .map(|ids| ids.iter().filter(|&&id| id != link.link_id).cloned().collect())
                .unwrap_or_default()
        } else {
            vec![]
        };

        results.push(AnimeLinkRichResponse {
            link_id: link.link_id,
            series_id: link.anime_id,
            group_id: link.group_id,
            group_name: group.group_name.clone(),
            episode_no: link.episode_no,
            title: link.title.clone(),
            url: link.url.clone(),
            source_hash: link.source_hash.clone(),
            filtered_flag: link.filtered_flag,
            conflict_flag: link.conflict_flag,
            conflicting_link_ids,
            download: download_info,
            created_at: link.created_at,
        });
    }

    tracing::info!(
        "Retrieved {} anime links for series_id={}",
        results.len(),
        series_id
    );
    (StatusCode::OK, Json(json!({ "links": results })))
}

/// List all anime links with conflict_flag = true, enriched with series/anime/subscription info
#[utoipa::path(
    get,
    path = "/api/core/links/conflicts",
    tag = "Links",
    responses(
        (status = 200, description = "Success", body = Vec<ConflictingLinkResponse>),
        (status = 500, description = "Database error")
    )
)]
pub async fn list_conflicting_links(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string(), "conflicts": [] })),
            );
        }
    };

    // 1. Fetch all conflicting links with subtitle group, series, and anime work
    let rows: Vec<(AnimeLink, SubtitleGroup, Anime, AnimeWork)> = match anime_links::table
        .inner_join(subtitle_groups::table.on(subtitle_groups::group_id.eq(anime_links::group_id)))
        .inner_join(animes::table.on(animes::anime_id.eq(anime_links::anime_id)))
        .inner_join(anime_works::table.on(anime_works::work_id.eq(animes::work_id)))
        .filter(anime_links::conflict_flag.eq(true))
        .select((
            AnimeLink::as_select(),
            SubtitleGroup::as_select(),
            Anime::as_select(),
            AnimeWork::as_select(),
        ))
        .load::<(AnimeLink, SubtitleGroup, Anime, AnimeWork)>(&mut conn)
    {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string(), "conflicts": [] })),
            );
        }
    };

    // 2. Batch fetch subscription info for links that have raw_item_id
    let raw_item_ids: Vec<i32> = rows
        .iter()
        .filter_map(|(link, _, _, _)| link.raw_item_id)
        .collect();

    // Map: raw_item_id -> (subscription_id, subscription_name)
    let sub_map: HashMap<i32, (i32, Option<String>)> = if !raw_item_ids.is_empty() {
        match raw_anime_items::table
            .inner_join(
                subscriptions::table
                    .on(subscriptions::subscription_id.eq(raw_anime_items::subscription_id)),
            )
            .filter(raw_anime_items::item_id.eq_any(&raw_item_ids))
            .select((
                raw_anime_items::item_id,
                subscriptions::subscription_id,
                subscriptions::name,
            ))
            .load::<(i32, i32, Option<String>)>(&mut conn)
        {
            Ok(data) => data
                .into_iter()
                .map(|(item_id, sub_id, name)| (item_id, (sub_id, name)))
                .collect(),
            Err(e) => {
                tracing::warn!("Failed to fetch subscription info for conflicting links: {}", e);
                HashMap::new()
            }
        }
    } else {
        HashMap::new()
    };

    // 3. Build conflict group map: (anime_id, group_id, episode_no) -> Vec<link_id>
    let mut conflict_groups: HashMap<(i32, i32, i32), Vec<i32>> = HashMap::new();
    for (link, _, _, _) in &rows {
        conflict_groups
            .entry((link.anime_id, link.group_id, link.episode_no))
            .or_default()
            .push(link.link_id);
    }

    // 4. Build response
    let results: Vec<ConflictingLinkResponse> = rows
        .iter()
        .map(|(link, group, series, work)| {
            let conflicting_link_ids = conflict_groups
                .get(&(link.anime_id, link.group_id, link.episode_no))
                .map(|ids| ids.iter().filter(|&&id| id != link.link_id).cloned().collect())
                .unwrap_or_default();

            let (subscription_id, subscription_name) = link
                .raw_item_id
                .and_then(|id| sub_map.get(&id))
                .map(|(sub_id, name)| (Some(*sub_id), name.clone()))
                .unwrap_or((None, None));

            ConflictingLinkResponse {
                link_id: link.link_id,
                episode_no: link.episode_no,
                group_name: group.group_name.clone(),
                url: link.url.clone(),
                conflicting_link_ids,
                series_id: link.anime_id,
                series_no: series.series_no,
                anime_work_id: work.work_id,
                anime_work_title: work.title.clone(),
                subscription_id,
                subscription_name,
            }
        })
        .collect();

    tracing::info!("Retrieved {} conflicting anime links", results.len());
    (StatusCode::OK, Json(json!({ "conflicts": results })))
}
