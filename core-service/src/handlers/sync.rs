use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use shared::ViewerSyncCallback;

use crate::state::AppState;

pub async fn sync_callback(
    State(state): State<AppState>,
    Json(payload): Json<ViewerSyncCallback>,
) -> (StatusCode, Json<serde_json::Value>) {
    tracing::info!(
        "Received sync callback for download {}: status={}",
        payload.download_id,
        payload.status
    );

    match state.db.get() {
        Ok(mut conn) => {
            match state.sync_service.handle_callback(
                &mut conn,
                payload.download_id,
                &payload.status,
                payload.target_path.as_deref(),
                payload.error_message.as_deref(),
            ) {
                Ok(()) => (StatusCode::OK, Json(json!({ "status": "ok" }))),
                Err(e) => {
                    tracing::error!("Failed to handle sync callback: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": e })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database connection error: {}", e) })),
            )
        }
    }
}
