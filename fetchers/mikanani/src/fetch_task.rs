use std::sync::Arc;
use shared::models::{RawAnimeItem, RawFetcherResultsPayload};

use crate::http_client::{HttpClient, HttpError};
use crate::RssParser;

// Legacy payload for backwards compatibility with old handler
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FetcherResultsPayload {
    pub subscription_id: i32,
    pub animes: Vec<shared::FetchedAnime>,
    pub fetcher_source: String,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Fetch 任務錯誤類型
#[derive(Debug, thiserror::Error)]
pub enum FetchTaskError {
    #[error("RSS parsing failed: {0}")]
    ParseError(String),
    #[error("Callback failed: {0}")]
    CallbackError(#[from] HttpError),
}

/// 可測試的 Fetch 任務邏輯
pub struct FetchTask<C: HttpClient> {
    parser: Arc<RssParser>,
    http_client: Arc<C>,
    fetcher_source: String,
}

impl<C: HttpClient> FetchTask<C> {
    pub fn new(parser: Arc<RssParser>, http_client: Arc<C>, fetcher_source: String) -> Self {
        Self {
            parser,
            http_client,
            fetcher_source,
        }
    }

    /// 執行新架構的 RSS 抓取並回傳原始項目 payload
    pub async fn execute(&self, rss_url: &str, subscription_id: i32) -> Result<RawFetcherResultsPayload, FetchTaskError> {
        match self.parser.fetch_raw_items(rss_url).await {
            Ok(items) => {
                tracing::info!(
                    "Fetch successful: {} raw items",
                    items.len()
                );
                Ok(RawFetcherResultsPayload {
                    subscription_id,
                    items,
                    fetcher_source: self.fetcher_source.clone(),
                    success: true,
                    error_message: None,
                })
            }
            Err(e) => {
                tracing::error!("Fetch failed: {}", e);
                Ok(RawFetcherResultsPayload {
                    subscription_id,
                    items: vec![],
                    fetcher_source: self.fetcher_source.clone(),
                    success: false,
                    error_message: Some(e.to_string()),
                })
            }
        }
    }

    /// 送出結果到 callback URL
    pub async fn send_callback(
        &self,
        callback_url: &str,
        payload: &RawFetcherResultsPayload,
    ) -> Result<(), FetchTaskError> {
        let response = self.http_client.post_json(callback_url, payload).await?;

        if response.status.is_success() {
            tracing::info!("Successfully sent raw results to core service");
            Ok(())
        } else {
            let err_msg = format!("Core service returned error: {}", response.status);
            tracing::error!("{}", err_msg);
            Err(FetchTaskError::CallbackError(HttpError::RequestFailed(err_msg)))
        }
    }

    /// 執行完整的 fetch + callback 流程（新架構）
    pub async fn execute_and_send(
        &self,
        subscription_id: i32,
        rss_url: &str,
        callback_url: &str,
    ) -> Result<RawFetcherResultsPayload, FetchTaskError> {
        let payload = self.execute(rss_url, subscription_id).await?;
        self.send_callback(callback_url, &payload).await?;
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::mock::MockHttpClient;
    use reqwest::StatusCode;

    fn create_test_task(mock_client: MockHttpClient) -> FetchTask<MockHttpClient> {
        let parser = Arc::new(RssParser::new());
        let http_client = Arc::new(mock_client);
        FetchTask::new(parser, http_client, "test-fetcher".to_string())
    }

    #[tokio::test]
    async fn test_execute_creates_payload_on_parse_error() {
        // 使用無效 URL 觸發解析錯誤
        let task = create_test_task(MockHttpClient::new());

        let payload = task.execute("invalid://url", 123).await.unwrap();

        assert_eq!(payload.subscription_id, 123);
        assert!(!payload.success);
        assert!(payload.error_message.is_some());
        assert_eq!(payload.fetcher_source, "test-fetcher");
    }

    #[tokio::test]
    async fn test_send_callback_success() {
        let mock_client = MockHttpClient::with_response(StatusCode::OK, "{}");
        let task = create_test_task(mock_client);

        let payload = RawFetcherResultsPayload {
            subscription_id: 1,
            items: vec![],
            fetcher_source: "test-fetcher".to_string(),
            success: true,
            error_message: None,
        };

        let result = task.send_callback("http://core/raw-fetcher-results", &payload).await;

        assert!(result.is_ok());

        // 驗證請求被正確發送
        let requests = task.http_client.get_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, "http://core/raw-fetcher-results");
        assert!(requests[0].1.contains("subscription_id"));
    }

    #[tokio::test]
    async fn test_send_callback_handles_error_response() {
        let mock_client = MockHttpClient::with_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            r#"{"error": "server error"}"#,
        );
        let task = create_test_task(mock_client);

        let payload = RawFetcherResultsPayload {
            subscription_id: 1,
            items: vec![],
            fetcher_source: "test-fetcher".to_string(),
            success: false,
            error_message: Some("parse error".to_string()),
        };

        let result = task.send_callback("http://core/raw-fetcher-results", &payload).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_callback_handles_network_error() {
        let mock_client = MockHttpClient::with_error(
            HttpError::RequestFailed("connection refused".to_string())
        );
        let task = create_test_task(mock_client);

        let payload = RawFetcherResultsPayload {
            subscription_id: 1,
            items: vec![],
            fetcher_source: "test-fetcher".to_string(),
            success: true,
            error_message: None,
        };

        let result = task.send_callback("http://core/raw-fetcher-results", &payload).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            FetchTaskError::CallbackError(_) => {} // 預期的錯誤類型
            other => panic!("Unexpected error type: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_payload_serialization() {
        let payload = RawFetcherResultsPayload {
            subscription_id: 42,
            items: vec![],
            fetcher_source: "test-fetcher".to_string(),
            success: true,
            error_message: None,
        };

        let json = serde_json::to_string(&payload).unwrap();

        assert!(json.contains("42"));
        assert!(json.contains("true"));
        assert!(json.contains("test-fetcher"));
    }
}
