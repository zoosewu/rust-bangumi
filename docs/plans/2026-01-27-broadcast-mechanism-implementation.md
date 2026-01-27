# 訂閱系統廣播機制 - 實現計畫

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 實現同步廣播機制，在訂閱創建時向 Fetcher 詢問能力，選擇優先級最高的能處理者，不能處理則拒絕創建。

**Architecture:** 統一的 `broadcast_can_handle()` 函數支持自動選擇和指定 Fetcher 兩種模式。透過新增 `base_url` 字段管理 Fetcher 網絡地址。在訂閱創建時同步調用廣播，決策失敗則返回相應 HTTP 錯誤。

**Tech Stack:** Rust, Diesel ORM, Tokio async, Axum, reqwest HTTP client

---

## Task 1: 建立數據庫遷移 - base_url 欄位

**Files:**
- Create: `/workspace/core-service/migrations/2026-01-27-000002-add-fetcher-base-url/up.sql`
- Create: `/workspace/core-service/migrations/2026-01-27-000002-add-fetcher-base-url/down.sql`

**Step 1: 建立遷移目錄和 up.sql 檔案**

```bash
mkdir -p /workspace/core-service/migrations/2026-01-27-000002-add-fetcher-base-url
```

在 `/workspace/core-service/migrations/2026-01-27-000002-add-fetcher-base-url/up.sql` 中：

```sql
-- Add base_url column to fetcher_modules for storing Fetcher service URLs
ALTER TABLE fetcher_modules
ADD COLUMN base_url VARCHAR(255) NOT NULL DEFAULT 'http://localhost:3000';

-- Index for faster lookups by base_url
CREATE INDEX idx_fetcher_modules_base_url ON fetcher_modules(base_url);
```

**Step 2: 建立 down.sql 檔案**

在 `/workspace/core-service/migrations/2026-01-27-000002-add-fetcher-base-url/down.sql` 中：

```sql
-- Rollback: Remove base_url column and index
DROP INDEX IF EXISTS idx_fetcher_modules_base_url;

ALTER TABLE fetcher_modules
DROP COLUMN base_url;
```

**Step 3: 驗證遷移檔案正確**

```bash
ls -la /workspace/core-service/migrations/2026-01-27-000002-add-fetcher-base-url/
```

Expected output: 兩個檔案 up.sql 和 down.sql

**Step 4: 執行遷移**

```bash
cd /workspace && diesel migration run
```

Expected: 遷移成功，數據庫新增 base_url 欄位

**Step 5: 更新 Diesel schema**

```bash
cd /workspace && diesel print-schema > /workspace/core-service/src/schema.rs
```

Expected: schema.rs 更新，fetcher_modules 表定義中新增 base_url 欄位

**Step 6: Commit**

```bash
git add core-service/migrations/2026-01-27-000002-add-fetcher-base-url/
git add core-service/src/schema.rs
git commit -m "feat: add base_url column to fetcher_modules table"
```

---

## Task 2: 更新 Rust 模型 - FetcherModule 和 NewFetcherModule

**Files:**
- Modify: `/workspace/core-service/src/models/db.rs` (FetcherModule, NewFetcherModule)
- Modify: `/workspace/core-service/src/handlers/services.rs` (Fetcher 註冊時設置 base_url)

**Step 1: 更新 FetcherModule struct**

在 `/workspace/core-service/src/models/db.rs` 中找到 FetcherModule 結構體，在 priority 後添加 base_url：

```rust
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::fetcher_modules)]
pub struct FetcherModule {
    pub fetcher_id: i32,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub config_schema: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub priority: i32,
    pub base_url: String,  // 新增欄位
}
```

**Step 2: 更新 NewFetcherModule struct**

在同一檔案中找到 NewFetcherModule，添加 base_url：

```rust
#[derive(Insertable)]
#[diesel(table_name = super::super::schema::fetcher_modules)]
pub struct NewFetcherModule {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub config_schema: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub priority: i32,
    pub base_url: String,  // 新增欄位
}
```

**Step 3: 編譯檢查**

```bash
cd /workspace && cargo check --lib
```

Expected: 編譯通過或有明確的編譯錯誤需要修正

**Step 4: 更新 Fetcher 註冊邏輯**

在 `/workspace/core-service/src/handlers/services.rs` 的 `register_fetcher()` 函數中，更新 payload DTO 和實現邏輯：

找到註冊請求的 payload 定義，添加可選的 base_url：

```rust
#[derive(Debug, serde::Deserialize)]
pub struct RegisterFetcherRequest {
    pub service_name: String,
    pub host: String,
    pub port: u16,
    pub base_url: Option<String>,  // 新增可選參數
}
```

在 `register_fetcher()` 函數的 NewFetcherModule 創建部分：

```rust
let base_url = payload.base_url.unwrap_or_else(|| {
    format!("http://{}:{}", payload.host, payload.port)
});

// 後續 NewFetcherModule 創建時使用此 base_url
let new_fetcher = NewFetcherModule {
    name: payload.service_name.clone(),
    version: "1.0.0".to_string(),
    description: Some(format!("{}:{}:{}", payload.service_name, payload.host, payload.port)),
    is_enabled: true,
    config_schema: None,
    created_at: naive_now,
    updated_at: naive_now,
    priority: 50,
    base_url,  // 使用計算得到的 base_url
};
```

**Step 5: 編譯檢查**

```bash
cd /workspace && cargo check
```

Expected: 編譯通過

**Step 6: Commit**

```bash
git add core-service/src/models/db.rs
git add core-service/src/handlers/services.rs
git commit -m "feat: add base_url field to FetcherModule and update registration"
```

---

## Task 3: 實現統一廣播函數 broadcast_can_handle()

**Files:**
- Modify: `/workspace/core-service/src/handlers/subscriptions.rs`

**Step 1: 添加廣播函數**

在 `/workspace/core-service/src/handlers/subscriptions.rs` 的 `auto_select_fetcher()` 函數下方添加新的廣播函數：

```rust
/// Broadcast can_handle requests to all enabled fetchers (or specific fetcher if target_fetcher_id is provided)
/// Returns a sorted list of fetchers that can handle the subscription (sorted by priority DESC)
/// Empty list means no fetcher can handle it
pub async fn broadcast_can_handle(
    state: &AppState,
    source_url: &str,
    source_type: &str,
    timeout_secs: u64,
    target_fetcher_id: Option<i32>,
) -> Result<Vec<(i32, i32)>, String> {
    let mut conn = state.db.get()
        .map_err(|e| format!("Database connection failed: {}", e))?;

    use crate::schema::fetcher_modules::dsl::*;

    // Build query based on target_fetcher_id
    let fetcher_list = if let Some(target_id) = target_fetcher_id {
        // Query specific fetcher
        fetcher_modules
            .filter(is_enabled.eq(true))
            .filter(fetcher_id.eq(target_id))
            .select(FetcherModule::as_select())
            .load::<FetcherModule>(&mut conn)
    } else {
        // Query all enabled fetchers
        fetcher_modules
            .filter(is_enabled.eq(true))
            .select(FetcherModule::as_select())
            .load::<FetcherModule>(&mut conn)
    };

    let fetcher_list = fetcher_list
        .map_err(|e| format!("Failed to load fetchers: {}", e))?;

    // Edge case 1: No fetchers found
    if fetcher_list.is_empty() {
        return if target_fetcher_id.is_some() {
            Err(format!("Target fetcher {} not found or disabled", target_fetcher_id.unwrap()))
        } else {
            Err("No enabled fetchers available".to_string())
        };
    }

    // Edge case 2: Validate base_url is configured
    for fetcher in &fetcher_list {
        if fetcher.base_url.is_empty() {
            return Err(format!("Fetcher {} has no base_url configured", fetcher.fetcher_id));
        }
    }

    // Spawn concurrent tasks for all fetchers
    let mut handles = vec![];
    for fetcher in fetcher_list {
        let handle = tokio::spawn(async move {
            let client = reqwest::Client::new();
            let url = format!("{}/can-handle-subscription", fetcher.base_url);

            let payload = CanHandleRequest {
                source_url: source_url.to_string(),
                source_type: source_type.to_string(),
            };

            match timeout(
                Duration::from_secs(timeout_secs),
                client.post(&url).json(&payload).send(),
            )
            .await
            {
                Ok(Ok(response)) => {
                    match response.json::<CanHandleResponse>().await {
                        Ok(data) if data.can_handle => {
                            tracing::debug!(
                                "Fetcher {} can handle: {} (priority: {})",
                                fetcher.fetcher_id,
                                source_url,
                                fetcher.priority
                            );
                            Some((fetcher.fetcher_id, fetcher.priority))
                        }
                        Ok(_) => {
                            tracing::debug!("Fetcher {} cannot handle: {}", fetcher.fetcher_id, source_url);
                            None
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Fetcher {} returned invalid response: {}",
                                fetcher.fetcher_id,
                                e
                            );
                            None
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("Fetcher {} request failed: {}", fetcher.fetcher_id, e);
                    None
                }
                Err(_) => {
                    tracing::warn!(
                        "Fetcher {} timeout after {} seconds",
                        fetcher.fetcher_id,
                        timeout_secs
                    );
                    None
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all responses
    let mut capable_fetchers = vec![];
    for handle in handles {
        if let Ok(Some(result)) = handle.await {
            capable_fetchers.push(result);
        }
    }

    // Sort by priority descending
    capable_fetchers.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(capable_fetchers)
}
```

**Step 2: 編譯檢查**

```bash
cd /workspace && cargo check
```

Expected: 編譯通過

**Step 3: Commit**

```bash
git add core-service/src/handlers/subscriptions.rs
git commit -m "feat: implement broadcast_can_handle function with support for target fetcher"
```

---

## Task 4: 改造訂閱創建流程 - 集成廣播

**Files:**
- Modify: `/workspace/core-service/src/handlers/subscriptions.rs` (create_subscription 函數)

**Step 1: 重寫 create_subscription 邏輯**

替換現有的 `create_subscription()` 函數中的決策邏輯部分（從檢查重複開始到創建記錄前）：

重點改動：
1. 移除舊的 `auto_select_fetcher()` 呼叫
2. 添加 `broadcast_can_handle()` 呼叫
3. 新增嚴格模式的檢查（無能力 Fetcher → 返回 400）

```rust
// 在 create_subscription() 中，替換重複檢查後的決策邏輯：

// 檢查重複訂閱（保持原有）
let existing = subscriptions::table
    .filter(subscriptions::source_url.eq(&payload.source_url))
    .select(Subscription::as_select())
    .first::<Subscription>(&mut conn)
    .optional();

match existing {
    Ok(Some(_)) => {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "already_exists",
                "message": "Subscription already exists for this URL"
            })),
        );
    }
    Err(e) => {
        tracing::error!("Database error checking subscription: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error"})),
        );
    }
    _ => {}
}

// 新增：廣播詢問 Fetcher（核心改動）
match broadcast_can_handle(
    &state,
    &payload.source_url,
    &source_type,
    60,
    payload.fetcher_id,  // 可選的目標 Fetcher
)
.await
{
    Ok(capable_fetchers) if !capable_fetchers.is_empty() => {
        // 選擇優先級最高的 Fetcher（已排序）
        let (fetcher_id, _priority) = capable_fetchers[0];
        let auto_selected = payload.fetcher_id.is_none();
        let assignment_status = if auto_selected {
            "auto_assigned"
        } else {
            "assigned"
        };

        // 創建訂閱記錄（保持原有邏輯）
        let new_subscription = NewSubscription {
            fetcher_id,
            source_url: payload.source_url.clone(),
            name: payload.name.clone(),
            description: payload.description.clone(),
            last_fetched_at: None,
            next_fetch_at: Some(now),
            fetch_interval_minutes: fetch_interval,
            is_active: true,
            config: payload.config.clone(),
            created_at: now,
            updated_at: now,
            source_type: source_type.clone(),
            assignment_status: assignment_status.to_string(),
            assigned_at: if auto_selected { None } else { Some(now) },
            auto_selected,
        };

        match diesel::insert_into(subscriptions::table)
            .values(&new_subscription)
            .returning(Subscription::as_returning())
            .get_result::<Subscription>(&mut conn)
        {
            Ok(subscription) => {
                tracing::info!(
                    "Created subscription {} for URL {} with fetcher {} ({})",
                    subscription.subscription_id,
                    subscription.source_url,
                    fetcher_id,
                    assignment_status
                );
                (StatusCode::CREATED, Json(json!(subscription)))
            }
            Err(e) => {
                tracing::error!("Failed to create subscription: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "creation_failed"})),
                )
            }
        }
    }
    Ok(_) => {
        // 沒有 Fetcher 能處理（嚴格模式）
        tracing::warn!(
            "No fetcher can handle subscription for URL: {} (type: {})",
            payload.source_url,
            source_type
        );
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "no_capable_fetcher",
                "message": "No fetcher can handle this subscription request"
            })),
        )
    }
    Err(e) => {
        tracing::error!("Broadcast failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "broadcast_failed",
                "message": e
            })),
        )
    }
}
```

**Step 2: 編譯檢查**

```bash
cd /workspace && cargo check
```

Expected: 編譯通過

**Step 3: 完整編譯**

```bash
cd /workspace && cargo build --release
```

Expected: 成功編譯

**Step 4: Commit**

```bash
git add core-service/src/handlers/subscriptions.rs
git commit -m "feat: integrate broadcast mechanism into create_subscription flow"
```

---

## Task 5: 更新 Mikanani Fetcher 註冊邏輯

**Files:**
- Modify: `/workspace/fetchers/mikanani/src/main.rs` (註冊時提供 base_url)

**Step 1: 更新 RegisterPayload**

在 `/workspace/fetchers/mikanani/src/main.rs` 中找到 Fetcher 註冊的 payload 結構，添加 base_url：

```rust
#[derive(Debug, serde::Serialize)]
struct RegisterPayload {
    service_name: String,
    host: String,
    port: u16,
    base_url: String,  // 新增：Fetcher 的完整服務 URL
}
```

**Step 2: 構造並發送註冊請求時包含 base_url**

找到發送 register 請求的程式碼，修改 payload 構造：

```rust
let register_payload = RegisterPayload {
    service_name: "mikanani-fetcher".to_string(),
    host: hostname.clone(),
    port: port,
    base_url: format!("http://{}:{}", hostname, port),  // 構造 base_url
};
```

**Step 3: 編譯檢查**

```bash
cd /workspace && cargo check --manifest-path fetchers/mikanani/Cargo.toml
```

Expected: 編譯通過

**Step 4: 完整編譯**

```bash
cd /workspace && cargo build --release --manifest-path fetchers/mikanani/Cargo.toml
```

Expected: 成功編譯

**Step 5: Commit**

```bash
git add fetchers/mikanani/src/main.rs
git commit -m "feat: include base_url in mikanani fetcher registration"
```

---

## Task 6: 完整端到端編譯驗證

**Files:**
- N/A (驗證步驟)

**Step 1: 清潔編譯**

```bash
cd /workspace && cargo clean && cargo build --release
```

Expected: 所有代碼成功編譯，無錯誤

**Step 2: 檢查編譯警告**

```bash
cd /workspace && cargo build --release 2>&1 | grep "^error"
```

Expected: 無 error 輸出（warning 可以接受）

**Step 3: 執行庫測試**

```bash
cd /workspace && cargo test --lib
```

Expected: 所有庫測試通過

**Step 4: 驗證主程序構建**

```bash
cd /workspace && cargo build --release --bin core-service
```

Expected: core-service 二進制成功構建

```bash
cd /workspace && cargo build --release --bin fetcher-mikanani
```

Expected: fetcher-mikanani 二進制成功構建

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: verify complete broadcast mechanism compilation"
```

---

## Task 7: 功能測試驗證

**Files:**
- N/A (手動測試)

**Step 1: 測試場景 - 自動選擇，單個 Fetcher 能處理**

前提：Mikanani Fetcher 已啟動且已向 CORE 註冊

發送請求：
```bash
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215",
    "name": "Test Anime"
  }'
```

Expected response: `201 Created`，包含 `"assignment_status": "auto_assigned"`, `"auto_selected": true`

**Step 2: 測試場景 - 指定 Fetcher，該 Fetcher 能處理**

發送請求：
```bash
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "fetcher_id": 1,
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3216",
    "name": "Test Anime 2"
  }'
```

Expected response: `201 Created`，包含 `"assignment_status": "assigned"`, `"auto_selected": false`

**Step 3: 測試場景 - 自動選擇，沒有任何 Fetcher 能處理**

發送請求（不同的 URL，Mikanani 不能處理）：
```bash
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "source_url": "https://example.com/feed.xml",
    "name": "Non-Mikanani Feed"
  }'
```

Expected response: `400 Bad Request`，包含 `"error": "no_capable_fetcher"`

**Step 4: 驗證日誌輸出**

檢查 CORE Service 和 Mikanani Fetcher 的日誌中：
- CORE 日誌應包含：「Broadcasting can_handle to fetchers」或「No fetcher can handle」
- Mikanani 日誌應包含：「Checking if can handle subscription」

**Step 5: 驗證數據庫**

```bash
psql $DATABASE_URL -c "SELECT subscription_id, fetcher_id, assignment_status, auto_selected FROM subscriptions ORDER BY created_at DESC LIMIT 3;"
```

Expected: 查到的訂閱記錄中，assignment_status 應為 'auto_assigned' 或 'assigned'，auto_selected 應對應

**Step 6: 驗證成功，無需 commit（測試操作）**

---

## 檢查清單

- [ ] 數據庫遷移成功執行
- [ ] FetcherModule 模型包含 base_url 欄位
- [ ] broadcast_can_handle() 函數實現完整
- [ ] create_subscription() 整合廣播機制
- [ ] Mikanani Fetcher 在註冊時提供 base_url
- [ ] 完整編譯無錯誤
- [ ] 自動選擇場景測試通過
- [ ] 指定 Fetcher 場景測試通過
- [ ] 無能力拒絕場景測試通過
- [ ] 日誌輸出正確
- [ ] 數據庫記錄正確
