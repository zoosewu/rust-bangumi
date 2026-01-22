use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use fetcher_mikanani::RssParser;

#[derive(Debug, Deserialize)]
pub struct FetchRequest {
    pub rss_url: String,
}

#[derive(Debug, Serialize)]
pub struct FetchResponse {
    pub status: String,
    pub count: usize,
    pub error: Option<String>,
}

pub async fn fetch(
    State(parser): State<Arc<RssParser>>,
    Json(payload): Json<FetchRequest>,
) -> (StatusCode, Json<FetchResponse>) {
    tracing::info!("Fetching RSS from: {}", payload.rss_url);

    match parser.parse_feed(&payload.rss_url).await {
        Ok(animes) => {
            let count = animes.iter().map(|a| a.links.len()).sum();
            tracing::info!("Successfully fetched {} anime series with {} total links", animes.len(), count);
            (StatusCode::OK, Json(FetchResponse {
                status: "success".to_string(),
                count,
                error: None,
            }))
        }
        Err(e) => {
            tracing::error!("Failed to fetch RSS: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(FetchResponse {
                status: "error".to_string(),
                count: 0,
                error: Some(e.to_string()),
            }))
        }
    }
}

pub async fn health_check() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}
