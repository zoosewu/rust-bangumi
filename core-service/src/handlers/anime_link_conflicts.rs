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
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };

    // Step 1: Get the conflict to find all links in this group
    let conflict = match state.repos.anime_link_conflict.find_by_id(conflict_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "conflict_not_found" })),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };

    // Step 2: Get all active links in the conflict group to find unchosen link_ids
    let all_links: Vec<AnimeLink> = match anime_links::table
        .filter(anime_links::series_id.eq(conflict.series_id))
        .filter(anime_links::group_id.eq(conflict.group_id))
        .filter(anime_links::episode_no.eq(conflict.episode_no))
        .filter(anime_links::link_status.eq("active"))
        .load::<AnimeLink>(&mut conn)
    {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };

    let unchosen_ids: Vec<i32> = all_links
        .iter()
        .filter(|l| l.link_id != payload.chosen_link_id)
        .map(|l| l.link_id)
        .collect();

    // Step 3: Resolve via ConflictDetectionService (sets chosen link conflict_flag=false, others to 'resolved' status)
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
        }
        Err(e) => {
            tracing::error!("Failed to resolve link conflict {}: {}", conflict_id, e);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "resolve_failed", "message": e })),
            );
        }
    }

    // Step 4: Cancel downloads for unchosen links
    if !unchosen_ids.is_empty() {
        if let Err(e) = state.cancel_service.cancel_downloads_for_links(&unchosen_ids).await {
            tracing::warn!("Failed to cancel unchosen downloads: {}", e);
        }
    }

    // Step 5: Dispatch chosen link if it passes filter (filtered_flag=false, conflict_flag=false)
    // dispatch_new_links already checks both flags, so just pass the chosen link ID
    let dispatch_result = state
        .dispatch_service
        .dispatch_new_links(vec![payload.chosen_link_id])
        .await;

    match &dispatch_result {
        Ok(r) => tracing::info!(
            "Dispatched chosen link {}: dispatched={}, no_downloader={}, failed={}",
            payload.chosen_link_id, r.dispatched, r.no_downloader, r.failed
        ),
        Err(e) => tracing::warn!("Failed to dispatch chosen link: {}", e),
    }

    (
        StatusCode::OK,
        Json(json!({
            "message": "Conflict resolved successfully",
            "conflict_id": conflict_id,
            "chosen_link_id": payload.chosen_link_id
        })),
    )
}
