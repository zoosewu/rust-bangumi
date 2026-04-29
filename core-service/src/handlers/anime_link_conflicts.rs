use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::collections::HashSet;
use diesel::prelude::*;
use serde_json::json;

use crate::dto::{
    AnimeLinkConflictInfo, AnimeLinkConflictLink, DownloadInfo,
    ResolveAnimeLinkConflictRequest, ResolveByRawItemRequest, ResolveByRawItemResponse,
};
use crate::models::{AnimeLink, Download};
use crate::schema::{anime_links, animes, anime_works, downloads, raw_anime_items, subtitle_groups};
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
        // Get anime title via anime -> anime_work
        let anime_title = animes::table
            .inner_join(anime_works::table)
            .filter(animes::anime_id.eq(conflict.anime_id))
            .select(anime_works::title)
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
            .filter(anime_links::anime_id.eq(conflict.anime_id))
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

                let raw_item_title: Option<String> = link.raw_item_id.and_then(|rid| {
                    raw_anime_items::table
                        .filter(raw_anime_items::item_id.eq(rid))
                        .select(raw_anime_items::title)
                        .first::<String>(&mut conn)
                        .ok()
                });

                let sibling_episodes: Vec<i32> = link
                    .raw_item_id
                    .map(|rid| {
                        anime_links::table
                            .filter(anime_links::raw_item_id.eq(rid))
                            .filter(anime_links::link_id.ne(link.link_id))
                            .select(anime_links::episode_no)
                            .load::<i32>(&mut conn)
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();

                AnimeLinkConflictLink {
                    link_id: link.link_id,
                    title: link.title.clone(),
                    url: link.url.clone(),
                    source_hash: link.source_hash.clone(),
                    conflict_flag: link.conflict_flag,
                    link_status: link.link_status.clone(),
                    download: download_info,
                    created_at: link.created_at,
                    raw_item_id: link.raw_item_id,
                    raw_item_title,
                    sibling_episodes,
                }
            })
            .collect();

        result.push(AnimeLinkConflictInfo {
            conflict_id: conflict.conflict_id,
            series_id: conflict.anime_id,
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
        .filter(anime_links::anime_id.eq(conflict.anime_id))
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

/// POST /link-conflicts/resolve-by-raw-item
///
/// 對 (anime_id, group_id) 下所有未解決的衝突，自動以指定 raw_item 為偏好來源。
/// 每個衝突若有候選 link 來自該 raw_item，便採用之；否則該衝突保持未解決。
/// 被選中的 link 會被派送下載；未選中的 link 對應的下載會被取消。
pub async fn resolve_link_conflicts_by_raw_item(
    State(state): State<AppState>,
    Json(payload): Json<ResolveByRawItemRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = match state
        .conflict_detection
        .resolve_conflicts_by_raw_item(
            payload.anime_id,
            payload.group_id,
            payload.chosen_raw_item_id,
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("resolve_conflicts_by_raw_item failed: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "resolve_failed", "message": e })),
            );
        }
    };
    let (resolved_conflicts, skipped_conflicts, chosen_links, unchosen_links) = result;

    // Cancel downloads for unchosen links
    if !unchosen_links.is_empty() {
        if let Err(e) = state
            .cancel_service
            .cancel_downloads_for_links(&unchosen_links)
            .await
        {
            tracing::warn!("Failed to cancel unchosen downloads: {}", e);
        }
    }

    // Dispatch chosen links (dedup)
    let dispatched_link_ids: Vec<i32> = chosen_links
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    if !dispatched_link_ids.is_empty() {
        match state
            .dispatch_service
            .dispatch_new_links(dispatched_link_ids.clone())
            .await
        {
            Ok(r) => tracing::info!(
                "resolve-by-raw-item dispatched {} links: dispatched={}, no_downloader={}, failed={}",
                dispatched_link_ids.len(),
                r.dispatched,
                r.no_downloader,
                r.failed
            ),
            Err(e) => tracing::warn!("Failed to dispatch chosen links: {}", e),
        }
    }

    let response = ResolveByRawItemResponse {
        resolved_conflicts,
        skipped_conflicts,
        dispatched_link_ids,
    };

    (StatusCode::OK, Json(serde_json::to_value(&response).unwrap_or(json!({}))))
}
