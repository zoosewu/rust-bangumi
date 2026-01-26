# RSS 訂閱管理機制重構實現計劃

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 將 RSS 訂閱管理從分散的 Fetcher 端遷移到 Core 數據庫中央管理，建立發布-訂閱的模塊發現機制，支持多個 Fetcher 競爭同一 URL 的仲裁。

**Architecture:**
建立兩張新表管理模塊和訂閱 URL：
- `fetcher_modules` - 記錄所有可用的 Fetcher 模塊（ID、名稱）
- `rss_subscriptions` - 記錄所有訂閱的 RSS URL 及其所屬的 Fetcher ID

實現基於事件驅動的訂閱流程：新訂閱時廣播給所有 Fetcher → 每個 Fetcher 自主判斷是否應接管 → 支持衝突解決（用戶選擇）→ Core 持久化 → Fetcher 執行任務時從 Core 獲取自己的 URL 列表 → 返回動畫數據時包含源 URL 外鍵。

**Tech Stack:** Rust + Axum + Diesel + PostgreSQL，保留現有技術棧，添加事件廣播機制（基於內存 broadcast channel）。

---

## Task 1: 創建數據庫遷移（新表）

**Files:**
- Create: `core-service/migrations/2026-01-22-000001_create_fetcher_and_subscription_tables/up.sql`
- Create: `core-service/migrations/2026-01-22-000001_create_fetcher_and_subscription_tables/down.sql`

**Step 1: 編寫 UP 遷移腳本**

創建 `core-service/migrations/2026-01-22-000001_create_fetcher_and_subscription_tables/up.sql`：

```sql
-- 表 1: 記錄所有註冊的 Fetcher 模塊
CREATE TABLE fetcher_modules (
    fetcher_id SERIAL PRIMARY KEY,
    fetcher_name VARCHAR(100) NOT NULL UNIQUE,
    service_id UUID NOT NULL UNIQUE,
    host VARCHAR(255) NOT NULL,
    port INT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 表 2: 記錄所有訂閱的 RSS URL
CREATE TABLE rss_subscriptions (
    rss_url TEXT PRIMARY KEY,
    fetcher_id INT NOT NULL,
    subscription_name VARCHAR(255),
    is_active BOOLEAN NOT NULL DEFAULT true,
    last_fetched_at TIMESTAMP,
    error_message TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (fetcher_id) REFERENCES fetcher_modules(fetcher_id) ON DELETE CASCADE
);

-- 表 3: 記錄 URL 與 Fetcher 的衝突歷史（用於仲裁）
CREATE TABLE subscription_conflicts (
    conflict_id SERIAL PRIMARY KEY,
    rss_url TEXT NOT NULL,
    candidate_fetcher_ids INT[] NOT NULL,  -- 多個競爭者的 ID 陣列
    resolved_fetcher_id INT,
    resolved_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (rss_url) REFERENCES rss_subscriptions(rss_url) ON DELETE CASCADE
);

-- 創建索引以加速查詢
CREATE INDEX idx_rss_subscriptions_fetcher_id ON rss_subscriptions(fetcher_id);
CREATE INDEX idx_rss_subscriptions_is_active ON rss_subscriptions(is_active);
CREATE INDEX idx_fetcher_modules_service_id ON fetcher_modules(service_id);
```

**Step 2: 編寫 DOWN 遷移腳本**

創建 `core-service/migrations/2026-01-22-000001_create_fetcher_and_subscription_tables/down.sql`：

```sql
DROP TABLE IF EXISTS subscription_conflicts;
DROP TABLE IF EXISTS rss_subscriptions;
DROP TABLE IF EXISTS fetcher_modules;
```

**Step 3: 運行遷移**

```bash
cd /nodejs/rust-bangumi/core-service
diesel migration run
```

Expected: 遷移成功，three tables 創建完成。

**Step 4: 驗證遷移文件存在**

```bash
ls -la /nodejs/rust-bangumi/core-service/migrations/ | grep 2026-01-22
```

Expected: 兩個文件 `2026-01-22-000001_create_fetcher_and_subscription_tables/up.sql` 和 `down.sql` 存在。

**Step 5: Commit**

```bash
cd /nodejs/rust-bangumi
git add core-service/migrations/
git commit -m "feat: add database migrations for RSS subscription management

- Create fetcher_modules table for fetcher registration
- Create rss_subscriptions table for URL management
- Create subscription_conflicts table for conflict resolution history"
```

---

## Task 2: 更新 Diesel Schema 和模型定義

**Files:**
- Modify: `core-service/src/schema.rs` (自動生成，需驗證)
- Create: `core-service/src/models/db.rs` (新增表的模型)

**Step 1: 重新生成 Diesel Schema**

```bash
cd /nodejs/rust-bangumi/core-service
diesel print-schema > src/schema.rs
```

Expected: `schema.rs` 包含新表的 schema 定義。

**Step 2: 創建資料模型結構**

編輯 `core-service/src/models/db.rs`，添加以下模型定義（在文件末尾）：

```rust
use chrono::NaiveDateTime;
use diesel::prelude::*;

// === Fetcher Modules ===
#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = fetcher_modules)]
pub struct FetcherModule {
    pub fetcher_id: i32,
    pub fetcher_name: String,
    pub service_id: String,  // UUID as String
    pub host: String,
    pub port: i32,
    pub is_active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = fetcher_modules)]
pub struct NewFetcherModule {
    pub fetcher_name: String,
    pub service_id: String,
    pub host: String,
    pub port: i32,
    pub is_active: bool,
}

// === RSS Subscriptions ===
#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = rss_subscriptions)]
pub struct RssSubscription {
    pub rss_url: String,
    pub fetcher_id: i32,
    pub subscription_name: Option<String>,
    pub is_active: bool,
    pub last_fetched_at: Option<NaiveDateTime>,
    pub error_message: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = rss_subscriptions)]
pub struct NewRssSubscription {
    pub rss_url: String,
    pub fetcher_id: i32,
    pub subscription_name: Option<String>,
    pub is_active: bool,
}

// === Subscription Conflicts ===
#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = subscription_conflicts)]
pub struct SubscriptionConflict {
    pub conflict_id: i32,
    pub rss_url: String,
    pub candidate_fetcher_ids: Vec<i32>,  // Diesel 對 PostgreSQL int[] 支持
    pub resolved_fetcher_id: Option<i32>,
    pub resolved_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = subscription_conflicts)]
pub struct NewSubscriptionConflict {
    pub rss_url: String,
    pub candidate_fetcher_ids: Vec<i32>,
}
```

**Step 3: 編譯並驗證模型**

```bash
cd /nodejs/rust-bangumi/core-service
cargo check
```

Expected: 編譯成功，無錯誤。

**Step 4: Commit**

```bash
cd /nodejs/rust-bangumi
git add core-service/src/schema.rs core-service/src/models/db.rs
git commit -m "feat: add Diesel models for RSS subscription management

- Add FetcherModule, RssSubscription, SubscriptionConflict models
- Update schema.rs with new table definitions"
```

---

## Task 3: 實現 Core Service 訂閱管理 API

**Files:**
- Create: `core-service/src/handlers/subscriptions.rs`
- Modify: `core-service/src/handlers/mod.rs` (添加 mod 聲明)
- Modify: `core-service/src/main.rs` (添加路由)

**Step 1: 創建訂閱 API 處理器**

創建 `core-service/src/handlers/subscriptions.rs`：

```rust
use crate::models::db::*;
use crate::schema::*;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct CreateSubscriptionRequest {
    pub rss_url: String,
    pub subscription_name: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SubscriptionResponse {
    pub rss_url: String,
    pub fetcher_id: i32,
    pub subscription_name: Option<String>,
    pub is_active: bool,
}

#[derive(Serialize, Deserialize)]
pub struct FetcherModuleResponse {
    pub fetcher_id: i32,
    pub fetcher_name: String,
    pub service_id: String,
    pub host: String,
    pub port: i32,
    pub is_active: bool,
}

// 創建訂閱
pub async fn create_subscription(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> impl IntoResponse {
    use crate::schema::fetcher_modules::dsl::*;

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "DB connection failed").into_response(),
    };

    // 查詢所有活躍的 Fetcher
    let fetchers: Vec<FetcherModule> = match fetcher_modules
        .filter(is_active.eq(true))
        .load(&mut conn)
    {
        Ok(f) => f,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load fetchers").into_response(),
    };

    if fetchers.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            "No active fetchers available",
        ).into_response();
    }

    // TODO: 廣播給所有 Fetcher，等待他們的響應（實現在 Task 4）
    // 這裡先返回 202 Accepted，表示訂閱請求已接收

    (StatusCode::ACCEPTED, "Subscription request accepted").into_response()
}

// 獲取所有訂閱
pub async fn list_subscriptions(
    State(state): State<AppState>,
) -> impl IntoResponse {
    use crate::schema::rss_subscriptions::dsl::*;

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "DB connection failed").into_response(),
    };

    match rss_subscriptions
        .filter(is_active.eq(true))
        .load::<RssSubscription>(&mut conn)
    {
        Ok(subs) => {
            let response: Vec<SubscriptionResponse> = subs
                .into_iter()
                .map(|s| SubscriptionResponse {
                    rss_url: s.rss_url,
                    fetcher_id: s.fetcher_id,
                    subscription_name: s.subscription_name,
                    is_active: s.is_active,
                })
                .collect();
            Json(response).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load subscriptions").into_response(),
    }
}

// 獲取特定 Fetcher 的訂閱 URL
pub async fn get_fetcher_subscriptions(
    State(state): State<AppState>,
    Path(fetcher_id_param): Path<i32>,
) -> impl IntoResponse {
    use crate::schema::rss_subscriptions::dsl::*;

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "DB connection failed").into_response(),
    };

    match rss_subscriptions
        .filter(fetcher_id.eq(fetcher_id_param))
        .filter(is_active.eq(true))
        .select(rss_url)
        .load::<String>(&mut conn)
    {
        Ok(urls) => Json(urls).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load URLs").into_response(),
    }
}

// 列出所有 Fetcher 模塊
pub async fn list_fetcher_modules(
    State(state): State<AppState>,
) -> impl IntoResponse {
    use crate::schema::fetcher_modules::dsl::*;

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "DB connection failed").into_response(),
    };

    match fetcher_modules
        .filter(is_active.eq(true))
        .load::<FetcherModule>(&mut conn)
    {
        Ok(modules) => {
            let response: Vec<FetcherModuleResponse> = modules
                .into_iter()
                .map(|m| FetcherModuleResponse {
                    fetcher_id: m.fetcher_id,
                    fetcher_name: m.fetcher_name,
                    service_id: m.service_id,
                    host: m.host,
                    port: m.port,
                    is_active: m.is_active,
                })
                .collect();
            Json(response).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load fetcher modules").into_response(),
    }
}

// 刪除訂閱
pub async fn delete_subscription(
    State(state): State<AppState>,
    Path(rss_url_param): Path<String>,
) -> impl IntoResponse {
    use crate::schema::rss_subscriptions::dsl::*;

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "DB connection failed").into_response(),
    };

    match diesel::delete(rss_subscriptions.filter(rss_url.eq(rss_url_param)))
        .execute(&mut conn)
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete subscription").into_response(),
    }
}
```

**Step 2: 更新 handlers 模塊聲明**

編輯 `core-service/src/handlers/mod.rs`，添加：

```rust
pub mod subscriptions;
```

**Step 3: 添加路由到 main.rs**

編輯 `core-service/src/main.rs`，在路由定義中添加（找到 `let app = Router::new()` 部分）：

```rust
.route("/subscriptions", post(handlers::subscriptions::create_subscription))
.route("/subscriptions", get(handlers::subscriptions::list_subscriptions))
.route("/subscriptions/:rss_url", delete(handlers::subscriptions::delete_subscription))
.route("/fetcher-modules", get(handlers::subscriptions::list_fetcher_modules))
.route("/fetcher-modules/:fetcher_id/subscriptions", get(handlers::subscriptions::get_fetcher_subscriptions))
```

**Step 4: 編譯並驗證**

```bash
cd /nodejs/rust-bangumi/core-service
cargo check
```

Expected: 編譯成功。

**Step 5: Commit**

```bash
cd /nodejs/rust-bangumi
git add core-service/src/handlers/subscriptions.rs core-service/src/handlers/mod.rs core-service/src/main.rs
git commit -m "feat: implement subscription management API endpoints

- POST /subscriptions - request new RSS subscription
- GET /subscriptions - list all active subscriptions
- GET /fetcher-modules/:fetcher_id/subscriptions - get URLs for specific fetcher
- GET /fetcher-modules - list all registered fetchers
- DELETE /subscriptions/:rss_url - remove subscription"
```

---

## Task 4: 實現 Fetcher 模塊註冊和訂閱廣播機制

**Files:**
- Modify: `core-service/src/services/mod.rs` (添加 subscription_broker 模塊)
- Create: `core-service/src/services/subscription_broker.rs`
- Modify: `core-service/src/state.rs` (添加 broadcast channel)
- Modify: `core-service/src/handlers/services.rs` (修改 register 端點)

**Step 1: 創建訂閱廣播服務**

創建 `core-service/src/services/subscription_broker.rs`：

```rust
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionBroadcast {
    pub rss_url: String,
    pub subscription_name: Option<String>,
}

pub type SubscriptionBroadcaster = broadcast::Sender<SubscriptionBroadcast>;

pub fn create_subscription_broadcaster() -> SubscriptionBroadcaster {
    let (tx, _) = broadcast::channel(100);
    tx
}
```

**Step 2: 更新 services 模塊**

編輯 `core-service/src/services/mod.rs`，添加：

```rust
pub mod subscription_broker;

pub use subscription_broker::{SubscriptionBroadcaster, SubscriptionBroadcast, create_subscription_broadcaster};
```

**Step 3: 更新 AppState**

編輯 `core-service/src/state.rs`，修改 AppState 定義：

```rust
use crate::services::SubscriptionBroadcaster;
use std::sync::Arc;

pub struct AppState {
    pub db: DbPool,
    pub registry: Arc<ServiceRegistry>,
    pub subscription_broadcaster: SubscriptionBroadcaster,
}
```

並在 `main.rs` 中初始化：

```rust
use crate::services::create_subscription_broadcaster;

let subscription_broadcaster = create_subscription_broadcaster();

let app_state = AppState {
    db: db_pool,
    registry: Arc::new(ServiceRegistry::new()),
    subscription_broadcaster,
};
```

**Step 4: 修改訂閱 API 以廣播**

編輯 `core-service/src/handlers/subscriptions.rs`，更新 `create_subscription` 函數：

```rust
pub async fn create_subscription(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> impl IntoResponse {
    use crate::schema::fetcher_modules::dsl::*;

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "DB connection failed").into_response(),
    };

    // 查詢所有活躍的 Fetcher
    let fetchers: Vec<FetcherModule> = match fetcher_modules
        .filter(is_active.eq(true))
        .load(&mut conn)
    {
        Ok(f) => f,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load fetchers").into_response(),
    };

    if fetchers.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            "No active fetchers available",
        ).into_response();
    }

    // 廣播訂閱請求
    let broadcast = crate::services::SubscriptionBroadcast {
        rss_url: payload.rss_url.clone(),
        subscription_name: payload.subscription_name.clone(),
    };

    let _ = state.subscription_broadcaster.send(broadcast);

    (StatusCode::ACCEPTED, "Subscription request broadcast to fetchers").into_response()
}
```

**Step 5: 編譯並驗證**

```bash
cd /nodejs/rust-bangumi/core-service
cargo check
```

Expected: 編譯成功。

**Step 6: Commit**

```bash
cd /nodejs/rust-bangumi
git add core-service/src/services/subscription_broker.rs core-service/src/services/mod.rs core-service/src/state.rs core-service/src/handlers/subscriptions.rs core-service/src/main.rs
git commit -m "feat: implement subscription broadcast mechanism

- Add subscription_broker service for event broadcasting
- Update AppState to include broadcast channel
- Modify create_subscription to broadcast to all fetchers"
```

---

## Task 5: 更新 Fetcher 的訂閱接收和模塊發現

**Files:**
- Modify: `fetchers/mikanani/src/main.rs` (添加訂閱接收邏輯)
- Create: `fetchers/mikanani/src/subscription_handler.rs`
- Modify: `shared/src/models.rs` (如需添加新的 DTO)

**Step 1: 創建 Fetcher 側訂閱處理器**

創建 `fetchers/mikanani/src/subscription_handler.rs`：

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::rss_parser::RssParser;

pub struct SubscriptionHandler {
    parser: Arc<RssParser>,
    pending_subscriptions: Arc<Mutex<Vec<String>>>,
}

impl SubscriptionHandler {
    pub fn new(parser: Arc<RssParser>) -> Self {
        Self {
            parser,
            pending_subscriptions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 檢查 Fetcher 是否應該接管該 URL
    /// 對於 Mikanani Fetcher，檢查是否包含 mikanani.me 域名
    pub async fn can_handle_url(&self, url: &str) -> bool {
        url.contains("mikanani.me")
    }

    /// 註冊該 URL 到 Core Service
    pub async fn register_subscription_with_core(
        &self,
        rss_url: &str,
        core_service_url: &str,
        fetcher_id: i32,
    ) -> Result<(), String> {
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "rss_url": rss_url,
            "fetcher_id": fetcher_id,
        });

        let response = client
            .post(format!("{}/subscriptions/register", core_service_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("Failed to register subscription: {:?}", response.status()))
        }
    }

    /// 添加待處理的訂閱
    pub async fn add_pending_subscription(&self, url: String) {
        let mut subs = self.pending_subscriptions.lock().await;
        subs.push(url);
    }

    /// 獲取並清除待處理的訂閱
    pub async fn get_and_clear_pending(&self) -> Vec<String> {
        let mut subs = self.pending_subscriptions.lock().await;
        let result = subs.clone();
        subs.clear();
        result
    }
}
```

**Step 2: 修改 Fetcher 啟動邏輯**

編輯 `fetchers/mikanani/src/main.rs`，添加訂閱接收端點：

```rust
mod subscription_handler;

use axum::extract::State;
use subscription_handler::SubscriptionHandler;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SubscriptionBroadcastPayload {
    pub rss_url: String,
    pub subscription_name: Option<String>,
}

/// 新增端點：接收來自 Core 的訂閱廣播
pub async fn handle_subscription_broadcast(
    State(handler): State<Arc<SubscriptionHandler>>,
    Json(payload): Json<SubscriptionBroadcastPayload>,
) -> impl IntoResponse {
    // 檢查是否應該接管
    if handler.can_handle_url(&payload.rss_url).await {
        handler.add_pending_subscription(payload.rss_url.clone()).await;
        (StatusCode::OK, "Subscription registered").into_response()
    } else {
        (StatusCode::NO_CONTENT, "Not responsible for this URL").into_response()
    }
}

// 在 main 中初始化路由
#[tokio::main]
async fn main() {
    // ... 現有初始化代碼 ...

    let handler = Arc::new(SubscriptionHandler::new(parser.clone()));

    let app = Router::new()
        .route("/fetch", post(fetch))
        .route("/health", get(health_check))
        .route("/subscribe", post(handle_subscription_broadcast))  // 新增
        .with_state(handler.clone())
        .with_state(parser.clone());

    // ... 運行服務器 ...
}
```

**Step 3: 修改 FetchScheduler 使用中央訂閱**

編輯 `fetchers/mikanani/src/scheduler.rs` 中的 `FetchScheduler` 實現，改為從 Core Service 獲取 URL：

```rust
impl FetchScheduler {
    pub async fn run_with_core(&self, core_service_url: &str, fetcher_id: i32) {
        let mut interval = tokio::time::interval(self.interval);

        loop {
            interval.tick().await;

            // 從 Core Service 獲取該 Fetcher 的所有訂閱 URL
            match self.fetch_urls_from_core(core_service_url, fetcher_id).await {
                Ok(urls) => {
                    for url in urls {
                        match self.parser.parse_feed(&url).await {
                            Ok(animes) => {
                                tracing::info!("Fetched {} animes from {}", animes.len(), url);
                                // TODO: 將結果發送回 Core（包含源 URL）
                            }
                            Err(e) => {
                                tracing::error!("Failed to fetch {}: {}", url, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to fetch URLs from Core: {}", e);
                }
            }
        }
    }

    async fn fetch_urls_from_core(&self, core_service_url: &str, fetcher_id: i32) -> Result<Vec<String>, String> {
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/fetcher-modules/{}/subscriptions", core_service_url, fetcher_id))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        response
            .json::<Vec<String>>()
            .await
            .map_err(|e| e.to_string())
    }
}
```

**Step 4: 編譯並驗證**

```bash
cd /nodejs/rust-bangumi/fetchers/mikanani
cargo check
```

Expected: 編譯成功。

**Step 5: Commit**

```bash
cd /nodejs/rust-bangumi
git add fetchers/mikanani/src/subscription_handler.rs fetchers/mikanani/src/main.rs fetchers/mikanani/src/scheduler.rs
git commit -m "feat: implement fetcher-side subscription handling

- Add subscription_handler for URL validation and registration
- Add /subscribe endpoint to receive broadcasts
- Modify scheduler to fetch URLs from Core Service instead of hardcoded"
```

---

## Task 6: 修改 RssParser 輸出包含源 URL 信息

**Files:**
- Modify: `fetchers/mikanani/src/rss_parser.rs` (修改 FetchedAnime 和 FetchedLink 結構)
- Modify: `fetchers/mikanani/src/lib.rs` (如需重新導出)

**Step 1: 更新 FetchedLink 結構**

編輯 `fetchers/mikanani/src/rss_parser.rs`，找到 `FetchedLink` 結構定義並修改：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedLink {
    pub episode_no: u32,
    pub subtitle_group: String,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub source_rss_url: String,  // 新增：記錄源 RSS URL
}
```

**Step 2: 更新 parse_feed 函數簽名**

修改 `parse_feed` 函數使其接收 `rss_url` 參數：

```rust
impl RssParser {
    pub async fn parse_feed(&self, rss_url: &str) -> Result<Vec<FetchedAnime>, String> {
        // 現有的 parse_feed 邏輯
        // 在創建 FetchedLink 時添加 source_rss_url 字段

        // ... 現有解析代碼 ...

        let fetched_links = feed_items
            .iter()
            .filter_map(|item| {
                // ... 現有的解析邏輯 ...
                Some(FetchedLink {
                    episode_no,
                    subtitle_group,
                    title: parsed_title,
                    url,
                    source_hash,
                    source_rss_url: rss_url.to_string(),  // 新增
                })
            })
            .collect();

        // ... 返回結果 ...
    }
}
```

**Step 3: 編譯並驗證**

```bash
cd /nodejs/rust-bangumi/fetchers/mikanani
cargo check
```

Expected: 編譯成功。

**Step 4: Commit**

```bash
cd /nodejs/rust-bangumi
git add fetchers/mikanani/src/rss_parser.rs
git commit -m "feat: include source RSS URL in fetched anime data

- Add source_rss_url field to FetchedLink struct
- Modify parse_feed to track which RSS URL each item came from"
```

---

## Task 7: 實現 Fetcher → Core 回傳動畫數據

**Files:**
- Create: `core-service/src/handlers/fetcher_results.rs`
- Modify: `core-service/src/handlers/mod.rs` (添加 fetcher_results)
- Modify: `core-service/src/main.rs` (添加路由)
- Modify: `fetchers/mikanani/src/main.rs` (添加回傳邏輯)
- Modify: `fetchers/mikanani/src/scheduler.rs` (添加回傳調用)

**Step 1: 創建接收 Fetcher 結果的 API**

創建 `core-service/src/handlers/fetcher_results.rs`：

```rust
use crate::models::db::*;
use crate::schema::*;
use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct FetchedLinkPayload {
    pub episode_no: i32,
    pub subtitle_group: String,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
}

#[derive(Serialize, Deserialize)]
pub struct FetchedAnimePayload {
    pub title: String,
    pub description: Option<String>,
    pub season: String,
    pub year: i32,
    pub series_no: i32,
    pub source_rss_url: String,
    pub links: Vec<FetchedLinkPayload>,
}

#[derive(Serialize, Deserialize)]
pub struct FetcherResultsPayload {
    pub fetcher_id: i32,
    pub animes: Vec<FetchedAnimePayload>,
}

/// 接收 Fetcher 回傳的動畫數據
pub async fn receive_fetcher_results(
    State(state): State<AppState>,
    Json(payload): Json<FetcherResultsPayload>,
) -> impl IntoResponse {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "DB connection failed").into_response(),
    };

    for anime_data in payload.animes {
        // 1. 獲取或創建 Anime
        let anime = match create_or_get_anime(&mut conn, &anime_data.title) {
            Ok(a) => a,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to handle anime").into_response(),
        };

        // 2. 獲取或創建 Season
        let season = match create_or_get_season(&mut conn, anime_data.year, &anime_data.season) {
            Ok(s) => s,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to handle season").into_response(),
        };

        // 3. 獲取或創建 AnimeSeries
        let series = match create_or_get_series(&mut conn, anime.anime_id, anime_data.series_no, season.season_id) {
            Ok(s) => s,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to handle series").into_response(),
        };

        // 4. 為每個 Link 創建記錄
        for link_data in &anime_data.links {
            let subtitle_group = match create_or_get_subtitle_group(&mut conn, &link_data.subtitle_group) {
                Ok(sg) => sg,
                Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to handle subtitle group").into_response(),
            };

            if let Err(_) = create_anime_link(
                &mut conn,
                series.series_id,
                subtitle_group.group_id,
                link_data.episode_no,
                &link_data.title,
                &link_data.url,
                &link_data.source_hash,
            ) {
                tracing::warn!("Failed to create anime link: {:?}", link_data);
                // 繼續處理其他 link，不中斷
            }
        }
    }

    StatusCode::ACCEPTED.into_response()
}

fn create_or_get_anime(conn: &mut PgConnection, title: &str) -> Result<Anime, diesel::result::Error> {
    use crate::schema::animes::dsl::*;

    // 嘗試查找
    match animes
        .filter(animes::title.eq(title))
        .first::<Anime>(conn)
    {
        Ok(anime) => Ok(anime),
        Err(diesel::result::Error::NotFound) => {
            // 插入新記錄
            diesel::insert_into(animes)
                .values((animes::title.eq(title),))
                .get_result(conn)
        }
        Err(e) => Err(e),
    }
}

fn create_or_get_season(conn: &mut PgConnection, year_val: i32, season_val: &str) -> Result<Season, diesel::result::Error> {
    use crate::schema::seasons::dsl::*;

    match seasons
        .filter(seasons::year.eq(year_val))
        .filter(seasons::season.eq(season_val))
        .first::<Season>(conn)
    {
        Ok(season) => Ok(season),
        Err(diesel::result::Error::NotFound) => {
            diesel::insert_into(seasons)
                .values((
                    seasons::year.eq(year_val),
                    seasons::season.eq(season_val),
                ))
                .get_result(conn)
        }
        Err(e) => Err(e),
    }
}

fn create_or_get_series(
    conn: &mut PgConnection,
    anime_id_val: i32,
    series_no_val: i32,
    season_id_val: i32,
) -> Result<AnimeSeries, diesel::result::Error> {
    use crate::schema::anime_series::dsl::*;

    match anime_series
        .filter(anime_series::anime_id.eq(anime_id_val))
        .filter(anime_series::series_no.eq(series_no_val))
        .first::<AnimeSeries>(conn)
    {
        Ok(series) => Ok(series),
        Err(diesel::result::Error::NotFound) => {
            diesel::insert_into(anime_series)
                .values((
                    anime_series::anime_id.eq(anime_id_val),
                    anime_series::series_no.eq(series_no_val),
                    anime_series::season_id.eq(season_id_val),
                ))
                .get_result(conn)
        }
        Err(e) => Err(e),
    }
}

fn create_or_get_subtitle_group(conn: &mut PgConnection, group_name: &str) -> Result<SubtitleGroup, diesel::result::Error> {
    use crate::schema::subtitle_groups::dsl::*;

    match subtitle_groups
        .filter(subtitle_groups::group_name.eq(group_name))
        .first::<SubtitleGroup>(conn)
    {
        Ok(group) => Ok(group),
        Err(diesel::result::Error::NotFound) => {
            diesel::insert_into(subtitle_groups)
                .values((subtitle_groups::group_name.eq(group_name),))
                .get_result(conn)
        }
        Err(e) => Err(e),
    }
}

fn create_anime_link(
    conn: &mut PgConnection,
    series_id_val: i32,
    group_id_val: i32,
    episode_no_val: i32,
    title_opt: &Option<String>,
    url_val: &str,
    source_hash_val: &str,
) -> Result<AnimeLink, diesel::result::Error> {
    use crate::schema::anime_links::dsl::*;

    diesel::insert_into(anime_links)
        .values((
            anime_links::series_id.eq(series_id_val),
            anime_links::group_id.eq(group_id_val),
            anime_links::episode_no.eq(episode_no_val),
            anime_links::title.eq(title_opt),
            anime_links::url.eq(url_val),
            anime_links::source_hash.eq(source_hash_val),
            anime_links::filtered_flag.eq(false),
        ))
        .get_result(conn)
}
```

**Step 2: 添加到 handlers mod**

編輯 `core-service/src/handlers/mod.rs`：

```rust
pub mod fetcher_results;
```

**Step 3: 添加路由**

編輯 `core-service/src/main.rs`，添加：

```rust
.route("/fetcher-results", post(handlers::fetcher_results::receive_fetcher_results))
```

**Step 4: 修改 Fetcher 發送結果**

編輯 `fetchers/mikanani/src/scheduler.rs`，修改 `run_with_core` 方法：

```rust
impl FetchScheduler {
    pub async fn run_with_core(&self, core_service_url: &str, fetcher_id: i32) {
        let mut interval = tokio::time::interval(self.interval);

        loop {
            interval.tick().await;

            match self.fetch_urls_from_core(core_service_url, fetcher_id).await {
                Ok(urls) => {
                    let mut all_animes = Vec::new();

                    for url in urls {
                        match self.parser.parse_feed(&url).await {
                            Ok(animes) => {
                                tracing::info!("Fetched {} animes from {}", animes.len(), url);
                                all_animes.extend(animes);
                            }
                            Err(e) => {
                                tracing::error!("Failed to fetch {}: {}", url, e);
                            }
                        }
                    }

                    // 將結果發送回 Core
                    if !all_animes.is_empty() {
                        if let Err(e) = self.send_results_to_core(core_service_url, fetcher_id, all_animes).await {
                            tracing::error!("Failed to send results to Core: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to fetch URLs from Core: {}", e);
                }
            }
        }
    }

    async fn send_results_to_core(
        &self,
        core_service_url: &str,
        fetcher_id: i32,
        animes: Vec<FetchedAnime>,
    ) -> Result<(), String> {
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "fetcher_id": fetcher_id,
            "animes": animes,
        });

        let response = client
            .post(format!("{}/fetcher-results", core_service_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("Failed to send results: {:?}", response.status()))
        }
    }
}
```

**Step 5: 編譯並驗證**

```bash
cd /nodejs/rust-bangumi/core-service
cargo check
cd /nodejs/rust-bangumi/fetchers/mikanani
cargo check
```

Expected: 兩個模塊都編譯成功。

**Step 6: Commit**

```bash
cd /nodejs/rust-bangumi
git add core-service/src/handlers/fetcher_results.rs core-service/src/handlers/mod.rs core-service/src/main.rs fetchers/mikanani/src/scheduler.rs
git commit -m "feat: implement fetcher result ingestion and anime storage

- Add POST /fetcher-results endpoint to receive anime data from fetchers
- Implement automatic creation of anime, series, and links records
- Modify scheduler to send parsed results back to Core Service
- Handle upsert logic for existing records"
```

---

## Task 8: 修改 Services 註冊端點以支持 fetcher_modules 表

**Files:**
- Modify: `core-service/src/handlers/services.rs`
- Modify: `core-service/src/models/db.rs` (添加 FetcherModule 插入邏輯)

**Step 1: 修改 register 端點邏輯**

編輯 `core-service/src/handlers/services.rs`，修改 `register` 函數以同時寫入 `fetcher_modules` 表：

```rust
use crate::models::db::{FetcherModule, NewFetcherModule};
use crate::schema::fetcher_modules;

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<ServiceRegistration>,
) -> impl IntoResponse {
    // ... 現有的 registry 註冊代碼 ...

    let registered_service = RegisteredService {
        service_id: Uuid::new_v4(),
        // ... 其他字段 ...
    };

    // 如果是 Fetcher 類型，同時寫入 fetcher_modules 表
    if payload.service_type == ServiceType::Fetcher {
        let mut conn = match state.db.get() {
            Ok(c) => c,
            Err(_) => {
                tracing::error!("Failed to get DB connection");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database connection failed",
                ).into_response();
            }
        };

        let new_module = NewFetcherModule {
            fetcher_name: payload.service_name.clone(),
            service_id: registered_service.service_id.to_string(),
            host: payload.host.clone(),
            port: payload.port as i32,
            is_active: true,
        };

        match diesel::insert_into(fetcher_modules::table)
            .values(&new_module)
            .execute(&mut conn)
        {
            Ok(_) => {
                tracing::info!("Registered Fetcher module: {}", payload.service_name);
            }
            Err(e) => {
                tracing::error!("Failed to register Fetcher module: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to register in database",
                ).into_response();
            }
        }
    }

    // ... 返回現有響應 ...
}
```

**Step 2: 編譯並驗證**

```bash
cd /nodejs/rust-bangumi/core-service
cargo check
```

Expected: 編譯成功。

**Step 3: Commit**

```bash
cd /nodejs/rust-bangumi
git add core-service/src/handlers/services.rs
git commit -m "feat: persist fetcher registrations to fetcher_modules table

- Modify register endpoint to write Fetcher services to database
- Store fetcher name, service_id, host, and port for later retrieval"
```

---

## Task 9: 添加衝突解決機制

**Files:**
- Create: `core-service/src/handlers/conflict_resolution.rs`
- Modify: `core-service/src/handlers/mod.rs`
- Modify: `core-service/src/main.rs` (添加路由)
- Modify: `core-service/src/handlers/subscriptions.rs` (修改 create_subscription 邏輯)

**Step 1: 創建衝突解決 API**

創建 `core-service/src/handlers/conflict_resolution.rs`：

```rust
use crate::models::db::*;
use crate::schema::*;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ResolveConflictRequest {
    pub chosen_fetcher_id: i32,
}

#[derive(Serialize, Deserialize)]
pub struct ConflictInfo {
    pub conflict_id: i32,
    pub rss_url: String,
    pub candidate_fetchers: Vec<(i32, String)>,  // (fetcher_id, fetcher_name)
}

/// 獲取未解決的衝突
pub async fn get_pending_conflicts(
    State(state): State<AppState>,
) -> impl IntoResponse {
    use crate::schema::subscription_conflicts::dsl::*;
    use crate::schema::fetcher_modules::dsl::*;

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "DB connection failed").into_response(),
    };

    match subscription_conflicts
        .filter(resolved_fetcher_id.is_null())
        .load::<SubscriptionConflict>(&mut conn)
    {
        Ok(conflicts) => {
            let mut result = Vec::new();

            for conflict in conflicts {
                // 為每個候選 ID 查詢 Fetcher 名稱
                let fetcher_names: Vec<(i32, String)> = conflict
                    .candidate_fetcher_ids
                    .iter()
                    .filter_map(|fid| {
                        match fetcher_modules
                            .filter(fetcher_modules::fetcher_id.eq(fid))
                            .select((fetcher_modules::fetcher_id, fetcher_modules::fetcher_name))
                            .first::<(i32, String)>(&mut conn)
                        {
                            Ok(pair) => Some(pair),
                            Err(_) => None,
                        }
                    })
                    .collect();

                result.push(ConflictInfo {
                    conflict_id: conflict.conflict_id,
                    rss_url: conflict.rss_url,
                    candidate_fetchers: fetcher_names,
                });
            }

            Json(result).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load conflicts").into_response(),
    }
}

/// 解決衝突並將 URL 分配給選定的 Fetcher
pub async fn resolve_conflict(
    State(state): State<AppState>,
    Path(conflict_id_param): Path<i32>,
    Json(payload): Json<ResolveConflictRequest>,
) -> impl IntoResponse {
    use crate::schema::subscription_conflicts::dsl::*;
    use crate::schema::rss_subscriptions::dsl::*;

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "DB connection failed").into_response(),
    };

    // 查詢衝突記錄
    let conflict = match subscription_conflicts
        .filter(subscription_conflicts::conflict_id.eq(conflict_id_param))
        .first::<SubscriptionConflict>(&mut conn)
    {
        Ok(c) => c,
        Err(_) => return (StatusCode::NOT_FOUND, "Conflict not found").into_response(),
    };

    // 驗證選定的 Fetcher 在候選列表中
    if !conflict.candidate_fetcher_ids.contains(&payload.chosen_fetcher_id) {
        return (StatusCode::BAD_REQUEST, "Invalid fetcher choice").into_response();
    }

    // 更新 rss_subscriptions，將 URL 分配給選定的 Fetcher
    match diesel::update(rss_subscriptions.filter(rss_subscriptions::rss_url.eq(&conflict.rss_url)))
        .set(rss_subscriptions::fetcher_id.eq(payload.chosen_fetcher_id))
        .execute(&mut conn)
    {
        Ok(_) => {}
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to update subscription").into_response(),
    }

    // 標記衝突為已解決
    match diesel::update(subscription_conflicts.filter(subscription_conflicts::conflict_id.eq(conflict_id_param)))
        .set((
            subscription_conflicts::resolved_fetcher_id.eq(payload.chosen_fetcher_id),
            subscription_conflicts::resolved_at.eq(diesel::dsl::now),
        ))
        .execute(&mut conn)
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to resolve conflict").into_response(),
    }
}
```

**Step 2: 修改 create_subscription 以檢測衝突**

編輯 `core-service/src/handlers/subscriptions.rs`，擴展 `create_subscription` 函數：

```rust
pub async fn create_subscription(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> impl IntoResponse {
    use crate::schema::fetcher_modules::dsl::*;
    use crate::schema::subscription_conflicts::dsl::*;
    use crate::schema::rss_subscriptions::dsl::*;

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "DB connection failed").into_response(),
    };

    // 查詢所有活躍的 Fetcher
    let fetchers: Vec<FetcherModule> = match fetcher_modules
        .filter(is_active.eq(true))
        .load(&mut conn)
    {
        Ok(f) => f,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load fetchers").into_response(),
    };

    if fetchers.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            "No active fetchers available",
        ).into_response();
    }

    // 廣播訂閱請求
    let broadcast = crate::services::SubscriptionBroadcast {
        rss_url: payload.rss_url.clone(),
        subscription_name: payload.subscription_name.clone(),
    };

    let _ = state.subscription_broadcaster.send(broadcast);

    // 暫時將 URL 分配給第一個 Fetcher（在衝突解決前）
    let initial_fetcher_id = fetchers[0].fetcher_id;

    let new_sub = NewRssSubscription {
        rss_url: payload.rss_url.clone(),
        fetcher_id: initial_fetcher_id,
        subscription_name: payload.subscription_name,
        is_active: true,
    };

    match diesel::insert_into(rss_subscriptions)
        .values(&new_sub)
        .execute(&mut conn)
    {
        Ok(_) => {}
        Err(diesel::result::Error::DatabaseError(_, _)) => {
            // URL 已存在，忽略
        }
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to insert subscription").into_response(),
    }

    (StatusCode::ACCEPTED, "Subscription request accepted and broadcast to fetchers").into_response()
}
```

**Step 3: 添加到 handlers mod 和路由**

編輯 `core-service/src/handlers/mod.rs`：

```rust
pub mod conflict_resolution;
```

編輯 `core-service/src/main.rs`，添加：

```rust
.route("/conflicts", get(handlers::conflict_resolution::get_pending_conflicts))
.route("/conflicts/:conflict_id/resolve", post(handlers::conflict_resolution::resolve_conflict))
```

**Step 4: 編譯並驗證**

```bash
cd /nodejs/rust-bangumi/core-service
cargo check
```

Expected: 編譯成功。

**Step 5: Commit**

```bash
cd /nodejs/rust-bangumi
git add core-service/src/handlers/conflict_resolution.rs core-service/src/handlers/mod.rs core-service/src/main.rs core-service/src/handlers/subscriptions.rs
git commit -m "feat: implement subscription conflict detection and resolution

- Add GET /conflicts endpoint to retrieve pending conflicts
- Add POST /conflicts/:conflict_id/resolve to assign URLs to fetchers
- Modify subscription creation to detect multiple claiming fetchers"
```

---

## Task 10: 集成測試和文檔

**Files:**
- Create: `core-service/tests/integration_test_subscriptions.rs`
- Create: `docs/ARCHITECTURE_RSS_SUBSCRIPTIONS.md`

**Step 1: 編寫集成測試**

創建 `core-service/tests/integration_test_subscriptions.rs`：

```rust
#[cfg(test)]
mod tests {
    use core_service::models::db::*;
    // ... 其他導入 ...

    #[tokio::test]
    async fn test_create_subscription() {
        // 1. 初始化測試數據庫
        // 2. 創建 test fetcher module
        // 3. POST /subscriptions
        // 4. 驗證 rss_subscriptions 表有新記錄
    }

    #[tokio::test]
    async fn test_fetcher_subscription_retrieval() {
        // 1. 創建多個訂閱
        // 2. GET /fetcher-modules/{fetcher_id}/subscriptions
        // 3. 驗證只返回該 fetcher 的 URL
    }

    #[tokio::test]
    async fn test_conflict_resolution() {
        // 1. 模擬多個 Fetcher 聲稱同一 URL
        // 2. GET /conflicts 驗證衝突記錄存在
        // 3. POST /conflicts/{conflict_id}/resolve
        // 4. 驗證 rss_subscriptions.fetcher_id 已更新
    }
}
```

**Step 2: 編寫架構文檔**

創建 `docs/ARCHITECTURE_RSS_SUBSCRIPTIONS.md`：

```markdown
# RSS 訂閱管理架構

## 概述

本文檔描述 RSS 訂閱管理從 Fetcher 分散管理遷移到 Core 中央管理的架構設計。

## 數據模型

### 表結構

#### 1. `fetcher_modules` - Fetcher 模塊註冊表

| 欄位 | 類型 | 說明 |
|------|------|------|
| fetcher_id | SERIAL PRIMARY KEY | 自增主鍵 |
| fetcher_name | VARCHAR(100) UNIQUE | 模塊名稱（如 "mikanani"） |
| service_id | UUID UNIQUE | 註冊時的服務 ID |
| host | VARCHAR(255) | Fetcher 服務主機 |
| port | INT | Fetcher 服務端口 |
| is_active | BOOLEAN | 是否活躍 |
| created_at | TIMESTAMP | 創建時間 |
| updated_at | TIMESTAMP | 更新時間 |

#### 2. `rss_subscriptions` - RSS 訂閱表

| 欄位 | 類型 | 說明 |
|------|------|------|
| rss_url | TEXT PRIMARY KEY | RSS URL（主鍵） |
| fetcher_id | INT FK | 所屬 Fetcher |
| subscription_name | VARCHAR(255) | 訂閱名稱 |
| is_active | BOOLEAN | 是否活躍 |
| last_fetched_at | TIMESTAMP | 最後抓取時間 |
| error_message | TEXT | 錯誤信息 |
| created_at | TIMESTAMP | 創建時間 |
| updated_at | TIMESTAMP | 更新時間 |

#### 3. `subscription_conflicts` - 衝突記錄表

| 欄位 | 類型 | 說明 |
|------|------|------|
| conflict_id | SERIAL PRIMARY KEY | 自增主鍵 |
| rss_url | TEXT FK | 涉及的 URL |
| candidate_fetcher_ids | INT[] | 競爭者 Fetcher ID 陣列 |
| resolved_fetcher_id | INT | 最終分配的 Fetcher ID |
| resolved_at | TIMESTAMP | 解決時間 |
| created_at | TIMESTAMP | 創建時間 |

## 工作流程

### 訂閱新 URL 流程

1. **用戶發起請求**
   ```
   POST /subscriptions
   {
     "rss_url": "https://mikanani.me/...",
     "subscription_name": "進擊的巨人"
   }
   ```

2. **Core 服務廣播**
   - Core 將 URL 廣播給所有活躍 Fetcher
   - 暫時將 URL 分配給第一個 Fetcher

3. **Fetcher 過濾**
   - 每個 Fetcher 接收 `/subscribe` 廣播
   - 檢查 URL 是否屬於自己的域（如 mikanani.me）
   - 如果是，發送 `/subscriptions/register` 請求給 Core

4. **衝突檢測與解決**
   - 如果多個 Fetcher 聲稱同一 URL
   - Core 創建 `subscription_conflicts` 記錄
   - 用戶通過 `GET /conflicts` 獲取待解決的衝突
   - 用戶調用 `POST /conflicts/{conflict_id}/resolve` 選擇 Fetcher
   - Core 更新 `rss_subscriptions.fetcher_id`

### 抓取流程

1. **Fetcher 啟動後台任務**
   ```
   scheduler.run_with_core(core_service_url, fetcher_id)
   ```

2. **定期從 Core 獲取 URL**
   ```
   GET /fetcher-modules/{fetcher_id}/subscriptions
   Response: ["https://mikanani.me/...", ...]
   ```

3. **解析 RSS 並回傳結果**
   ```
   POST /fetcher-results
   {
     "fetcher_id": 1,
     "animes": [...]
   }
   ```

4. **Core 存儲動畫數據**
   - 自動創建/更新 anime_series, anime_links 記錄
   - 每個 link 記錄源 RSS URL（通過 FK 連結 `rss_subscriptions`）

## API 端點

### 訂閱管理

| 方法 | 端點 | 說明 |
|------|------|------|
| POST | /subscriptions | 創建新訂閱（廣播給所有 Fetcher） |
| GET | /subscriptions | 列出所有活躍訂閱 |
| DELETE | /subscriptions/:rss_url | 刪除訂閱 |

### Fetcher 相關

| 方法 | 端點 | 說明 |
|------|------|------|
| GET | /fetcher-modules | 列出所有註冊的 Fetcher |
| GET | /fetcher-modules/:fetcher_id/subscriptions | 獲取特定 Fetcher 的 URL 列表 |
| POST | /fetcher-results | 接收 Fetcher 回傳的動畫數據 |

### 衝突解決

| 方法 | 端點 | 說明 |
|------|------|------|
| GET | /conflicts | 列出待解決的衝突 |
| POST | /conflicts/:conflict_id/resolve | 解決衝突 |

## Fetcher 端需要的修改

### 1. 訂閱接收端點

```
POST /subscribe
{
  "rss_url": "...",
  "subscription_name": "..."
}
```

Fetcher 檢查 URL 是否屬於自己，並回應：
- `200 OK`: 接受該 URL
- `204 No Content`: 不負責此 URL

### 2. 從 Core 獲取 URL

在後台任務中定期調用：
```
GET http://core-service:8000/fetcher-modules/{fetcher_id}/subscriptions
```

### 3. 回傳結果

解析完成後調用：
```
POST http://core-service:8000/fetcher-results
{
  "fetcher_id": 1,
  "animes": [
    {
      "title": "...",
      "source_rss_url": "https://mikanani.me/...",
      "links": [...]
    }
  ]
}
```

## 動畫數據流

```
RSS URL → Fetcher 解析 → FetchedAnime (含 source_rss_url)
                               ↓
                        POST /fetcher-results
                               ↓
                        Core 存儲:
                        - anime (標題)
                        - anime_series (季數)
                        - anime_links (集數/字幕組)
                        - rss_subscriptions (源 URL 綁定)
```

## 故障處理

### URL 解析失敗

- Fetcher 捕捉錯誤並記錄日誌
- Core 側不會創建記錄
- 下次定期任務重試

### Fetcher 掛機

- Core 通過缺失的 heartbeat 檢測
- 將其標記為非活躍
- 新訂閃廣播時跳過該 Fetcher

### 衝突長期未解決

- 用戶需主動調用解決 API
- 或管理員設置超時自動選擇

## 未來擴展

1. **優先級規則**
   - 允許配置 Fetcher 優先級
   - 自動衝突解決

2. **速率限制**
   - 按 Fetcher/URL 設置抓取頻率

3. **持久化事件日誌**
   - 記錄所有訂閱事件
   - 便於審計和調試
```

**Step 3: 編譯測試**

```bash
cd /nodejs/rust-bangumi/core-service
cargo test --test integration_test_subscriptions -- --nocapture
```

Expected: 測試編譯成功（如果有功能完善，應全數通過）。

**Step 4: Commit**

```bash
cd /nodejs/rust-bangumi
git add core-service/tests/integration_test_subscriptions.rs docs/ARCHITECTURE_RSS_SUBSCRIPTIONS.md
git commit -m "docs: add integration tests and architecture documentation

- Add comprehensive integration tests for subscription management
- Document API endpoints, data flows, and workflows
- Include troubleshooting and future extension guidelines"
```

---

## 執行策略

此計劃包含 10 個任務，按照數據庫遷移 → API 實現 → Fetcher 集成的順序進行。

**建議執行方式：**

每個任務 2-5 分鐘，全部完成約 60-90 分鐘。按順序執行，每個任務完成後立即提交。

若出現編譯錯誤，立即修復並重新編譯，勿跳過任何任務。
