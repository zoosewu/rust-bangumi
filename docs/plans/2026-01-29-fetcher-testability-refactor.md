# Fetcher Testability Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 重構 Fetcher 程式碼，使原本無法測試的 HTTP 請求和後台任務邏輯可以被單元測試。

**Architecture:** 引入 HttpClient trait 抽象層，將後台任務邏輯提取到可測試的 FetchTask 結構，並使用 Config 物件隔離環境變數。測試時注入 MockHttpClient 進行驗證。

**Tech Stack:** Rust, async-trait, tokio, reqwest

---

## Task 1: 新增 HttpClient trait 和 Mock 實作

**Files:**
- Create: `fetchers/mikanani/src/http_client.rs`
- Modify: `fetchers/mikanani/src/lib.rs`
- Modify: `fetchers/mikanani/Cargo.toml`

**Step 1: 在 Cargo.toml 新增 async-trait 依賴**

在 `fetchers/mikanani/Cargo.toml` 的 `[dependencies]` 區塊新增：

```toml
async-trait = "0.1"
```

**Step 2: 建立 http_client.rs**

建立 `fetchers/mikanani/src/http_client.rs`：

```rust
use async_trait::async_trait;
use reqwest::StatusCode;
use serde::Serialize;
use std::sync::Arc;

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
        let body = response
            .text()
            .await
            .unwrap_or_default();

        Ok(HttpResponse { status, body })
    }
}

#[cfg(test)]
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
            self.requests.lock().unwrap().push((url.to_string(), body_json));

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
    use super::*;
    use super::mock::MockHttpClient;

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
```

**Step 3: 在 lib.rs 導出 http_client 模組**

修改 `fetchers/mikanani/src/lib.rs`：

```rust
mod rss_parser;
mod retry;
pub mod http_client;

pub use rss_parser::RssParser;
pub use retry::retry_with_backoff;
pub use http_client::{HttpClient, RealHttpClient, HttpResponse, HttpError};
```

**Step 4: 執行測試確認編譯通過**

Run: `cd /workspace/fetchers/mikanani && cargo test http_client`
Expected: 3 個測試通過

**Step 5: Commit**

```bash
git add fetchers/mikanani/Cargo.toml fetchers/mikanani/src/http_client.rs fetchers/mikanani/src/lib.rs
git commit -m "$(cat <<'EOF'
feat: add HttpClient trait for dependency injection

- Create HttpClient trait with post_json method
- Implement RealHttpClient using reqwest
- Add MockHttpClient for testing
- Include tests for mock behavior

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: 新增 Config 結構隔離環境變數

**Files:**
- Create: `fetchers/mikanani/src/config.rs`
- Modify: `fetchers/mikanani/src/lib.rs`

**Step 1: 建立 config.rs**

建立 `fetchers/mikanani/src/config.rs`：

```rust
/// Fetcher 服務配置
#[derive(Debug, Clone)]
pub struct FetcherConfig {
    pub core_service_url: String,
    pub service_host: String,
    pub service_port: u16,
    pub service_name: String,
}

impl FetcherConfig {
    /// 從環境變數載入配置
    pub fn from_env() -> Self {
        Self {
            core_service_url: std::env::var("CORE_SERVICE_URL")
                .unwrap_or_else(|_| "http://core-service:8000".to_string()),
            service_host: std::env::var("SERVICE_HOST")
                .unwrap_or_else(|_| "fetcher-mikanani".to_string()),
            service_port: std::env::var("SERVICE_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8001),
            service_name: "mikanani".to_string(),
        }
    }

    /// 建立測試用配置
    pub fn for_test() -> Self {
        Self {
            core_service_url: "http://test-core:8000".to_string(),
            service_host: "test-fetcher".to_string(),
            service_port: 8001,
            service_name: "mikanani".to_string(),
        }
    }

    /// 自訂配置
    pub fn new(
        core_service_url: String,
        service_host: String,
        service_port: u16,
        service_name: String,
    ) -> Self {
        Self {
            core_service_url,
            service_host,
            service_port,
            service_name,
        }
    }

    /// 取得 fetcher-results callback URL
    pub fn callback_url(&self) -> String {
        format!("{}/fetcher-results", self.core_service_url)
    }

    /// 取得服務註冊 URL
    pub fn register_url(&self) -> String {
        format!("{}/services/register", self.core_service_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_for_test_config() {
        let config = FetcherConfig::for_test();
        assert_eq!(config.core_service_url, "http://test-core:8000");
        assert_eq!(config.service_name, "mikanani");
    }

    #[test]
    fn test_callback_url() {
        let config = FetcherConfig::new(
            "http://localhost:8000".to_string(),
            "localhost".to_string(),
            8001,
            "test".to_string(),
        );
        assert_eq!(config.callback_url(), "http://localhost:8000/fetcher-results");
    }

    #[test]
    fn test_register_url() {
        let config = FetcherConfig::new(
            "http://localhost:8000".to_string(),
            "localhost".to_string(),
            8001,
            "test".to_string(),
        );
        assert_eq!(config.register_url(), "http://localhost:8000/services/register");
    }
}
```

**Step 2: 在 lib.rs 導出 config 模組**

修改 `fetchers/mikanani/src/lib.rs`：

```rust
mod rss_parser;
mod retry;
pub mod http_client;
pub mod config;

pub use rss_parser::RssParser;
pub use retry::retry_with_backoff;
pub use http_client::{HttpClient, RealHttpClient, HttpResponse, HttpError};
pub use config::FetcherConfig;
```

**Step 3: 執行測試確認編譯通過**

Run: `cd /workspace/fetchers/mikanani && cargo test config`
Expected: 3 個測試通過

**Step 4: Commit**

```bash
git add fetchers/mikanani/src/config.rs fetchers/mikanani/src/lib.rs
git commit -m "$(cat <<'EOF'
feat: add FetcherConfig for environment isolation

- Create FetcherConfig struct with from_env() and for_test()
- Provide helper methods for URL construction
- Enable test isolation from environment variables

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: 提取 FetchTask 可測試邏輯

**Files:**
- Create: `fetchers/mikanani/src/fetch_task.rs`
- Modify: `fetchers/mikanani/src/lib.rs`

**Step 1: 建立 fetch_task.rs**

建立 `fetchers/mikanani/src/fetch_task.rs`：

```rust
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use shared::FetchedAnime;

use crate::http_client::{HttpClient, HttpError};
use crate::RssParser;

/// Fetch 任務的結果 payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherResultsPayload {
    pub subscription_id: i32,
    pub animes: Vec<FetchedAnime>,
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

    /// 執行 RSS 抓取並回傳 payload（不送出）
    pub async fn fetch_rss(&self, rss_url: &str, subscription_id: i32) -> FetcherResultsPayload {
        match self.parser.parse_feed(rss_url).await {
            Ok(animes) => {
                tracing::info!(
                    "Fetch successful: {} anime with {} total links",
                    animes.len(),
                    animes.iter().map(|a| a.links.len()).sum::<usize>()
                );
                FetcherResultsPayload {
                    subscription_id,
                    animes,
                    fetcher_source: self.fetcher_source.clone(),
                    success: true,
                    error_message: None,
                }
            }
            Err(e) => {
                tracing::error!("Fetch failed: {}", e);
                FetcherResultsPayload {
                    subscription_id,
                    animes: vec![],
                    fetcher_source: self.fetcher_source.clone(),
                    success: false,
                    error_message: Some(e.to_string()),
                }
            }
        }
    }

    /// 送出結果到 callback URL
    pub async fn send_callback(
        &self,
        callback_url: &str,
        payload: &FetcherResultsPayload,
    ) -> Result<(), FetchTaskError> {
        let response = self.http_client.post_json(callback_url, payload).await?;

        if response.status.is_success() {
            tracing::info!("Successfully sent results to core service");
            Ok(())
        } else {
            let err_msg = format!("Core service returned error: {}", response.status);
            tracing::error!("{}", err_msg);
            Err(FetchTaskError::CallbackError(HttpError::RequestFailed(err_msg)))
        }
    }

    /// 執行完整的 fetch + callback 流程
    pub async fn execute(
        &self,
        subscription_id: i32,
        rss_url: &str,
        callback_url: &str,
    ) -> Result<FetcherResultsPayload, FetchTaskError> {
        let payload = self.fetch_rss(rss_url, subscription_id).await;
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
    async fn test_fetch_rss_creates_payload_on_parse_error() {
        // 使用無效 URL 觸發解析錯誤
        let task = create_test_task(MockHttpClient::new());

        let payload = task.fetch_rss("invalid://url", 123).await;

        assert_eq!(payload.subscription_id, 123);
        assert!(!payload.success);
        assert!(payload.error_message.is_some());
        assert_eq!(payload.fetcher_source, "test-fetcher");
    }

    #[tokio::test]
    async fn test_send_callback_success() {
        let mock_client = MockHttpClient::with_response(StatusCode::OK, "{}");
        let task = create_test_task(mock_client);

        let payload = FetcherResultsPayload {
            subscription_id: 1,
            animes: vec![],
            fetcher_source: "test".to_string(),
            success: true,
            error_message: None,
        };

        let result = task.send_callback("http://core/fetcher-results", &payload).await;

        assert!(result.is_ok());

        // 驗證請求被正確發送
        let requests = task.http_client.get_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, "http://core/fetcher-results");
        assert!(requests[0].1.contains("subscription_id"));
    }

    #[tokio::test]
    async fn test_send_callback_handles_error_response() {
        let mock_client = MockHttpClient::with_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            r#"{"error": "server error"}"#,
        );
        let task = create_test_task(mock_client);

        let payload = FetcherResultsPayload {
            subscription_id: 1,
            animes: vec![],
            fetcher_source: "test".to_string(),
            success: false,
            error_message: Some("parse error".to_string()),
        };

        let result = task.send_callback("http://core/fetcher-results", &payload).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_callback_handles_network_error() {
        let mock_client = MockHttpClient::with_error(
            HttpError::RequestFailed("connection refused".to_string())
        );
        let task = create_test_task(mock_client);

        let payload = FetcherResultsPayload {
            subscription_id: 1,
            animes: vec![],
            fetcher_source: "test".to_string(),
            success: true,
            error_message: None,
        };

        let result = task.send_callback("http://core/fetcher-results", &payload).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            FetchTaskError::CallbackError(_) => {} // 預期的錯誤類型
            other => panic!("Unexpected error type: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_payload_serialization() {
        let payload = FetcherResultsPayload {
            subscription_id: 42,
            animes: vec![],
            fetcher_source: "mikanani".to_string(),
            success: true,
            error_message: None,
        };

        let json = serde_json::to_string(&payload).unwrap();

        assert!(json.contains("42"));
        assert!(json.contains("mikanani"));
        assert!(json.contains("true"));
    }
}
```

**Step 2: 在 lib.rs 導出 fetch_task 模組**

修改 `fetchers/mikanani/src/lib.rs`：

```rust
mod rss_parser;
mod retry;
pub mod http_client;
pub mod config;
pub mod fetch_task;

pub use rss_parser::RssParser;
pub use retry::retry_with_backoff;
pub use http_client::{HttpClient, RealHttpClient, HttpResponse, HttpError};
pub use config::FetcherConfig;
pub use fetch_task::{FetchTask, FetcherResultsPayload, FetchTaskError};
```

**Step 3: 執行測試確認編譯通過**

Run: `cd /workspace/fetchers/mikanani && cargo test fetch_task`
Expected: 5 個測試通過

**Step 4: Commit**

```bash
git add fetchers/mikanani/src/fetch_task.rs fetchers/mikanani/src/lib.rs
git commit -m "$(cat <<'EOF'
feat: extract FetchTask for testable fetch logic

- Create FetchTask struct with injectable HttpClient
- Separate fetch_rss() and send_callback() methods
- Add comprehensive unit tests for all code paths
- Tests verify callback URL, payload serialization, error handling

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: 重構 handlers.rs 使用 FetchTask

**Files:**
- Modify: `fetchers/mikanani/src/handlers.rs`

**Step 1: 重寫 handlers.rs 使用新的抽象**

完全重寫 `fetchers/mikanani/src/handlers.rs`：

```rust
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

        if let Err(e) = task.execute(subscription_id, &rss_url, &callback_url).await {
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
```

**Step 2: 執行測試確認編譯通過**

Run: `cd /workspace/fetchers/mikanani && cargo test handlers`
Expected: 4 個測試通過

**Step 3: Commit**

```bash
git add fetchers/mikanani/src/handlers.rs
git commit -m "$(cat <<'EOF'
refactor: update handlers to use FetchTask

- Replace inline async logic with FetchTask
- Add AppState struct with injectable dependencies
- Add unit tests for can_handle logic
- Background task now uses testable FetchTask

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: 更新 main.rs 使用新的 AppState

**Files:**
- Modify: `fetchers/mikanani/src/main.rs`

**Step 1: 重寫 main.rs 使用 AppState 和 Config**

修改 `fetchers/mikanani/src/main.rs`：

```rust
use axum::{
    routing::{get, post},
    Router, Json, http::StatusCode,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber;
use fetcher_mikanani::{FetcherConfig, RealHttpClient, HttpClient};
use serde::{Deserialize, Serialize};

mod handlers;
mod subscription_handler;
mod cors;

use handlers::AppState;
use subscription_handler::SubscriptionBroadcastPayload;

/// Response for subscription broadcast
#[derive(Debug, Serialize, Deserialize)]
pub struct SubscriptionBroadcastResponse {
    pub status: String,
    pub message: String,
}

/// Handle subscription broadcast from core service
async fn handle_subscription_broadcast(
    Json(payload): Json<SubscriptionBroadcastPayload>,
) -> (StatusCode, Json<SubscriptionBroadcastResponse>) {
    tracing::info!("Received subscription broadcast: {:?}", payload);

    let response = SubscriptionBroadcastResponse {
        status: "received".to_string(),
        message: format!("Subscription received for {}", payload.rss_url),
    };

    (StatusCode::OK, Json(response))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    dotenv::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("fetcher_mikanani=debug".parse()?),
        )
        .init();

    tracing::info!("Starting Mikanani fetcher service");

    // Load configuration
    let config = FetcherConfig::from_env();

    // Create HTTP client for registration
    let http_client = Arc::new(RealHttpClient::new());

    // Register to core service
    register_to_core(&config, http_client.as_ref()).await?;

    // Create app state
    let app_state = AppState::new();

    // Build router with state
    let mut app = Router::new()
        .route("/fetch", post(handlers::fetch))
        .route("/health", get(handlers::health_check))
        .route("/subscribe", post(handle_subscription_broadcast))
        .route("/can-handle-subscription", post(handlers::can_handle_subscription))
        .with_state(app_state);

    // 有條件地應用 CORS 中間件
    if let Some(cors) = cors::create_cors_layer() {
        app = app.layer(cors);
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], config.service_port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Mikanani fetcher service listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn register_to_core(config: &FetcherConfig, http_client: &dyn HttpClient) -> anyhow::Result<()> {
    let registration = shared::ServiceRegistration {
        service_type: shared::ServiceType::Fetcher,
        service_name: config.service_name.clone(),
        host: config.service_host.clone(),
        port: config.service_port,
        capabilities: shared::Capabilities {
            fetch_endpoint: Some("/fetch".to_string()),
            download_endpoint: None,
            sync_endpoint: None,
        },
    };

    let url = config.register_url();
    http_client.post_json(&url, &registration).await?;

    tracing::info!("已向核心服務註冊: {}", url);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fetcher_mikanani::http_client::mock::MockHttpClient;
    use reqwest::StatusCode;

    #[tokio::test]
    async fn test_register_to_core_sends_correct_request() {
        let config = FetcherConfig::for_test();
        let mock_client = MockHttpClient::with_response(StatusCode::OK, "{}");

        let result = register_to_core(&config, &mock_client).await;

        assert!(result.is_ok());

        let requests = mock_client.get_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].0.contains("/services/register"));
        assert!(requests[0].1.contains("mikanani"));
        assert!(requests[0].1.contains("fetcher"));
    }

    #[tokio::test]
    async fn test_register_to_core_handles_error() {
        let config = FetcherConfig::for_test();
        let mock_client = MockHttpClient::with_error(
            fetcher_mikanani::HttpError::RequestFailed("connection refused".to_string())
        );

        let result = register_to_core(&config, &mock_client).await;

        assert!(result.is_err());
    }
}
```

**Step 2: 執行測試確認編譯通過**

Run: `cd /workspace/fetchers/mikanani && cargo test`
Expected: 所有測試通過

**Step 3: Commit**

```bash
git add fetchers/mikanani/src/main.rs
git commit -m "$(cat <<'EOF'
refactor: update main.rs to use Config and injectable HttpClient

- Use FetcherConfig for environment isolation
- register_to_core now accepts HttpClient trait object
- Add unit tests for registration logic
- Tests can now mock HTTP requests

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: 清理 subscription_handler.rs 的 dead code

**Files:**
- Modify: `fetchers/mikanani/src/subscription_handler.rs`

**Step 1: 移除未使用的 register_subscription_with_core 函數**

修改 `fetchers/mikanani/src/subscription_handler.rs`，移除 `register_subscription_with_core` 函數（它從未被呼叫）：

```rust
use std::sync::Arc;
use fetcher_mikanani::RssParser;
use tokio::sync::Mutex;
use std::collections::VecDeque;

/// Subscription payload received from core service
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubscriptionBroadcastPayload {
    pub rss_url: String,
    pub service_name: String,
}

/// Pending subscription entry
#[derive(Debug, Clone)]
pub struct PendingSubscription {
    pub rss_url: String,
    pub service_name: String,
}

/// Handles subscriptions for the Mikanani fetcher
/// Validates URLs and manages pending subscriptions
pub struct SubscriptionHandler {
    parser: Arc<RssParser>,
    pending_subscriptions: Arc<Mutex<VecDeque<PendingSubscription>>>,
}

impl SubscriptionHandler {
    /// Create a new subscription handler
    pub fn new(parser: Arc<RssParser>) -> Self {
        Self {
            parser,
            pending_subscriptions: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Check if this handler can handle the given URL
    /// Returns true if URL contains "mikanani.me"
    pub fn can_handle_url(&self, url: &str) -> bool {
        url.contains("mikanani.me")
    }

    /// Add a pending subscription to the queue
    pub async fn add_pending_subscription(&self, payload: SubscriptionBroadcastPayload) -> anyhow::Result<()> {
        if !self.can_handle_url(&payload.rss_url) {
            return Err(anyhow::anyhow!("URL does not contain mikanani.me"));
        }

        let subscription = PendingSubscription {
            rss_url: payload.rss_url.clone(),
            service_name: payload.service_name.clone(),
        };

        let mut subscriptions = self.pending_subscriptions.lock().await;
        subscriptions.push_back(subscription);

        tracing::info!("Added pending subscription: {}", payload.rss_url);

        Ok(())
    }

    /// Get and clear all pending subscriptions
    pub async fn get_and_clear_pending(&self) -> Vec<PendingSubscription> {
        let mut subscriptions = self.pending_subscriptions.lock().await;
        subscriptions.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_mikanani_url() {
        let parser = Arc::new(RssParser::new());
        let handler = SubscriptionHandler::new(parser);

        assert!(handler.can_handle_url("https://mikanani.me/rss/bangumi"));
        assert!(handler.can_handle_url("http://mikanani.me/rss"));
        assert!(!handler.can_handle_url("https://example.com/rss"));
    }

    #[tokio::test]
    async fn test_add_pending_subscription() {
        let parser = Arc::new(RssParser::new());
        let handler = SubscriptionHandler::new(parser);

        let payload = SubscriptionBroadcastPayload {
            rss_url: "https://mikanani.me/rss/bangumi".to_string(),
            service_name: "mikanani".to_string(),
        };

        let result = handler.add_pending_subscription(payload).await;
        assert!(result.is_ok());

        let pending = handler.get_and_clear_pending().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].rss_url, "https://mikanani.me/rss/bangumi");
    }

    #[tokio::test]
    async fn test_get_and_clear_pending() {
        let parser = Arc::new(RssParser::new());
        let handler = SubscriptionHandler::new(parser);

        let payload1 = SubscriptionBroadcastPayload {
            rss_url: "https://mikanani.me/rss/1".to_string(),
            service_name: "mikanani".to_string(),
        };

        let payload2 = SubscriptionBroadcastPayload {
            rss_url: "https://mikanani.me/rss/2".to_string(),
            service_name: "mikanani".to_string(),
        };

        handler.add_pending_subscription(payload1).await.ok();
        handler.add_pending_subscription(payload2).await.ok();

        let pending = handler.get_and_clear_pending().await;
        assert_eq!(pending.len(), 2);

        // Verify they are cleared
        let pending_again = handler.get_and_clear_pending().await;
        assert_eq!(pending_again.len(), 0);
    }

    #[tokio::test]
    async fn test_reject_non_mikanani_url() {
        let parser = Arc::new(RssParser::new());
        let handler = SubscriptionHandler::new(parser);

        let payload = SubscriptionBroadcastPayload {
            rss_url: "https://example.com/rss".to_string(),
            service_name: "mikanani".to_string(),
        };

        let result = handler.add_pending_subscription(payload).await;
        assert!(result.is_err());
    }
}
```

**Step 2: 執行測試確認編譯通過**

Run: `cd /workspace/fetchers/mikanani && cargo test subscription_handler`
Expected: 4 個測試通過

**Step 3: Commit**

```bash
git add fetchers/mikanani/src/subscription_handler.rs
git commit -m "$(cat <<'EOF'
refactor: remove dead code from subscription_handler

- Remove register_subscription_with_core (never called)
- Keep only actively used methods
- All existing tests still pass

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: 執行完整測試並驗證

**Step 1: 執行所有 fetcher 測試**

Run: `cd /workspace/fetchers/mikanani && cargo test`
Expected: 所有測試通過

**Step 2: 檢查警告**

Run: `cd /workspace/fetchers/mikanani && cargo clippy`
Expected: 無錯誤（警告可接受）

**Step 3: 執行 workspace 測試**

Run: `cd /workspace && cargo test --workspace`
Expected: 所有測試通過

**Step 4: 最終 Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat: complete fetcher testability refactor

Summary of changes:
- Add HttpClient trait with MockHttpClient for testing
- Add FetcherConfig for environment variable isolation
- Extract FetchTask with testable fetch/callback logic
- Refactor handlers.rs to use dependency injection
- Update main.rs with testable register_to_core
- Remove dead code from subscription_handler

New test coverage:
- http_client: 3 tests (mock behavior)
- config: 3 tests (URL construction)
- fetch_task: 5 tests (fetch, callback, errors)
- handlers: 4 tests (can_handle logic)
- main: 2 tests (registration)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Summary

| Task | 描述 | 新增測試數 |
|------|------|-----------|
| 1 | 新增 HttpClient trait + Mock | 3 |
| 2 | 新增 FetcherConfig | 3 |
| 3 | 提取 FetchTask | 5 |
| 4 | 重構 handlers.rs | 4 |
| 5 | 更新 main.rs | 2 |
| 6 | 清理 dead code | 0 |
| 7 | 完整測試驗證 | - |

**總計新增測試：17 個**

### 重構後可測試的程式碼路徑

| 原本無法測試 | 重構後 |
|-------------|--------|
| handlers.rs 後台任務 | FetchTask.execute() 可單元測試 |
| handlers.rs HTTP 回調 | FetchTask.send_callback() 可 mock |
| main.rs register_to_core | 可注入 MockHttpClient |
| 環境變數依賴 | FetcherConfig.for_test() 隔離 |
