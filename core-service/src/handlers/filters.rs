use axum::Json;

// 過濾規則處理 - 將在後續實現
pub async fn create_filter() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

pub async fn list_filters() -> Json<serde_json::Value> {
    Json(serde_json::json!({"filters": []}))
}

pub async fn delete_filter() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}
