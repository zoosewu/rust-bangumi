use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::json;

use crate::dto::{AnimeLinkRequest, AnimeLinkResponse, AnimeLinkRichResponse, DownloadInfo};
use crate::models::{AnimeLink, Download, NewAnimeLink, SubtitleGroup};
use crate::schema::{anime_links, downloads, subtitle_groups};
use crate::state::AppState;

/// Create a new anime link
pub async fn create_anime_link(
    State(state): State<AppState>,
    Json(payload): Json<AnimeLinkRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let now = Utc::now().naive_utc();
    let new_link = NewAnimeLink {
        series_id: payload.series_id,
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
    };

    match state.repos.anime_link.create(new_link).await {
        Ok(link) => {
            tracing::info!("Created anime link: {}", link.link_id);
            let response = AnimeLinkResponse {
                link_id: link.link_id,
                series_id: link.series_id,
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

/// Get anime links by series_id — returns ALL links (filtered + unfiltered)
/// with group_name and download status
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
        .filter(anime_links::series_id.eq(series_id))
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

    let mut results = Vec::new();
    for (link, group) in &links_with_groups {
        // Get download info for this link (latest)
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

        results.push(AnimeLinkRichResponse {
            link_id: link.link_id,
            series_id: link.series_id,
            group_id: link.group_id,
            group_name: group.group_name.clone(),
            episode_no: link.episode_no,
            title: link.title.clone(),
            url: link.url.clone(),
            source_hash: link.source_hash.clone(),
            filtered_flag: link.filtered_flag,
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
