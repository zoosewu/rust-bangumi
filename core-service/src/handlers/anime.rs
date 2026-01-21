use axum::Json;

// 動畫管理處理 - 將在後續實現
pub async fn list_anime() -> Json<serde_json::Value> {
    Json(serde_json::json!({"animes": []}))
}

pub async fn get_anime() -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

pub async fn get_links() -> Json<serde_json::Value> {
    Json(serde_json::json!({"links": []}))
}
