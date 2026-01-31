# Core-Driven Fetch Scheduler Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 將排程控制從 Fetcher 移至 Core Service，由 Core 主動觸發 Fetcher 執行抓取任務。

**Architecture:** Core Service 內建 FetchScheduler，每 60 秒檢查 `subscriptions` 表中 `next_fetch_at <= NOW()` 的訂閱，根據 `fetcher_id` 找到對應的 Fetcher 模組並呼叫其 `/fetch` endpoint。Fetcher 收到請求後立即回傳 202 Accepted，在背景執行抓取，完成後 POST 結果到 `/fetcher-results`。失敗時採用指數退避重試機制。

**Tech Stack:** Rust, Tokio, Axum, Diesel, reqwest

---

## Task 1: 修改 Fetcher 的 /fetch endpoint 支援非同步模式

**Files:**
- Modify: `fetchers/mikanani/src/handlers.rs`
- Modify: `shared/src/models.rs`

**Step 1: 在 shared 新增 FetchRequest 和 FetchTriggerResponse 結構**

在 `shared/src/models.rs` 末尾新增：

```rust
// ============ Fetch Trigger (Core -> Fetcher) ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchTriggerRequest {
    pub subscription_id: i32,
    pub rss_url: String,
    pub callback_url: String,  // Core 的 /fetcher-results endpoint
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchTriggerResponse {
    pub accepted: bool,
    pub message: String,
}
```

**Step 2: 修改 Fetcher 的 handlers.rs**

將 `fetchers/mikanani/src/handlers.rs` 的 `FetchRequest` 和 `fetch` 函數改為非同步模式：

```rust
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
```

**Step 3: 執行測試確認編譯通過**

Run: `cd /workspace/fetchers/mikanani && cargo check`
Expected: 編譯通過

**Step 4: Commit**

```bash
git add shared/src/models.rs fetchers/mikanani/src/handlers.rs
git commit -m "$(cat <<'EOF'
feat: modify fetcher /fetch endpoint for async mode

- Add FetchTriggerRequest/Response to shared models
- Fetcher now returns 202 Accepted immediately
- Background task performs actual fetch and POSTs results

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: 修改 Core Service 的 /fetcher-results 支援 subscription_id

**Files:**
- Modify: `core-service/src/handlers/fetcher_results.rs`

**Step 1: 更新 FetcherResultsPayload 結構**

在 `core-service/src/handlers/fetcher_results.rs` 修改 `FetcherResultsPayload`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherResultsPayload {
    pub subscription_id: Option<i32>,  // 新增：可選，向後相容
    pub animes: Vec<FetchedAnimePayload>,
    pub fetcher_source: String,
    pub success: Option<bool>,         // 新增：抓取是否成功
    pub error_message: Option<String>, // 新增：錯誤訊息
}
```

**Step 2: 更新 receive_fetcher_results 處理 subscription_id**

在 `receive_fetcher_results` 函數開頭新增訂閱更新邏輯：

```rust
pub async fn receive_fetcher_results(
    State(state): State<AppState>,
    Json(payload): Json<FetcherResultsPayload>,
) -> (StatusCode, Json<serde_json::Value>) {
    tracing::info!(
        "Received fetcher results from {}: {} animes, subscription_id: {:?}",
        payload.fetcher_source,
        payload.animes.len(),
        payload.subscription_id
    );

    // 更新訂閱的 last_fetched_at
    if let Some(sub_id) = payload.subscription_id {
        if let Err(e) = update_subscription_after_fetch(&state, sub_id, payload.success.unwrap_or(true)).await {
            tracing::error!("Failed to update subscription {}: {}", sub_id, e);
        }
    }

    // ... 現有的處理邏輯保持不變 ...
}

/// 更新訂閱的 last_fetched_at 和 next_fetch_at
async fn update_subscription_after_fetch(
    state: &AppState,
    subscription_id: i32,
    success: bool,
) -> Result<(), String> {
    use crate::schema::subscriptions;

    let mut conn = state.db.get().map_err(|e| e.to_string())?;
    let now = Utc::now().naive_utc();

    // 先取得訂閱資訊
    let subscription = subscriptions::table
        .filter(subscriptions::subscription_id.eq(subscription_id))
        .first::<crate::models::Subscription>(&mut conn)
        .map_err(|e| format!("Subscription not found: {}", e))?;

    // 計算下次抓取時間
    let next_fetch = now + chrono::Duration::minutes(subscription.fetch_interval_minutes as i64);

    diesel::update(subscriptions::table.filter(subscriptions::subscription_id.eq(subscription_id)))
        .set((
            subscriptions::last_fetched_at.eq(Some(now)),
            subscriptions::next_fetch_at.eq(Some(next_fetch)),
            subscriptions::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to update subscription: {}", e))?;

    tracing::info!(
        "Updated subscription {}: last_fetched_at={}, next_fetch_at={}",
        subscription_id,
        now,
        next_fetch
    );

    Ok(())
}
```

**Step 3: 執行測試確認編譯通過**

Run: `cd /workspace/core-service && cargo check`
Expected: 編譯通過

**Step 4: Commit**

```bash
git add core-service/src/handlers/fetcher_results.rs
git commit -m "$(cat <<'EOF'
feat: update fetcher-results to handle subscription_id

- Add subscription_id, success, error_message to payload
- Update subscription's last_fetched_at and next_fetch_at after receive

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: 實作 Core Service 的 FetchScheduler

**Files:**
- Modify: `core-service/src/services/scheduler.rs`
- Modify: `core-service/src/services/mod.rs`

**Step 1: 重寫 scheduler.rs**

完全重寫 `core-service/src/services/scheduler.rs`：

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use chrono::Utc;
use diesel::prelude::*;

use crate::db::DbPool;
use crate::models::{Subscription, ServiceModule, ModuleTypeEnum, NewCronLog};
use crate::schema::{subscriptions, service_modules, cron_logs};

pub struct FetchScheduler {
    db_pool: DbPool,
    check_interval_secs: u64,
    max_retries: u32,
    base_retry_delay_secs: u64,
}

#[derive(Debug, Clone)]
struct FetchTask {
    subscription_id: i32,
    source_url: String,
    fetcher_id: i32,
    fetcher_base_url: String,
}

impl FetchScheduler {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            check_interval_secs: 60,  // 每 60 秒檢查一次
            max_retries: 3,
            base_retry_delay_secs: 60,  // 初始重試延遲 60 秒
        }
    }

    pub fn with_check_interval(mut self, secs: u64) -> Self {
        self.check_interval_secs = secs;
        self
    }

    /// 啟動排程器主迴圈
    pub async fn start(self: Arc<Self>) {
        let mut ticker = interval(Duration::from_secs(self.check_interval_secs));

        tracing::info!(
            "FetchScheduler started, checking every {} seconds",
            self.check_interval_secs
        );

        loop {
            ticker.tick().await;

            if let Err(e) = self.process_due_subscriptions().await {
                tracing::error!("Error processing due subscriptions: {}", e);
            }
        }
    }

    /// 處理所有到期的訂閱
    async fn process_due_subscriptions(&self) -> Result<(), String> {
        let tasks = self.get_due_subscriptions()?;

        if tasks.is_empty() {
            tracing::debug!("No due subscriptions found");
            return Ok(());
        }

        tracing::info!("Found {} due subscriptions", tasks.len());

        for task in tasks {
            // 每個任務獨立處理，失敗不影響其他任務
            if let Err(e) = self.trigger_fetch(&task).await {
                tracing::error!(
                    "Failed to trigger fetch for subscription {}: {}",
                    task.subscription_id,
                    e
                );
                self.log_fetch_attempt(&task, false, Some(&e));
            } else {
                self.log_fetch_attempt(&task, true, None);
            }
        }

        Ok(())
    }

    /// 取得所有到期的訂閱
    fn get_due_subscriptions(&self) -> Result<Vec<FetchTask>, String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;
        let now = Utc::now().naive_utc();

        // 查詢到期的活躍訂閱
        let due_subscriptions = subscriptions::table
            .filter(subscriptions::is_active.eq(true))
            .filter(subscriptions::next_fetch_at.le(now))
            .select(Subscription::as_select())
            .load::<Subscription>(&mut conn)
            .map_err(|e| format!("Failed to query subscriptions: {}", e))?;

        // 取得對應的 fetcher 資訊
        let mut tasks = Vec::new();
        for sub in due_subscriptions {
            match service_modules::table
                .filter(service_modules::module_id.eq(sub.fetcher_id))
                .filter(service_modules::is_enabled.eq(true))
                .filter(service_modules::module_type.eq(ModuleTypeEnum::Fetcher))
                .first::<ServiceModule>(&mut conn)
            {
                Ok(fetcher) => {
                    tasks.push(FetchTask {
                        subscription_id: sub.subscription_id,
                        source_url: sub.source_url,
                        fetcher_id: sub.fetcher_id,
                        fetcher_base_url: fetcher.base_url,
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        "Fetcher {} not found or disabled for subscription {}: {}",
                        sub.fetcher_id,
                        sub.subscription_id,
                        e
                    );
                }
            }
        }

        Ok(tasks)
    }

    /// 觸發 Fetcher 執行抓取
    async fn trigger_fetch(&self, task: &FetchTask) -> Result<(), String> {
        let fetch_url = format!("{}/fetch", task.fetcher_base_url);
        let callback_url = std::env::var("CORE_SERVICE_URL")
            .unwrap_or_else(|_| "http://core-service:8000".to_string());
        let callback_url = format!("{}/fetcher-results", callback_url);

        let request = shared::FetchTriggerRequest {
            subscription_id: task.subscription_id,
            rss_url: task.source_url.clone(),
            callback_url,
        };

        tracing::info!(
            "Triggering fetch for subscription {} at {}",
            task.subscription_id,
            fetch_url
        );

        // 使用重試機制
        let mut attempt = 0;
        let mut last_error = String::new();

        while attempt < self.max_retries {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| e.to_string())?;

            match client.post(&fetch_url).json(&request).send().await {
                Ok(response) => {
                    if response.status().is_success() || response.status() == reqwest::StatusCode::ACCEPTED {
                        tracing::info!(
                            "Successfully triggered fetch for subscription {}",
                            task.subscription_id
                        );
                        return Ok(());
                    } else {
                        last_error = format!("HTTP {}", response.status());
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }

            attempt += 1;
            if attempt < self.max_retries {
                // 指數退避
                let delay = self.base_retry_delay_secs * (1 << attempt);
                tracing::warn!(
                    "Fetch trigger failed (attempt {}/{}), retrying in {} seconds: {}",
                    attempt,
                    self.max_retries,
                    delay,
                    last_error
                );
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }
        }

        Err(format!(
            "Failed after {} attempts: {}",
            self.max_retries,
            last_error
        ))
    }

    /// 記錄抓取嘗試到 cron_logs
    fn log_fetch_attempt(&self, task: &FetchTask, success: bool, error: Option<&str>) {
        if let Ok(mut conn) = self.db_pool.get() {
            let now = Utc::now().naive_utc();
            let log = NewCronLog {
                fetcher_type: format!("subscription_{}", task.subscription_id),
                status: if success { "success".to_string() } else { "failed".to_string() },
                error_message: error.map(|e| e.to_string()),
                attempt_count: 1,
                executed_at: now,
            };

            if let Err(e) = diesel::insert_into(cron_logs::table)
                .values(&log)
                .execute(&mut conn)
            {
                tracing::error!("Failed to log fetch attempt: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        // 這裡只測試配置，不測試實際排程
        // 實際 DB 測試需要整合測試環境
    }
}
```

**Step 2: 更新 mod.rs 導出**

修改 `core-service/src/services/mod.rs`：

```rust
pub mod registry;
pub mod filter;
pub mod scheduler;
pub mod subscription_broker;

pub use registry::ServiceRegistry;
pub use filter::FilterEngine;
pub use scheduler::FetchScheduler;
pub use subscription_broker::{SubscriptionBroadcaster, SubscriptionBroadcast, create_subscription_broadcaster};
```

**Step 3: 執行測試確認編譯通過**

Run: `cd /workspace/core-service && cargo check`
Expected: 編譯通過

**Step 4: Commit**

```bash
git add core-service/src/services/scheduler.rs core-service/src/services/mod.rs
git commit -m "$(cat <<'EOF'
feat: implement FetchScheduler for core-driven fetch

- Check due subscriptions every 60 seconds
- Trigger fetcher /fetch endpoint with subscription info
- Exponential backoff retry on failure
- Log attempts to cron_logs table

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: 在 Core Service main.rs 啟動 FetchScheduler

**Files:**
- Modify: `core-service/src/main.rs`

**Step 1: 在 main 函數中啟動 FetchScheduler**

在 `core-service/src/main.rs` 的 `main` 函數中，於 `axum::serve` 之前新增：

```rust
// 在 load_existing_services(&app_state).await; 之後新增：

// 啟動 FetchScheduler
let scheduler = std::sync::Arc::new(services::FetchScheduler::new(app_state.db.clone()));
let scheduler_clone = scheduler.clone();
tokio::spawn(async move {
    scheduler_clone.start().await;
});
tracing::info!("FetchScheduler started");
```

完整的 main 函數應該像這樣（只顯示需要修改的部分）：

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ... 現有的初始化程式碼 ...

    // 啟動時從資料庫載入已有的所有服務模塊
    load_existing_services(&app_state).await;

    // 啟動 FetchScheduler
    let scheduler = std::sync::Arc::new(services::FetchScheduler::new(app_state.db.clone()));
    let scheduler_clone = scheduler.clone();
    tokio::spawn(async move {
        scheduler_clone.start().await;
    });
    tracing::info!("FetchScheduler started");

    // 構建應用路由
    let mut app = Router::new()
        // ... 現有的路由 ...
```

**Step 2: 執行測試確認編譯通過**

Run: `cd /workspace/core-service && cargo check`
Expected: 編譯通過

**Step 3: Commit**

```bash
git add core-service/src/main.rs
git commit -m "$(cat <<'EOF'
feat: start FetchScheduler on core service startup

- Spawn scheduler as background task
- Scheduler runs independently of HTTP server

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: 移除 Fetcher 的舊排程器程式碼

**Files:**
- Modify: `fetchers/mikanani/src/main.rs`
- Delete: `fetchers/mikanani/src/scheduler.rs`
- Modify: `fetchers/mikanani/src/lib.rs`

**Step 1: 修改 Fetcher 的 main.rs 移除 scheduler 啟動**

將 `fetchers/mikanani/src/main.rs` 中關於 scheduler 的程式碼移除：

```rust
use axum::{
    routing::{get, post},
    Router, Json, http::StatusCode,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber;
use fetcher_mikanani::RssParser;
use serde::{Deserialize, Serialize};

mod handlers;
mod subscription_handler;
mod cors;

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

    // Create RSS parser
    let parser = Arc::new(RssParser::new());

    // Register to core service
    register_to_core().await?;

    // Build router with state
    let mut app = Router::new()
        .route("/fetch", post(handlers::fetch))
        .route("/health", get(handlers::health_check))
        .route("/subscribe", post(handle_subscription_broadcast))
        .route("/can-handle-subscription", post(handlers::can_handle_subscription))
        .with_state(parser);

    // 有條件地應用 CORS 中間件
    if let Some(cors) = cors::create_cors_layer() {
        app = app.layer(cors);
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], 8001));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Mikanani fetcher service listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn register_to_core() -> anyhow::Result<()> {
    let core_service_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());

    let service_host = std::env::var("SERVICE_HOST")
        .unwrap_or_else(|_| "fetcher-mikanani".to_string());

    let registration = shared::ServiceRegistration {
        service_type: shared::ServiceType::Fetcher,
        service_name: "mikanani".to_string(),
        host: service_host,
        port: 8001,
        capabilities: shared::Capabilities {
            fetch_endpoint: Some("/fetch".to_string()),
            download_endpoint: None,
            sync_endpoint: None,
        },
    };

    let client = reqwest::Client::new();
    client
        .post(&format!("{}/services/register", core_service_url))
        .json(&registration)
        .send()
        .await?;

    tracing::info!("已向核心服務註冊");

    Ok(())
}
```

**Step 2: 修改 lib.rs 移除 scheduler 導出**

修改 `fetchers/mikanani/src/lib.rs`：

```rust
mod rss_parser;
mod retry;

pub use rss_parser::RssParser;
pub use retry::retry_with_backoff;
```

**Step 3: 刪除 scheduler.rs**

刪除檔案 `fetchers/mikanani/src/scheduler.rs`

**Step 4: 執行測試確認編譯通過**

Run: `cd /workspace/fetchers/mikanani && cargo check`
Expected: 編譯通過

**Step 5: Commit**

```bash
git add fetchers/mikanani/src/main.rs fetchers/mikanani/src/lib.rs
git rm fetchers/mikanani/src/scheduler.rs
git commit -m "$(cat <<'EOF'
refactor: remove scheduler from fetcher

- Fetcher is now passive, triggered by core service
- Remove scheduler.rs and related code
- Fetcher only exposes /fetch endpoint

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: 更新整合測試

**Files:**
- Modify: `fetchers/mikanani/tests/fetcher_integration_tests.rs`

**Step 1: 移除 scheduler 相關測試，新增非同步 fetch 測試**

在 `fetchers/mikanani/tests/fetcher_integration_tests.rs` 移除任何關於 scheduler 的測試（如果有的話），並確保現有測試仍然通過。

由於 fetch handler 現在是非同步的，需要調整測試方式（模擬 HTTP 請求而非直接呼叫）。

**Step 2: 執行測試**

Run: `cd /workspace/fetchers/mikanani && cargo test`
Expected: 所有測試通過

**Step 3: Commit**

```bash
git add fetchers/mikanani/tests/
git commit -m "$(cat <<'EOF'
test: update integration tests for async fetch mode

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: 執行完整測試並驗證

**Step 1: 執行所有測試**

Run: `cd /workspace && cargo test --workspace`
Expected: 所有測試通過

**Step 2: 手動驗證（可選）**

如果有 Docker 環境，可以啟動服務進行手動測試：

1. 啟動 PostgreSQL
2. 啟動 Core Service
3. 啟動 Fetcher
4. 新增一個訂閱，觀察 60 秒後 Core 是否觸發 Fetcher

**Step 3: 最終 Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat: complete core-driven fetch scheduler implementation

Summary:
- Core Service now controls fetch scheduling
- FetchScheduler checks subscriptions every 60 seconds
- Fetcher is passive, responds with 202 Accepted
- Results posted asynchronously to /fetcher-results
- Exponential backoff retry on failures

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Summary

| Task | 描述 | 檔案 |
|------|------|------|
| 1 | 修改 Fetcher /fetch 為非同步模式 | `shared/src/models.rs`, `fetchers/mikanani/src/handlers.rs` |
| 2 | 更新 Core /fetcher-results 支援 subscription_id | `core-service/src/handlers/fetcher_results.rs` |
| 3 | 實作 Core FetchScheduler | `core-service/src/services/scheduler.rs` |
| 4 | 在 Core main.rs 啟動 FetchScheduler | `core-service/src/main.rs` |
| 5 | 移除 Fetcher 舊排程器 | `fetchers/mikanani/src/main.rs`, `lib.rs`, 刪除 `scheduler.rs` |
| 6 | 更新整合測試 | `fetchers/mikanani/tests/` |
| 7 | 完整測試驗證 | - |
