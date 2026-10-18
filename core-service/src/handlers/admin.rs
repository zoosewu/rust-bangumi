//! 維運用的一次性操作（回填等），預設 dry-run。

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::services::identity::HttpMetadataLookup;
use crate::services::identity_backfill::{run_backfill, HttpIdentitySource};
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
