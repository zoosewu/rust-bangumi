# Webhook Feature Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在下載完成並觸發 Viewer 同步時，同時觸發使用者自訂的 webhook（發送 HTTP POST 請求到指定 URL，支援動態模板參數）。

**Architecture:** 新增 `webhooks` 資料庫表儲存 webhook 設定；新增 `WebhookService` 負責模板渲染與 HTTP 觸發；在 `SyncService::notify_viewer()` 成功後，以 fire-and-forget 方式非同步觸發所有已啟用的 webhook。

**Tech Stack:** Rust, Axum, Diesel 2.1, PostgreSQL, reqwest, tokio

---

## 支援的模板變數

webhook 的 `payload_template` 欄位中使用 `{{variable}}` 語法，支援以下變數：

| 變數 | 說明 |
|------|------|
| `{{download_id}}` | 下載記錄 ID |
| `{{anime_id}}` | 動畫系列 ID |
| `{{anime_title}}` | 動畫標題 |
| `{{episode_no}}` | 集數 |
| `{{series_no}}` | 第幾季 |
| `{{subtitle_group}}` | 字幕組名稱 |
| `{{video_path}}` | 影片檔案路徑 |

**模板範例：**
```json
{"title": "{{anime_title}} EP{{episode_no}}", "path": "{{video_path}}"}
```

---

## 檔案變動清單

### 新增
- `core-service/migrations/2026-03-13-000000-add-webhooks/up.sql`
- `core-service/migrations/2026-03-13-000000-add-webhooks/down.sql`
- `core-service/src/db/repository/webhook.rs`
- `core-service/src/services/webhook_service.rs`
- `core-service/src/handlers/webhooks.rs`

### 修改
- `core-service/src/schema.rs` — Diesel 自動生成（`diesel migration run` 後）
- `core-service/src/models/db.rs` — 新增 `Webhook`、`NewWebhook` 模型
- `core-service/src/db/repository/mod.rs` — 導出 webhook repository
- `core-service/src/db/mod.rs` — 導出 `WebhookRepository`、`DieselWebhookRepository`
- `core-service/src/services/mod.rs` — 導出 `WebhookService`
- `core-service/src/services/sync_service.rs` — 接收 `Arc<WebhookService>`，觸發 webhook
- `core-service/src/state.rs` — 新增 `webhook_service: Arc<WebhookService>`
- `core-service/src/handlers/mod.rs` — 新增 `pub mod webhooks`
- `core-service/src/main.rs` — 新增 webhook CRUD 路由

---

## Chunk 1: 資料庫層

### Task 1: 建立 Diesel Migration

**Files:**
- Create: `core-service/migrations/2026-03-13-000000-add-webhooks/up.sql`
- Create: `core-service/migrations/2026-03-13-000000-add-webhooks/down.sql`

- [ ] **Step 1: 建立 migration 目錄與 up.sql**

```bash
mkdir -p core-service/migrations/2026-03-13-000000-add-webhooks
```

`up.sql` 內容：
```sql
CREATE TABLE webhooks (
    webhook_id   SERIAL PRIMARY KEY,
    name         VARCHAR(255) NOT NULL,
    url          TEXT NOT NULL,
    payload_template TEXT NOT NULL DEFAULT '{"download_id": {{download_id}}, "anime_title": "{{anime_title}}", "episode_no": {{episode_no}}}',
    is_active    BOOLEAN NOT NULL DEFAULT TRUE,
    created_at   TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMP NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 2: 建立 down.sql**

```sql
DROP TABLE IF EXISTS webhooks;
```

- [ ] **Step 3: 執行 migration，更新 schema.rs**

```bash
cd core-service && diesel migration run
```

預期輸出：`Running migration 2026-03-13-000000-add-webhooks`

驗證 `src/schema.rs` 中出現 `webhooks` 表定義。

- [ ] **Step 4: 驗證 redo 可行**

```bash
diesel migration redo
```

預期輸出：先 down 再 up，無錯誤。

---

### Task 2: 新增 Diesel 模型

**Files:**
- Modify: `core-service/src/models/db.rs`

- [ ] **Step 1: 在 `models/db.rs` 末尾新增 Webhook 模型**

```rust
// ============ Webhook 模型 ============
#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = crate::schema::webhooks)]
#[diesel(primary_key(webhook_id))]
pub struct Webhook {
    pub webhook_id: i32,
    pub name: String,
    pub url: String,
    pub payload_template: String,
    pub is_active: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::webhooks)]
pub struct NewWebhook {
    pub name: String,
    pub url: String,
    pub payload_template: String,
    pub is_active: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}
```

- [ ] **Step 2: 在 `models/mod.rs` 確認 Webhook 有被 re-export**

`src/models/mod.rs` 目前只有：
```rust
pub mod db;
```

在 `db.rs` 中已直接定義，`models/mod.rs` 不需要修改（其他模型也是直接在 db.rs 定義，使用時透過 `crate::models::Webhook`）。

- [ ] **Step 3: 更新 `lib.rs` 或主模塊使 Webhook 可被導入**

確認 `src/main.rs` 有 `mod models;`（已存在），Webhook 透過 `crate::models::Webhook` 訪問。

- [ ] **Step 4: 編譯確認**

```bash
cd core-service && cargo check 2>&1 | head -30
```

預期：無錯誤（只有關於 unused 的 warning 是可接受的）。

---

### Task 3: 建立 Webhook Repository

**Files:**
- Create: `core-service/src/db/repository/webhook.rs`
- Modify: `core-service/src/db/repository/mod.rs`
- Modify: `core-service/src/db/mod.rs`

- [ ] **Step 1: 建立 `db/repository/webhook.rs`**

```rust
use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::{NewWebhook, Webhook};
use crate::schema::webhooks;

#[async_trait]
pub trait WebhookRepository: Send + Sync {
    async fn find_all(&self) -> Result<Vec<Webhook>, RepositoryError>;
    async fn find_active(&self) -> Result<Vec<Webhook>, RepositoryError>;
    async fn find_by_id(&self, id: i32) -> Result<Option<Webhook>, RepositoryError>;
    async fn create(&self, new_webhook: NewWebhook) -> Result<Webhook, RepositoryError>;
    async fn update(&self, webhook: Webhook) -> Result<Webhook, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
}

pub struct DieselWebhookRepository {
    pool: DbPool,
}

impl DieselWebhookRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WebhookRepository for DieselWebhookRepository {
    async fn find_all(&self) -> Result<Vec<Webhook>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            webhooks::table
                .order(webhooks::webhook_id.asc())
                .load::<Webhook>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_active(&self) -> Result<Vec<Webhook>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            webhooks::table
                .filter(webhooks::is_active.eq(true))
                .order(webhooks::webhook_id.asc())
                .load::<Webhook>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<Webhook>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            webhooks::table
                .find(id)
                .first::<Webhook>(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn create(&self, new_webhook: NewWebhook) -> Result<Webhook, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::insert_into(webhooks::table)
                .values(&new_webhook)
                .get_result::<Webhook>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn update(&self, webhook: Webhook) -> Result<Webhook, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();
            diesel::update(webhooks::table.find(webhook.webhook_id))
                .set((
                    webhooks::name.eq(&webhook.name),
                    webhooks::url.eq(&webhook.url),
                    webhooks::payload_template.eq(&webhook.payload_template),
                    webhooks::is_active.eq(webhook.is_active),
                    webhooks::updated_at.eq(now),
                ))
                .get_result::<Webhook>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let count = diesel::delete(webhooks::table.find(id))
                .execute(&mut conn)
                .map_err(RepositoryError::from)?;
            Ok(count > 0)
        })
        .await?
    }
}
```

- [ ] **Step 2: 更新 `db/repository/mod.rs`**

在現有 `mod.rs` 中新增：
```rust
pub mod webhook;
pub use webhook::{DieselWebhookRepository, WebhookRepository};
```

- [ ] **Step 3: 更新 `db/mod.rs`**

在 `pub use repository::{...}` 的列表末尾加入：
```rust
DieselWebhookRepository, WebhookRepository,
```

- [ ] **Step 4: 編譯確認**

```bash
cd core-service && cargo check 2>&1 | head -30
```

預期：無錯誤。

---

## Chunk 2: 服務層

### Task 4: 建立 WebhookService

**Files:**
- Create: `core-service/src/services/webhook_service.rs`
- Modify: `core-service/src/services/mod.rs`

- [ ] **Step 1: 建立 `services/webhook_service.rs`**

```rust
use crate::db::DbPool;
use crate::schema::webhooks;
use crate::models::Webhook;
use diesel::prelude::*;

/// 模板渲染所需的動畫下載上下文
pub struct WebhookContext {
    pub download_id: i32,
    pub anime_id: i32,
    pub anime_title: String,
    pub episode_no: i32,
    pub series_no: i32,
    pub subtitle_group: String,
    pub video_path: String,
}

pub struct WebhookService {
    db_pool: DbPool,
    http_client: reqwest::Client,
}

impl WebhookService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap(),
        }
    }

    /// 載入所有啟用的 webhook，逐一渲染模板並發送（fire-and-forget）。
    /// 此方法應以 tokio::spawn 包裹，不阻塞主流程。
    pub async fn fire(&self, ctx: WebhookContext) {
        let webhooks = match self.load_active_webhooks() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to load webhooks: {}", e);
                return;
            }
        };

        for webhook in webhooks {
            let payload = render_template(&webhook.payload_template, &ctx);
            let url = webhook.url.clone();
            let client = self.http_client.clone();
            let webhook_id = webhook.webhook_id;

            tokio::spawn(async move {
                match client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .body(payload)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        tracing::info!(
                            "Webhook {} fired to {}: status {}",
                            webhook_id,
                            url,
                            resp.status()
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Webhook {} failed to fire to {}: {}",
                            webhook_id,
                            url,
                            e
                        );
                    }
                }
            });
        }
    }

    fn load_active_webhooks(&self) -> Result<Vec<Webhook>, String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;
        webhooks::table
            .filter(webhooks::is_active.eq(true))
            .load::<Webhook>(&mut conn)
            .map_err(|e| e.to_string())
    }
}

/// 將 `{{variable}}` 佔位符替換為對應值。
/// 數字型變數直接插入，字串型變數不加引號（由模板作者自行決定格式）。
pub fn render_template(template: &str, ctx: &WebhookContext) -> String {
    template
        .replace("{{download_id}}", &ctx.download_id.to_string())
        .replace("{{anime_id}}", &ctx.anime_id.to_string())
        .replace("{{anime_title}}", &ctx.anime_title)
        .replace("{{episode_no}}", &ctx.episode_no.to_string())
        .replace("{{series_no}}", &ctx.series_no.to_string())
        .replace("{{subtitle_group}}", &ctx.subtitle_group)
        .replace("{{video_path}}", &ctx.video_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> WebhookContext {
        WebhookContext {
            download_id: 42,
            anime_id: 7,
            anime_title: "進擊的巨人".to_string(),
            episode_no: 3,
            series_no: 2,
            subtitle_group: "字幕組A".to_string(),
            video_path: "/downloads/ep03.mkv".to_string(),
        }
    }

    #[test]
    fn renders_all_variables() {
        let template = r#"{"id":{{download_id}},"title":"{{anime_title}}","ep":{{episode_no}}}"#;
        let result = render_template(template, &make_ctx());
        assert_eq!(result, r#"{"id":42,"title":"進擊的巨人","ep":3}"#);
    }

    #[test]
    fn renders_series_no_and_subtitle_group() {
        let template = "S{{series_no}}E{{episode_no}} - {{subtitle_group}}";
        let result = render_template(template, &make_ctx());
        assert_eq!(result, "S2E3 - 字幕組A");
    }

    #[test]
    fn renders_video_path() {
        let template = "{{video_path}}";
        let result = render_template(template, &make_ctx());
        assert_eq!(result, "/downloads/ep03.mkv");
    }

    #[test]
    fn unknown_placeholder_left_intact() {
        let template = "{{unknown}} {{download_id}}";
        let result = render_template(template, &make_ctx());
        assert_eq!(result, "{{unknown}} 42");
    }
}
```

- [ ] **Step 2: 更新 `services/mod.rs`**

新增：
```rust
pub mod webhook_service;
pub use webhook_service::{WebhookContext, WebhookService};
```

- [ ] **Step 3: 執行單元測試確認**

```bash
cd core-service && cargo test webhook_service 2>&1
```

預期：4 個測試全部 PASS。

- [ ] **Step 4: 整體編譯確認**

```bash
cd core-service && cargo check 2>&1 | head -30
```

---

### Task 5: 將 WebhookService 整合進 SyncService

**Files:**
- Modify: `core-service/src/services/sync_service.rs`
- Modify: `core-service/src/state.rs`

- [ ] **Step 1: 修改 `sync_service.rs` 的 struct 與建構子**

在 `use` 區塊新增：
```rust
use crate::services::webhook_service::{WebhookContext, WebhookService};
use std::sync::Arc;
```

修改 `SyncService` struct：
```rust
pub struct SyncService {
    db_pool: DbPool,
    http_client: reqwest::Client,
    core_service_url: String,
    webhook_service: Arc<WebhookService>,
}
```

修改 `SyncService::new()`：
```rust
pub fn new(db_pool: DbPool, webhook_service: Arc<WebhookService>) -> Self {
    let core_service_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());
    Self {
        db_pool,
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap(),
        core_service_url,
        webhook_service,
    }
}
```

- [ ] **Step 2: 在 `notify_viewer()` 成功後觸發 webhook**

找到 `notify_viewer()` 中更新 `status = "syncing"` 之後的 `Ok(true)` 回傳，改為：

```rust
if response.status() == reqwest::StatusCode::ACCEPTED || response.status().is_success() {
    // Update status to syncing
    let now = chrono::Utc::now().naive_utc();
    diesel::update(
        downloads::table.filter(downloads::download_id.eq(download.download_id)),
    )
    .set((
        downloads::status.eq("syncing"),
        downloads::updated_at.eq(now),
    ))
    .execute(&mut conn)
    .map_err(|e| format!("Failed to update download status: {}", e))?;

    // Fire webhooks (fire-and-forget, build context from sync_request)
    let webhook_ctx = WebhookContext {
        download_id: sync_request.download_id,
        anime_id: sync_request.series_id,
        anime_title: sync_request.anime_title.clone(),
        episode_no: sync_request.episode_no,
        series_no: sync_request.series_no,
        subtitle_group: sync_request.subtitle_group.clone(),
        video_path: sync_request.video_path.clone(),
    };
    let wh_service = self.webhook_service.clone();
    tokio::spawn(async move {
        wh_service.fire(webhook_ctx).await;
    });

    Ok(true)
} else {
    Err(format!("Viewer returned status: {}", response.status()))
}
```

注意：需要在取得 `sync_request` 之後才能建構 `webhook_ctx`。目前 `notify_viewer()` 的結構是：
1. 查詢 link (conflict check)
2. 查詢 viewer
3. `build_sync_request()` → `sync_request`
4. 發送請求
5. 更新 status

`sync_request` 在步驟 3 取得，可以直接用。

- [ ] **Step 3: 修改 `state.rs` 的 AppState 建構**

在 `state.rs` 中新增 import：
```rust
use crate::services::WebhookService;
```

在 `AppState` struct 中新增：
```rust
pub webhook_service: Arc<WebhookService>,
```

修改 `AppState::new()`：
```rust
pub fn new(db: DbPool, registry: ServiceRegistry) -> Self {
    let repos = Repositories::new(db.clone());
    let dispatch_service = DownloadDispatchService::new(db.clone());
    let webhook_service = Arc::new(WebhookService::new(db.clone()));
    let sync_service = SyncService::new(db.clone(), webhook_service.clone());
    let conflict_detection = ConflictDetectionService::new(
        repos.anime_link.clone(),
        repos.anime_link_conflict.clone(),
        Arc::new(db.clone()),
    );
    let cancel_service = DownloadCancelService::new(db.clone());
    Self {
        db,
        registry: Arc::new(registry),
        repos: Arc::new(repos),
        dispatch_service: Arc::new(dispatch_service),
        sync_service: Arc::new(sync_service),
        conflict_detection: Arc::new(conflict_detection),
        cancel_service: Arc::new(cancel_service),
        webhook_service,
    }
}
```

- [ ] **Step 4: 編譯確認**

```bash
cd core-service && cargo check 2>&1 | head -40
```

預期：無錯誤。

---

## Chunk 3: HTTP 層

### Task 6: 建立 Webhook CRUD Handler

**Files:**
- Create: `core-service/src/handlers/webhooks.rs`
- Modify: `core-service/src/handlers/mod.rs`
- Modify: `core-service/src/main.rs`

- [ ] **Step 1: 建立 `handlers/webhooks.rs`**

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::{NewWebhook, Webhook};
use crate::state::AppState;
use crate::db::WebhookRepository;

// ─── Request / Response DTOs ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub name: String,
    pub url: String,
    pub payload_template: String,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebhookRequest {
    pub name: Option<String>,
    pub url: Option<String>,
    pub payload_template: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub webhook_id: i32,
    pub name: String,
    pub url: String,
    pub payload_template: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Webhook> for WebhookResponse {
    fn from(w: Webhook) -> Self {
        Self {
            webhook_id: w.webhook_id,
            name: w.name,
            url: w.url,
            payload_template: w.payload_template,
            is_active: w.is_active,
            created_at: w.created_at.to_string(),
            updated_at: w.updated_at.to_string(),
        }
    }
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// GET /webhooks — 列出所有 webhook
pub async fn list_webhooks(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let repo = crate::db::DieselWebhookRepository::new(state.db.clone());
    match repo.find_all().await {
        Ok(webhooks) => {
            let response: Vec<WebhookResponse> = webhooks.into_iter().map(Into::into).collect();
            (StatusCode::OK, Json(json!(response)))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error", "message": e.to_string()})),
        ),
    }
}

/// POST /webhooks — 建立 webhook
pub async fn create_webhook(
    State(state): State<AppState>,
    Json(payload): Json<CreateWebhookRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if payload.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_name", "message": "name cannot be empty"})),
        );
    }
    if payload.url.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_url", "message": "url cannot be empty"})),
        );
    }

    let now = Utc::now().naive_utc();
    let new_webhook = NewWebhook {
        name: payload.name,
        url: payload.url,
        payload_template: payload.payload_template,
        is_active: payload.is_active.unwrap_or(true),
        created_at: now,
        updated_at: now,
    };

    let repo = crate::db::DieselWebhookRepository::new(state.db.clone());
    match repo.create(new_webhook).await {
        Ok(webhook) => {
            tracing::info!("Created webhook: {}", webhook.webhook_id);
            (StatusCode::CREATED, Json(json!(WebhookResponse::from(webhook))))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error", "message": e.to_string()})),
        ),
    }
}

/// GET /webhooks/:id — 取得單一 webhook
pub async fn get_webhook(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let repo = crate::db::DieselWebhookRepository::new(state.db.clone());
    match repo.find_by_id(id).await {
        Ok(Some(webhook)) => (StatusCode::OK, Json(json!(WebhookResponse::from(webhook)))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "not_found", "message": "Webhook not found"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error", "message": e.to_string()})),
        ),
    }
}

/// PUT /webhooks/:id — 更新 webhook
pub async fn update_webhook(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateWebhookRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let repo = crate::db::DieselWebhookRepository::new(state.db.clone());

    let existing = match repo.find_by_id(id).await {
        Ok(Some(w)) => w,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "not_found", "message": "Webhook not found"})),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "database_error", "message": e.to_string()})),
            )
        }
    };

    let updated = crate::models::Webhook {
        name: payload.name.unwrap_or(existing.name),
        url: payload.url.unwrap_or(existing.url),
        payload_template: payload.payload_template.unwrap_or(existing.payload_template),
        is_active: payload.is_active.unwrap_or(existing.is_active),
        ..existing
    };

    match repo.update(updated).await {
        Ok(webhook) => (StatusCode::OK, Json(json!(WebhookResponse::from(webhook)))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error", "message": e.to_string()})),
        ),
    }
}

/// DELETE /webhooks/:id — 刪除 webhook
pub async fn delete_webhook(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let repo = crate::db::DieselWebhookRepository::new(state.db.clone());
    match repo.delete(id).await {
        Ok(true) => (StatusCode::OK, Json(json!({"deleted": true}))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "not_found", "message": "Webhook not found"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error", "message": e.to_string()})),
        ),
    }
}
```

- [ ] **Step 2: 更新 `handlers/mod.rs`**

新增：
```rust
pub mod webhooks;
```

- [ ] **Step 3: 更新 `main.rs`，新增 webhook 路由**

在路由區塊中（建議放在 `// 其他` 之前）新增：
```rust
// Webhook 管理
.route(
    "/webhooks",
    get(handlers::webhooks::list_webhooks).post(handlers::webhooks::create_webhook),
)
.route(
    "/webhooks/:id",
    get(handlers::webhooks::get_webhook)
        .put(handlers::webhooks::update_webhook)
        .delete(handlers::webhooks::delete_webhook),
)
```

- [ ] **Step 4: 完整編譯確認**

```bash
cd core-service && cargo build 2>&1 | tail -20
```

預期：`Compiling core-service ... Finished`，無錯誤。

- [ ] **Step 5: 執行所有測試**

```bash
cd core-service && cargo test 2>&1 | tail -20
```

預期：全部通過（包含 Task 4 的 webhook_service 單元測試）。

- [ ] **Step 6: Commit**

```bash
git add core-service/migrations/2026-03-13-000000-add-webhooks/ \
        core-service/src/schema.rs \
        core-service/src/models/db.rs \
        core-service/src/db/repository/webhook.rs \
        core-service/src/db/repository/mod.rs \
        core-service/src/db/mod.rs \
        core-service/src/services/webhook_service.rs \
        core-service/src/services/mod.rs \
        core-service/src/services/sync_service.rs \
        core-service/src/state.rs \
        core-service/src/handlers/webhooks.rs \
        core-service/src/handlers/mod.rs \
        core-service/src/main.rs
git commit -m "feat(webhook): add webhook trigger on download completion"
```

---

## API 端點摘要

| 方法 | 路徑 | 說明 |
|------|------|------|
| `GET` | `/webhooks` | 列出所有 webhook |
| `POST` | `/webhooks` | 建立 webhook |
| `GET` | `/webhooks/:id` | 取得單一 webhook |
| `PUT` | `/webhooks/:id` | 更新 webhook |
| `DELETE` | `/webhooks/:id` | 刪除 webhook |

### POST /webhooks 請求範例

```json
{
  "name": "Jellyfin 通知",
  "url": "https://hooks.example.com/notify",
  "payload_template": "{\"title\": \"{{anime_title}} 第{{episode_no}}集\", \"path\": \"{{video_path}}\", \"group\": \"{{subtitle_group}}\"}",
  "is_active": true
}
```

### 觸發時機

下載完成後 → `SyncService::notify_viewer()` 向 Viewer 發送同步請求成功 → 非同步觸發所有已啟用的 webhook（fire-and-forget，不阻塞 viewer 同步流程，webhook 失敗不影響主流程）。
