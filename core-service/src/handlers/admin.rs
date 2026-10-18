//! 維運用的一次性操作（回填等），預設 dry-run。

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::services::identity::HttpMetadataLookup;
use crate::services::identity_backfill::{run_backfill, HttpIdentitySource};
use crate::services::library_maintenance;
use crate::state::AppState;

#[derive(Debug, Default, Deserialize)]
pub struct BackfillRequest {
    /// false（預設）：只產出報表，資料庫不變
    #[serde(default)]
    pub apply: bool,
}

/// POST /admin/identity-backfill — 依 fetcher 回報的身分回填既有作品與季
pub async fn identity_backfill(
    State(state): State<AppState>,
    Json(req): Json<BackfillRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    tracing::info!("Identity backfill requested (apply={})", req.apply);
    let source = HttpIdentitySource::new(state.db.clone());
    let lookup = HttpMetadataLookup::new(state.db.clone());
    match run_backfill(&state.db, &source, &lookup, req.apply).await {
        Ok(report) => {
            tracing::info!(
                "Identity backfill {}: {} subscriptions, {} duplicate downloads merged, {} works soft-deleted",
                if report.applied { "applied" } else { "dry-run" },
                report.subscriptions.len(),
                report.merged_duplicates.len(),
                report.soft_deleted_works.len()
            );
            (StatusCode::OK, Json(serde_json::json!(report)))
        }
        Err(e) => {
            tracing::error!("Identity backfill failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "backfill_failed", "message": e })),
            )
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ApplyRequest {
    #[serde(default)]
    pub apply: bool,
}

/// POST /admin/resync-library — 依目前的系列名稱重新同步所有已同步的集數（搬到新資料夾並產出 NFO / 海報）。
/// dry-run 回傳清單；套用時在背景一集一集進行，進度用 GET 查詢。
pub async fn resync_library(
    State(state): State<AppState>,
    Json(req): Json<ApplyRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let plan = state
        .db
        .get()
        .map_err(|e| e.to_string())
        .and_then(|mut conn| {
            library_maintenance::resync_plan(&mut conn).map_err(|e| e.to_string())
        });
    let (items, shared) = match plan {
        Ok(plan) => plan,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "database_error", "message": e })),
            )
        }
    };
    if !req.apply {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "applied": false,
                "total": items.len(),
                "items": items,
                "shared_with": shared,
            })),
        );
    }
    let total = items.len();
    if !library_maintenance::start_resync(state.sync_service.clone(), state.db.clone(), items) {
        return (
            StatusCode::CONFLICT,
            Json(
                serde_json::json!({ "error": "conflict", "message": "A library resync is already running" }),
            ),
        );
    }
    tracing::info!("Library resync started for {} episodes", total);
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "applied": true, "total": total, "shared_with": shared })),
    )
}

/// GET /admin/resync-library — 重新同步的進度
pub async fn resync_library_progress() -> Json<library_maintenance::ResyncProgress> {
    Json(library_maintenance::resync_progress())
}

/// POST /admin/merged-duplicates — 刪除回填合併掉、仍留在媒體庫的重複檔案（dry-run 只列清單）
pub async fn merged_duplicates(
    State(state): State<AppState>,
    Json(req): Json<ApplyRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let files = match state
        .db
        .get()
        .map_err(|e| e.to_string())
        .and_then(|mut conn| {
            library_maintenance::merged_duplicate_files(&mut conn).map_err(|e| e.to_string())
        }) {
        Ok(files) => files,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "database_error", "message": e })),
            )
        }
    };
    if !req.apply {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "applied": false, "files": files })),
        );
    }
    match library_maintenance::delete_duplicate_files(&state.sync_service, &state.db, &files).await
    {
        Ok(result) => {
            let ok = result.deleted.iter().filter(|r| r.success).count();
            tracing::info!("Deleted {} of {} merged duplicate files", ok, files.len());
            (
                StatusCode::OK,
                Json(
                    serde_json::json!({ "applied": true, "files": files, "deleted": result.deleted }),
                ),
            )
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": "viewer_error", "message": e })),
        ),
    }
}
