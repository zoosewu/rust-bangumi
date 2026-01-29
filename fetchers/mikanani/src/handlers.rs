use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use fetcher_mikanani::RssParser;
use shared::{FetchTriggerRequest, FetchTriggerResponse, FetchedAnime};

#[derive(Debug, Serialize)]
pub struct FetcherResultsPayload {
    pub subscription_id: i32,
    pub animes: Vec<FetchedAnime>,
    pub fetcher_source: String,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CanHandleRequest {
    pub source_url: String,
    pub source_type: String,
}

#[derive(Debug, Serialize)]
pub struct CanHandleResponse {
    pub can_handle: bool,
}

pub async fn fetch(
    State(parser): State<Arc<RssParser>>,
    Json(payload): Json<FetchTriggerRequest>,
) -> (StatusCode, Json<FetchTriggerResponse>) {
    tracing::info!(
        "Received fetch trigger for subscription {}: {}",
        payload.subscription_id,
        payload.rss_url
    );

    // 立即回傳 202 Accepted
    let response = FetchTriggerResponse {
        accepted: true,
        message: format!("Fetch task accepted for subscription {}", payload.subscription_id),
    };

    // 在背景執行抓取任務
    let parser_clone = parser.clone();
    let subscription_id = payload.subscription_id;
    let rss_url = payload.rss_url.clone();
    let callback_url = payload.callback_url.clone();

    tokio::spawn(async move {
        tracing::info!("Starting background fetch for: {}", rss_url);

        let result = parser_clone.parse_feed(&rss_url).await;

        let payload = match result {
            Ok(animes) => {
                let count: usize = animes.iter().map(|a| a.links.len()).sum();
                tracing::info!(
                    "Background fetch successful: {} links from {} anime",
                    count,
                    animes.len()
                );
                FetcherResultsPayload {
                    subscription_id,
                    animes,
                    fetcher_source: "mikanani".to_string(),
                    success: true,
                    error_message: None,
                }
            }
            Err(e) => {
                tracing::error!("Background fetch failed: {}", e);
                FetcherResultsPayload {
                    subscription_id,
                    animes: vec![],
                    fetcher_source: "mikanani".to_string(),
                    success: false,
                    error_message: Some(e.to_string()),
                }
            }
        };

        // 送回結果到 Core Service
        let client = reqwest::Client::new();
        match client.post(&callback_url).json(&payload).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    tracing::info!("Successfully sent results to core service");
                } else {
                    tracing::error!(
                        "Core service returned error: {}",
                        resp.status()
                    );
                }
            }
            Err(e) => {
                tracing::error!("Failed to send results to core service: {}", e);
            }
        }
    });

    (StatusCode::ACCEPTED, Json(response))
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

    let can_handle = payload.source_type == "rss" && payload.source_url.contains("mikanani.me");

    let response = CanHandleResponse { can_handle };

    let status = if can_handle {
        StatusCode::OK
    } else {
        StatusCode::NO_CONTENT
    };

    tracing::info!("can_handle_subscription result: can_handle={}", can_handle);

    (status, Json(response))
}
