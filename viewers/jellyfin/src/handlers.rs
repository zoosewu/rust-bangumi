use axum::Json;

pub async fn sync() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "synced"}))
}

pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}
