use async_trait::async_trait;
use reqwest::StatusCode;
use serde::Serialize;

/// HTTP 回應的抽象
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: StatusCode,
    pub body: String,
}

/// HTTP 錯誤類型
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),
}

/// HTTP Client trait - 允許依賴注入和 mock
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn post_json<T: Serialize + Send + Sync>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<HttpResponse, HttpError>;
}

/// 真實的 HTTP Client 實作
pub struct RealHttpClient {
    client: reqwest::Client,
}

impl RealHttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(timeout_secs))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

impl Default for RealHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for RealHttpClient {
    async fn post_json<T: Serialize + Send + Sync>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<HttpResponse, HttpError> {
        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| HttpError::RequestFailed(e.to_string()))?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        Ok(HttpResponse { status, body })
    }
}

pub mod mock {
    use super::*;
    use std::sync::Mutex;

    /// Mock HTTP Client 用於測試
    pub struct MockHttpClient {
        /// 預設回應
        pub response: Mutex<Option<Result<HttpResponse, HttpError>>>,
        /// 記錄收到的請求
        pub requests: Mutex<Vec<(String, String)>>,
    }

    impl MockHttpClient {
        pub fn new() -> Self {
            Self {
                response: Mutex::new(Some(Ok(HttpResponse {
                    status: StatusCode::OK,
                    body: "{}".to_string(),
                }))),
                requests: Mutex::new(Vec::new()),
            }
        }

        pub fn with_response(status: StatusCode, body: &str) -> Self {
            Self {
                response: Mutex::new(Some(Ok(HttpResponse {
                    status,
                    body: body.to_string(),
                }))),
                requests: Mutex::new(Vec::new()),
            }
        }

        pub fn with_error(error: HttpError) -> Self {
            Self {
                response: Mutex::new(Some(Err(error))),
                requests: Mutex::new(Vec::new()),
            }
        }

        /// 取得收到的請求記錄
        pub fn get_requests(&self) -> Vec<(String, String)> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl Default for MockHttpClient {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl HttpClient for MockHttpClient {
        async fn post_json<T: Serialize + Send + Sync>(
            &self,
            url: &str,
            body: &T,
        ) -> Result<HttpResponse, HttpError> {
            // 記錄請求
            let body_json = serde_json::to_string(body)
                .map_err(|e| HttpError::SerializationFailed(e.to_string()))?;
            self.requests
                .lock()
                .unwrap()
                .push((url.to_string(), body_json));

            // 回傳預設回應
            self.response
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Ok(HttpResponse {
                    status: StatusCode::OK,
                    body: "{}".to_string(),
                }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockHttpClient;
    use super::*;

    #[tokio::test]
    async fn test_mock_client_records_requests() {
        let client = MockHttpClient::new();

        #[derive(Serialize)]
        struct TestBody {
            value: i32,
        }

        let body = TestBody { value: 42 };
        let result = client.post_json("http://test.com/api", &body).await;

        assert!(result.is_ok());
        let requests = client.get_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, "http://test.com/api");
        assert!(requests[0].1.contains("42"));
    }

    #[tokio::test]
    async fn test_mock_client_returns_configured_response() {
        let client = MockHttpClient::with_response(StatusCode::CREATED, r#"{"id": 1}"#);

        #[derive(Serialize)]
        struct Empty {}

        let result = client.post_json("http://test.com", &Empty {}).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status, StatusCode::CREATED);
        assert!(response.body.contains("id"));
    }

    #[tokio::test]
    async fn test_mock_client_returns_error() {
        let client = MockHttpClient::with_error(HttpError::RequestFailed("timeout".to_string()));

        #[derive(Serialize)]
        struct Empty {}

        let result = client.post_json("http://test.com", &Empty {}).await;

        assert!(result.is_err());
    }
}
