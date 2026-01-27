# 訂閱系統廣播機制完整設計

## 概述

實現一個同步廣播機制，當用戶創建訂閱時（無顯式 `fetcher_id`），向所有啟用的 Fetcher 詢問它們是否能處理該訂閱。如果用戶指定了 `fetcher_id`，則只向該 Fetcher 發送確認請求。選擇能處理且優先級最高的 Fetcher，或拒絕創建訂閱（如無任何 Fetcher 能處理）。

## 核心決策

1. **廣播模式**：同步阻塞（60 秒超時）
2. **無能力時處理**：嚴格模式（拒絕創建，返回 400 Bad Request）
3. **網絡地址管理**：FetcherModule 新增 `base_url` 字段
4. **邏輯統一**：同一個 `broadcast_can_handle()` 函數，透過可選 `target_fetcher_id` 參數支持兩種模式

## 數據庫更新

### 新建遷移

**文件**：`/workspace/core-service/migrations/2026-01-27-000002-add-fetcher-base-url/up.sql`

```sql
ALTER TABLE fetcher_modules
ADD COLUMN base_url VARCHAR(255) NOT NULL DEFAULT 'http://localhost:3000';

CREATE INDEX idx_fetcher_modules_base_url ON fetcher_modules(base_url);
```

**Rollback**：`down.sql`

```sql
DROP INDEX IF EXISTS idx_fetcher_modules_base_url;
ALTER TABLE fetcher_modules
DROP COLUMN base_url;
```

### 模型更新

在 `/workspace/core-service/src/models/db.rs` 中，FetcherModule 和 NewFetcherModule 都添加：

```rust
pub base_url: String,  // 例如 "http://localhost:3001"
```

## 核心邏輯實現

### 1. 統一廣播函數

位置：`/workspace/core-service/src/handlers/subscriptions.rs`

函數簽名：
```rust
pub async fn broadcast_can_handle(
    state: &AppState,
    source_url: &str,
    source_type: &str,
    timeout_secs: u64,
    target_fetcher_id: Option<i32>,  // None=廣播到所有，Some(id)=只查該 Fetcher
) -> Result<Vec<(i32, i32)>, String>  // 返回 (fetcher_id, priority) 按優先級降序
```

**邏輯流程**：

1. 獲取數據庫連接
2. 根據 `target_fetcher_id` 決定查詢範圍：
   - `None`：查所有 `is_enabled=true` 的 Fetcher
   - `Some(id)`：只查該特定 Fetcher 且必須啟用
3. 驗證邊界情況：
   - Fetcher 列表不為空
   - 每個 Fetcher 的 `base_url` 非空
4. 並發發送請求：
   - 為每個 Fetcher 創建 tokio task
   - 發送 `POST {base_url}/can-handle-subscription`
   - 請求體包含 `source_url` 和 `source_type`
   - 共享 60 秒超時
5. 收集結果：
   - 只返回 `can_handle=true` 的 Fetcher
   - 按 `priority` 降序排列
6. 返回結果或錯誤

**錯誤處理**：
- 數據庫連接失敗 → 返回 `Err`
- 目標 Fetcher 不存在或未啟用 → 返回 `Err`
- Fetcher base_url 未配置 → 返回 `Err`
- 請求超時 → 日誌警告，忽略該 Fetcher
- 響應無效 JSON → 日誌警告，忽略該 Fetcher

### 2. 訂閱創建流程改造

位置：`/workspace/core-service/src/handlers/subscriptions.rs` 中的 `create_subscription()`

**新流程**：

1. 檢查重複訂閱（如已存在則返回 409 Conflict）
2. **調用廣播函數**：
   ```rust
   match broadcast_can_handle(&state, &payload.source_url, &source_type, 60, payload.fetcher_id).await {
       Ok(capable_fetchers) if !capable_fetchers.is_empty() => {
           // 步驟 3：選擇優先級最高的
           let (fetcher_id, _priority) = capable_fetchers[0];
           // 步驟 4：創建訂閱
       }
       Ok(_) => {
           // 沒有任何 Fetcher 能處理 → 返回 400 Bad Request
           return (StatusCode::BAD_REQUEST, Json(json!({
               "error": "no_capable_fetcher",
               "message": "No fetcher can handle this subscription request"
           })));
       }
       Err(e) => {
           // 廣播過程失敗 → 返回 500
           return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
               "error": "broadcast_failed",
               "message": e
           })));
       }
   }
   ```
3. 創建訂閱記錄到數據庫
4. 返回 201 Created

**assignment_status 設置**：
- 自動選擇（`payload.fetcher_id.is_none()`）→ `"auto_assigned"`
- 指定 Fetcher（`payload.fetcher_id.is_some()`）→ `"assigned"`

### 3. DTO 更新

在 `/workspace/core-service/src/handlers/subscriptions.rs` 中，CanHandleResponse 的結構：

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct CanHandleResponse {
    pub can_handle: bool,
    pub fetcher_id: Option<i32>,   // 可選，由 Fetcher 提供（CORE 會忽略）
    pub priority: Option<i32>,     // 可選，由 Fetcher 提供（CORE 會忽略）
}
```

CORE 已知每個 Fetcher 的 ID 和優先級，所以 Fetcher 回應中的這兩個欄位可選。

## Fetcher 側改動

### Mikanani Fetcher

**1. can_handle_subscription 處理器**（`/workspace/fetchers/mikanani/src/handlers.rs`）

保持現有實現，僅返回 `can_handle` 布爾值：

```rust
pub async fn can_handle_subscription(
    Json(payload): Json<CanHandleRequest>,
) -> (StatusCode, Json<CanHandleResponse>) {
    let can_handle = payload.source_type == "rss"
        && payload.source_url.contains("mikanani.me");

    let response = CanHandleResponse {
        can_handle,
        fetcher_id: None,
        priority: None,
    };

    let status = if can_handle {
        StatusCode::OK
    } else {
        StatusCode::NO_CONTENT
    };

    (status, Json(response))
}
```

**2. 註冊時提供 base_url**（`/workspace/fetchers/mikanani/src/main.rs`）

```rust
struct RegisterPayload {
    service_name: String,
    host: String,
    port: u16,
    base_url: String,  // 新增
}

let register_payload = RegisterPayload {
    service_name: "mikanani-fetcher".to_string(),
    host: hostname.clone(),
    port: port,
    base_url: format!("http://{}:{}", hostname, port),
};
```

### CORE Service 註冊端點更新

在 `/workspace/core-service/src/handlers/services.rs` 的 Fetcher 註冊中：

```rust
#[derive(Debug, serde::Deserialize)]
pub struct RegisterFetcherRequest {
    pub service_name: String,
    pub host: String,
    pub port: u16,
    pub base_url: Option<String>,
}

pub async fn register_fetcher(
    State(state): State<AppState>,
    Json(payload): Json<RegisterFetcherRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let base_url = payload.base_url.unwrap_or_else(|| {
        format!("http://{}:{}", payload.host, payload.port)
    });

    // 使用 base_url 創建 NewFetcherModule...
}
```

## 請求/響應流程

### 用戶創建訂閱（自動選擇模式）

**請求**：
```http
POST /subscriptions
Content-Type: application/json

{
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215",
    "name": "Test Anime"
}
```

**CORE 內部流程**：
1. 調用 `broadcast_can_handle(..., target_fetcher_id=None)`
2. 向所有啟用 Fetcher 發送 `POST {base_url}/can-handle-subscription`
3. Mikanani Fetcher 回應 `{can_handle: true}`
4. 選擇最高優先級的 Fetcher（假設 mikanani=100）
5. 創建訂閱，`assignment_status="auto_assigned"`, `auto_selected=true`

**響應**：
```http
201 Created
Content-Type: application/json

{
    "subscription_id": 1,
    "fetcher_id": 1,
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215",
    "name": "Test Anime",
    "source_type": "rss",
    "assignment_status": "auto_assigned",
    "auto_selected": true,
    "assigned_at": null,
    ...
}
```

### 用戶創建訂閱（指定 Fetcher）

**請求**：
```http
POST /subscriptions
Content-Type: application/json

{
    "fetcher_id": 1,
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215",
    "name": "Test Anime"
}
```

**CORE 內部流程**：
1. 調用 `broadcast_can_handle(..., target_fetcher_id=Some(1))`
2. 只向 Fetcher #1 發送確認請求
3. 如果回應 `can_handle: false` 或超時 → 返回 400 Bad Request
4. 如果回應 `can_handle: true` → 創建訂閱，`assignment_status="assigned"`, `auto_selected=false`

**響應（成功）**：
```http
201 Created
...
{
    "assignment_status": "assigned",
    "auto_selected": false,
    "assigned_at": "2026-01-27T10:00:00",
    ...
}
```

**響應（失敗）**：
```http
400 Bad Request
{
    "error": "no_capable_fetcher",
    "message": "Fetcher 1 cannot handle this subscription request"
}
```

## 錯誤場景

| 場景 | 返回狀態 | 說明 |
|------|--------|------|
| 訂閱 URL 已存在 | 409 Conflict | 數據庫層面重複檢查 |
| 沒有啟用 Fetcher | 500 Internal Server Error | 廣播失敗 |
| 指定 Fetcher 不存在 | 500 Internal Server Error | 廣播失敗 |
| 沒有 Fetcher 能處理 | 400 Bad Request | 嚴格模式，拒絕創建 |
| 所有 Fetcher 超時 | 400 Bad Request | 嚴格模式，視同無能力 |
| Fetcher base_url 未配置 | 500 Internal Server Error | 廣播失敗 |
| 數據庫連接失敗 | 500 Internal Server Error | 廣播失敗 |

## 訂閱表使用

現有訂閱表欄位完整支持此機制，無需調整：

- `fetcher_id` - 廣播後選定的 Fetcher
- `source_url` - 廣播時發送給 Fetcher
- `source_type` - 廣播時發送給 Fetcher
- `assignment_status` - `auto_assigned`/`assigned`
- `auto_selected` - 布爾標記
- `assigned_at` - 時間戳

詳細日誌（哪些 Fetcher 被詢問、它們的回應等）透過 `tracing` 在應用層記錄。

## 測試覆蓋

1. **廣播成功場景**：
   - 自動選擇：多個 Fetcher 能處理，選擇最高優先級
   - 指定 Fetcher：該 Fetcher 能處理，創建成功

2. **廣播失敗場景**：
   - 無任何 Fetcher 能處理 → 400 Bad Request
   - 指定 Fetcher 無法處理 → 400 Bad Request
   - 指定 Fetcher 不存在 → 500 Internal Server Error
   - 所有 Fetcher 超時 → 400 Bad Request

3. **邊界情況**：
   - Fetcher base_url 為空 → 500 Internal Server Error
   - 沒有啟用 Fetcher → 500 Internal Server Error
   - 訂閱 URL 重複 → 409 Conflict

## 日誌級別

- `INFO`：訂閱成功創建、選擇的 Fetcher ID
- `DEBUG`：每個 Fetcher 的回應（can_handle=true/false）
- `WARN`：Fetcher 無響應、超時、無效回應
- `ERROR`：數據庫錯誤、廣播失敗、配置問題
