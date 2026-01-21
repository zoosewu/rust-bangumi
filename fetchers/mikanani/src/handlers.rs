use axum::Json;

pub async fn fetch() -> Json<serde_json::Value> {
    Json(serde_json::json!({"animes": []}))
}

pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}
