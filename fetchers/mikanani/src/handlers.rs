use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use fetcher_mikanani::{RssParser, FetchTask, RealHttpClient};
use shared::{FetchTriggerRequest, FetchTriggerResponse};

#[derive(Debug, Deserialize)]
pub struct CanHandleRequest {
    pub source_url: String,
    pub source_type: String,
}

#[derive(Debug, Serialize)]
pub struct CanHandleResponse {
    pub can_handle: bool,
}

/// 應用程式共享狀態
#[derive(Clone)]
pub struct AppState {
    pub parser: Arc<RssParser>,
    pub http_client: Arc<RealHttpClient>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            parser: Arc::new(RssParser::new()),
            http_client: Arc::new(RealHttpClient::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn fetch(
    State(state): State<AppState>,
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
    let parser = state.parser.clone();
    let http_client = state.http_client.clone();
    let subscription_id = payload.subscription_id;
    let rss_url = payload.rss_url.clone();
    let callback_url = payload.callback_url.clone();

    tokio::spawn(async move {
        tracing::info!("Starting background fetch for: {}", rss_url);

        let task = FetchTask::new(parser, http_client, "mikanani".to_string());

        if let Err(e) = task.execute_and_send(subscription_id, &rss_url, &callback_url).await {
            tracing::error!("Background fetch task failed: {}", e);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_mikanani_rss() {
        let request = CanHandleRequest {
            source_url: "https://mikanani.me/rss/bangumi".to_string(),
            source_type: "rss".to_string(),
        };

        let can_handle = request.source_type == "rss" && request.source_url.contains("mikanani.me");
        assert!(can_handle);
    }

    #[test]
    fn test_cannot_handle_other_rss() {
        let request = CanHandleRequest {
            source_url: "https://example.com/rss".to_string(),
            source_type: "rss".to_string(),
        };

        let can_handle = request.source_type == "rss" && request.source_url.contains("mikanani.me");
        assert!(!can_handle);
    }

    #[test]
    fn test_cannot_handle_non_rss_type() {
        let request = CanHandleRequest {
            source_url: "https://mikanani.me/api".to_string(),
            source_type: "api".to_string(),
        };

        let can_handle = request.source_type == "rss" && request.source_url.contains("mikanani.me");
        assert!(!can_handle);
    }

    #[test]
    fn test_app_state_creation() {
        let state = AppState::new();
        // 確保可以建立狀態
        assert!(Arc::strong_count(&state.parser) >= 1);
    }
}
