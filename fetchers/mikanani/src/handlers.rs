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

    #[tokio::test]
    async fn test_health_check_returns_ok() {
        let (status, body) = health_check().await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.0["status"], "ok");
    }

    #[tokio::test]
    async fn test_can_handle_mikanani_rss() {
        let payload = Json(CanHandleRequest {
            source_url: "https://mikanani.me/rss/bangumi".to_string(),
            source_type: "rss".to_string(),
        });

        let (status, response) = can_handle_subscription(payload).await;

        assert_eq!(status, StatusCode::OK);
        assert!(response.can_handle);
    }

    #[tokio::test]
    async fn test_cannot_handle_other_rss() {
        let payload = Json(CanHandleRequest {
            source_url: "https://example.com/rss".to_string(),
            source_type: "rss".to_string(),
        });

        let (status, response) = can_handle_subscription(payload).await;

        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(!response.can_handle);
    }

    #[tokio::test]
    async fn test_cannot_handle_non_rss_type() {
        let payload = Json(CanHandleRequest {
            source_url: "https://mikanani.me/api".to_string(),
            source_type: "api".to_string(),
        });

        let (status, response) = can_handle_subscription(payload).await;

        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(!response.can_handle);
    }

    #[tokio::test]
    async fn test_fetch_returns_202_accepted() {
        let state = AppState::new();
        let payload = Json(FetchTriggerRequest {
            subscription_id: 123,
            rss_url: "https://mikanani.me/rss/test".to_string(),
            callback_url: "http://core/callback".to_string(),
        });

        let (status, response) = fetch(State(state), payload).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert!(response.accepted);
        assert!(response.message.contains("123"));
    }
}
