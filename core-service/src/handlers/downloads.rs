//! 下載記錄查詢 API

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::dto::{RetryBulkRequest, RetryOneResponse, RetryResultResponse};
use crate::models::Download;
use crate::schema::{anime_links, downloads};
use crate::services::download_dispatch::{RetryResult, RETRYABLE_STATUSES};
use crate::state::AppState;

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListDownloadsQuery {
    /// Filter by download status (pending, downloading, completed, failed)
    pub status: Option<String>,
    /// Maximum number of records to return
    pub limit: Option<i64>,
    /// Number of records to skip
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct DownloadRow {
    pub download_id: i32,
    pub link_id: i32,
    pub title: Option<String>,
    pub downloader_type: String,
    pub status: String,
    pub progress: Option<f32>,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub torrent_hash: Option<String>,
    pub file_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// List download records
#[utoipa::path(
    get,
    path = "/api/core/downloads",
    tag = "Downloads",
    params(ListDownloadsQuery),
    responses(
        (status = 200, description = "Success"),
        (status = 500, description = "Database error")
    )
)]
pub async fn list_downloads(
    State(state): State<AppState>,
    Query(params): Query<ListDownloadsQuery>,
) -> Result<Json<Vec<DownloadRow>>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let mut query = downloads::table
        .order(downloads::updated_at.desc())
        .limit(limit)
        .offset(offset)
        .into_boxed();

    if let Some(ref status) = params.status {
        query = query.filter(downloads::status.eq(status));
    }

    let dl_list: Vec<Download> = query
        .load(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Batch-fetch link titles
    let link_ids: Vec<i32> = dl_list.iter().map(|d| d.link_id).collect();
    let link_titles: Vec<(i32, Option<String>)> = anime_links::table
        .filter(anime_links::link_id.eq_any(&link_ids))
        .select((anime_links::link_id, anime_links::title))
        .load(&mut conn)
        .unwrap_or_default();

    let title_map: std::collections::HashMap<i32, Option<String>> =
        link_titles.into_iter().collect();

    let rows: Vec<DownloadRow> = dl_list
        .into_iter()
        .map(|d| {
            let title = title_map.get(&d.link_id).cloned().flatten();
            DownloadRow {
                download_id: d.download_id,
                link_id: d.link_id,
                title,
                downloader_type: d.downloader_type,
                status: d.status,
                progress: d.progress,
                downloaded_bytes: d.downloaded_bytes,
                total_bytes: d.total_bytes,
                error_message: d.error_message,
                torrent_hash: d.torrent_hash,
                file_path: d.file_path,
                created_at: crate::serde_utils::format_utc(&d.created_at),
                updated_at: crate::serde_utils::format_utc(&d.updated_at),
            }
        })
        .collect();

    Ok(Json(rows))
}

fn into_response(r: RetryResult) -> RetryResultResponse {
    RetryResultResponse {
        downloads_matched: r.downloads_matched,
        not_retryable: r.not_retryable,
        unique_links: r.unique_links,
        dispatched: r.dispatched,
        no_downloader: r.no_downloader,
        conflict_or_filtered: r.conflict_or_filtered,
        failed: r.failed,
    }
}

/// POST /downloads/:download_id/retry — manually retry a single download.
#[utoipa::path(
    post,
    path = "/api/core/downloads/{download_id}/retry",
    tag = "Downloads",
    params(
        ("download_id" = i32, Path, description = "Download ID to retry")
    ),
    responses(
        (status = 200, description = "Success", body = RetryOneResponse),
        (status = 404, description = "Download not found"),
        (status = 409, description = "Download status not retryable"),
        (status = 500, description = "Dispatch failed")
    )
)]
pub async fn retry_one(
    State(state): State<AppState>,
    Path(download_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = match state
        .dispatch_service
        .manual_retry(vec![download_id])
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("retry_one({}) failed: {}", download_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "dispatch_failed", "message": e })),
            );
        }
    };

    if result.downloads_matched == 0 {
        let mut conn = match state.db.get() {
            Ok(c) => c,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "db_error", "message": e.to_string() })),
                );
            }
        };
        let exists = downloads::table
            .filter(downloads::download_id.eq(download_id))
            .select(downloads::download_id)
            .first::<i32>(&mut conn)
            .is_ok();
        if !exists {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "download_not_found", "download_id": download_id })),
            );
        }
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "not_retryable",
                "download_id": download_id,
                "message": "Download status is not in retryable set",
                "retryable_statuses": RETRYABLE_STATUSES,
            })),
        );
    }

    if result.not_retryable == 1 {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "not_retryable",
                "download_id": download_id,
                "retryable_statuses": RETRYABLE_STATUSES,
            })),
        );
    }

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "db_error", "message": e.to_string() })),
            );
        }
    };
    let link_id: i32 = match downloads::table
        .filter(downloads::download_id.eq(download_id))
        .select(downloads::link_id)
        .first::<i32>(&mut conn)
    {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "db_error", "message": e.to_string() })),
            );
        }
    };

    if result.dispatched == 1 {
        let resp = RetryOneResponse {
            download_id,
            link_id,
            status: "dispatched".to_string(),
        };
        return (StatusCode::OK, Json(serde_json::to_value(resp).expect("response serialization should not fail")));
    }

    if result.no_downloader == 1 {
        let resp = RetryOneResponse {
            download_id,
            link_id,
            status: "no_downloader".to_string(),
        };
        return (StatusCode::OK, Json(serde_json::to_value(resp).expect("response serialization should not fail")));
    }

    if result.conflict_or_filtered == 1 {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "link_not_dispatchable",
                "download_id": download_id,
                "link_id": link_id,
                "message": "Link is filtered, in conflict, or otherwise blocked from dispatch",
            })),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "dispatch_failed", "result": into_response(result) })),
    )
}

/// POST /downloads/retry — bulk retry, optional filters in the body.
#[utoipa::path(
    post,
    path = "/api/core/downloads/retry",
    tag = "Downloads",
    request_body = RetryBulkRequest,
    responses(
        (status = 200, description = "Success", body = RetryResultResponse),
        (status = 500, description = "Dispatch failed")
    )
)]
pub async fn retry_bulk(
    State(state): State<AppState>,
    Json(payload): Json<RetryBulkRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "db_error", "message": e.to_string() })),
            );
        }
    };

    let mut q = downloads::table
        .filter(downloads::status.eq_any(RETRYABLE_STATUSES))
        .into_boxed();
    if let Some(ids) = &payload.download_ids {
        q = q.filter(downloads::download_id.eq_any(ids));
    }
    if let Some(s) = &payload.status {
        q = q.filter(downloads::status.eq_any(s));
    }
    if let Some(dt) = &payload.downloader_type {
        q = q.filter(downloads::downloader_type.eq(dt));
    }

    let candidate_ids: Vec<i32> = match q.select(downloads::download_id).load::<i32>(&mut conn) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "db_error", "message": e.to_string() })),
            );
        }
    };

    drop(conn);

    if candidate_ids.is_empty() {
        let empty = RetryResultResponse {
            downloads_matched: 0,
            not_retryable: 0,
            unique_links: 0,
            dispatched: 0,
            no_downloader: 0,
            conflict_or_filtered: 0,
            failed: 0,
        };
        return (StatusCode::OK, Json(serde_json::to_value(empty).expect("response serialization should not fail")));
    }

    let count = candidate_ids.len();
    match state.dispatch_service.manual_retry(candidate_ids).await {
        Ok(r) => {
            tracing::info!("retry_bulk: candidates={}, result={:?}", count, r);
            (
                StatusCode::OK,
                Json(serde_json::to_value(into_response(r)).expect("response serialization should not fail")),
            )
        }
        Err(e) => {
            tracing::error!("retry_bulk failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "dispatch_failed", "message": e })),
            )
        }
    }
}
