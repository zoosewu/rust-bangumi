use axum::Json;
use shared::ServiceType;

// 服務註冊處理 - 將在後續實現
pub async fn register() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

pub async fn list_services() -> Json<serde_json::Value> {
    Json(serde_json::json!({"services": []}))
}

pub async fn list_by_type() -> Json<serde_json::Value> {
    Json(serde_json::json!({"services": []}))
}

pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}
