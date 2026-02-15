use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use diesel::prelude::*;
use serde_json::json;

use crate::dto::{
    AnimeLinkConflictInfo, AnimeLinkConflictLink, DownloadInfo,
    ResolveAnimeLinkConflictRequest,
};
use crate::models::{AnimeLink, Download};
use crate::schema::{anime_links, anime_series, animes, downloads, subtitle_groups};
use crate::state::AppState;

/// GET /link-conflicts - List all unresolved anime link conflicts
pub async fn list_link_conflicts(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let conflicts = match state.repos.anime_link_conflict.find_unresolved().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to get link conflicts: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string(), "conflicts": [] })),
            );
        }
    };

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string(), "conflicts": [] })),
            );
        }
    };

    let mut result = Vec::new();

    for conflict in &conflicts {
        // Get anime title via series -> anime
        let anime_title = anime_series::table
            .inner_join(animes::table)
            .filter(anime_series::series_id.eq(conflict.series_id))
            .select(animes::title)
            .first::<String>(&mut conn)
            .unwrap_or_else(|_| "Unknown".to_string());

        // Get group name
        let group_name = subtitle_groups::table
            .filter(subtitle_groups::group_id.eq(conflict.group_id))
            .select(subtitle_groups::group_name)
            .first::<String>(&mut conn)
            .unwrap_or_else(|_| "Unknown".to_string());

        // Get all active links for this episode
        let links: Vec<AnimeLink> = anime_links::table
            .filter(anime_links::series_id.eq(conflict.series_id))
            .filter(anime_links::group_id.eq(conflict.group_id))
            .filter(anime_links::episode_no.eq(conflict.episode_no))
            .filter(anime_links::link_status.eq("active"))
            .load::<AnimeLink>(&mut conn)
            .unwrap_or_default();

        let link_infos: Vec<AnimeLinkConflictLink> = links
            .iter()
            .map(|link| {
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

                AnimeLinkConflictLink {
                    link_id: link.link_id,
                    title: link.title.clone(),
                    url: link.url.clone(),
                    source_hash: link.source_hash.clone(),
                    conflict_flag: link.conflict_flag,
                    link_status: link.link_status.clone(),
                    download: download_info,
                    created_at: link.created_at,
                }
            })
            .collect();

        result.push(AnimeLinkConflictInfo {
            conflict_id: conflict.conflict_id,
            series_id: conflict.series_id,
            group_id: conflict.group_id,
            episode_no: conflict.episode_no,
            anime_title,
            group_name,
            resolution_status: conflict.resolution_status.clone(),
            chosen_link_id: conflict.chosen_link_id,
            links: link_infos,
            created_at: conflict.created_at,
        });
    }

    tracing::info!("Retrieved {} unresolved link conflicts", result.len());
    (StatusCode::OK, Json(json!({ "conflicts": result })))
}

/// POST /link-conflicts/:id/resolve - Resolve a link conflict
pub async fn resolve_link_conflict(
    State(state): State<AppState>,
    Path(conflict_id): Path<i32>,
    Json(payload): Json<ResolveAnimeLinkConflictRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state
        .conflict_detection
        .resolve_conflict(conflict_id, payload.chosen_link_id)
        .await
    {
        Ok(()) => {
            tracing::info!(
                "Resolved link conflict {}: chosen link_id={}",
                conflict_id,
                payload.chosen_link_id
            );
            (
                StatusCode::OK,
                Json(json!({
                    "message": "Conflict resolved successfully",
                    "conflict_id": conflict_id,
                    "chosen_link_id": payload.chosen_link_id
                })),
            )
        }
        Err(e) => {
            tracing::error!("Failed to resolve link conflict {}: {}", conflict_id, e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "resolve_failed",
                    "message": e
                })),
            )
        }
    }
}
