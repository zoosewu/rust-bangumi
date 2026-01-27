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

#[derive(Debug, Deserialize)]
pub struct CanHandleRequest {
    pub source_url: String,
    pub source_type: String,
}

#[derive(Debug, Serialize)]
pub struct CanHandleResponse {
    pub fetcher_id: i32,
    pub can_handle: bool,
    pub priority: i32,
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

pub async fn can_handle_subscription(
    Json(payload): Json<CanHandleRequest>,
) -> (StatusCode, Json<CanHandleResponse>) {
    tracing::info!(
        "Checking if can handle subscription: url={}, type={}",
        payload.source_url,
        payload.source_type
    );

    // Mikanani Fetcher handles RSS feeds from mikanani.me domain
    let can_handle = payload.source_type == "rss" && payload.source_url.contains("mikanani.me");

    let response = CanHandleResponse {
        fetcher_id: 1, // Mikanani is typically ID 1
        can_handle,
        priority: 100, // High priority for Mikanani RSS
    };

    let status = if can_handle {
        StatusCode::OK
    } else {
        StatusCode::NO_CONTENT
    };

    tracing::info!(
        "can_handle_subscription result: can_handle={}, priority={}",
        can_handle,
        response.priority
    );

    (status, Json(response))
}
