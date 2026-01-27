# 訂閱系統最終設計

**日期：** 2026-01-26
**版本：** Final - 無追蹤表

---

## 📋 確認項目

- [x] 超時：60 秒
- [x] 優先級：整數
- [x] 決策：布林值（能/不能）
- [x] 選擇：優先級最高的能處理的 Fetcher
- [x] **無追蹤表**（簡洁設計）

---

## 🗄️ 數據庫設計（最終版）

### 修改 `fetcher_modules` 表

```sql
ALTER TABLE fetcher_modules ADD COLUMN (
  priority INTEGER NOT NULL DEFAULT 50
  COMMENT '優先級：整數，值越大優先級越高。範例: 10, 50, 80, 100'
);
```

### 修改 `subscriptions` 表（改名自 rss_subscriptions）

```sql
ALTER TABLE rss_subscriptions RENAME TO subscriptions;

ALTER TABLE subscriptions
RENAME COLUMN rss_url TO source_url;

ALTER TABLE subscriptions ADD COLUMN (
  source_type VARCHAR(50) NOT NULL DEFAULT 'rss'
  COMMENT '源類型: rss, http, custom, etc.',

  assignment_status VARCHAR(20) NOT NULL DEFAULT 'pending'
  COMMENT 'pending, assigned, failed, inactive',

  assigned_at TIMESTAMP NULL,

  auto_selected BOOLEAN NOT NULL DEFAULT false
  COMMENT '是否通過自動選擇分配（true）還是手動指定（false）'
);

-- 更新唯一約束
ALTER TABLE subscriptions DROP CONSTRAINT subscriptions_fetcher_id_rss_url_key;
ALTER TABLE subscriptions ADD CONSTRAINT
  subscriptions_source_url_fetcher_id_key
  UNIQUE(source_url, fetcher_id);
```

**最終 subscriptions 表結構：**
```
subscription_id      SERIAL PRIMARY KEY
fetcher_id           INTEGER NOT NULL REFERENCES fetcher_modules
source_url           VARCHAR(2048) NOT NULL
source_type          VARCHAR(50) DEFAULT 'rss'
name                 VARCHAR(255)
description          TEXT
last_fetched_at      TIMESTAMP
next_fetch_at        TIMESTAMP
fetch_interval_minutes INTEGER DEFAULT 60
is_active            BOOLEAN DEFAULT true
config               JSONB
assignment_status    VARCHAR(20) DEFAULT 'pending'
assigned_at          TIMESTAMP
auto_selected        BOOLEAN DEFAULT false
created_at           TIMESTAMP
updated_at           TIMESTAMP
```

---

## 🔌 API 規格（最終版）

### CORE Service

#### 創建訂閱

```yaml
POST /subscriptions

Request:
  source_url*: string
    示例: "https://mikanani.me/RSS/Bangumi?bangumiId=3215"

  fetcher_id?: integer
    如果提供，直接分配給此 Fetcher（跳過廣播）

  name?: string
  description?: string
  fetch_interval_minutes?: integer (預設: 60)
  config?: object

Response 201:
  subscription_id: integer
  source_url: string
  source_type: string
  fetcher_id: integer (分配的 Fetcher ID)
  assignment_status: string ("pending", "assigned", "failed")
  auto_selected: boolean (true=自動選擇, false=手動指定)
  assigned_at: timestamp (null 表示還未分配)
  created_at: timestamp
```

#### 列出訂閱

```yaml
GET /subscriptions?status=assigned&fetcher_id=1

Response 200:
  - Array of subscriptions
```

---

### Fetcher Service

#### 判斷是否能處理

```yaml
POST /can-handle-subscription

Request:
  source_url*: string

Response 200:
  can_handle*: boolean

實現例子（Mikanani）:
  return { "can_handle": source_url.contains("mikanani.me") }
```

#### 接收訂閱

```yaml
POST /subscribe

Request:
  subscription_id*: integer
  source_url*: string
  source_type*: string
  name?: string
  config?: object

Response 200:
  status: string ("accepted", "processing")
  message: string
```

---

## 🏗️ CORE 實現邏輯

### 創建訂閱流程

```rust
async fn create_subscription(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> Result<SubscriptionResponse> {

    // 1. 驗證 source_url
    if payload.source_url.is_empty() {
        return Err("source_url cannot be empty");
    }

    // 2. 儲存訂閱到數據庫（初始狀態：pending）
    let subscription = db::insert_subscription(&payload);

    // 3. 決定 Fetcher
    let (assigned_fetcher_id, auto_selected) = if let Some(fetcher_id) = payload.fetcher_id {
        // 顯式指定：驗證 Fetcher 存在且已啟用
        verify_fetcher_exists_and_enabled(fetcher_id)?;
        (fetcher_id, false)
    } else {
        // 自動選擇：廣播給所有 Fetcher
        let fetcher_id = auto_select_fetcher(&subscription).await?;
        (fetcher_id, true)
    };

    // 4. 通知 Fetcher
    notify_fetcher(assigned_fetcher_id, &subscription).await?;

    // 5. 更新訂閱狀態
    db::update_subscription(subscription.id, |s| {
        s.fetcher_id = assigned_fetcher_id;
        s.assignment_status = "assigned";
        s.assigned_at = Some(now());
        s.auto_selected = auto_selected;
    });

    Ok(subscription_to_response(subscription))
}

async fn auto_select_fetcher(subscription: &Subscription) -> Result<i32> {
    // 1. 獲取所有已啟用的 Fetcher
    let fetchers = db::get_enabled_fetchers();

    if fetchers.is_empty() {
        return Err("No fetcher available".into());
    }

    // 2. 並發廣播給所有 Fetcher（60 秒超時）
    let handles: Vec<_> = fetchers
        .iter()
        .map(|f| {
            let source_url = subscription.source_url.clone();
            tokio::spawn(async move {
                broadcast_can_handle(f, &source_url).await
            })
        })
        .collect();

    let results = futures::future::join_all(handles).await;

    // 3. 過濾回應結果
    let mut candidates: Vec<(i32, i32)> = Vec::new(); // (fetcher_id, priority)

    for (i, result) in results.iter().enumerate() {
        if let Ok(Ok(response)) = result {
            if response.can_handle {
                candidates.push((fetchers[i].id, fetchers[i].priority));
            }
        }
        // 超時或錯誤視為不能處理
    }

    if candidates.is_empty() {
        return Err("No fetcher can handle this URL".into());
    }

    // 4. 選擇優先級最高的
    let selected_id = candidates
        .into_iter()
        .max_by_key(|(_, priority)| *priority)
        .map(|(id, _)| id)
        .unwrap();

    Ok(selected_id)
}

async fn broadcast_can_handle(
    fetcher: &FetcherModule,
    source_url: &str,
) -> Result<CanHandleResponse> {
    let client = reqwest::Client::new();
    let url = format!(
        "http://{}:{}/can-handle-subscription",
        fetcher.host, fetcher.port
    );

    let response = tokio::time::timeout(
        Duration::from_secs(60),  // 60 秒超時
        client.post(&url)
            .json(&CanHandleRequest {
                source_url: source_url.to_string(),
            })
            .send()
    )
    .await
    .map_err(|_| "Timeout")?
    .map_err(|e| e.to_string())?
    .json::<CanHandleResponse>()
    .await
    .map_err(|e| e.to_string())?;

    Ok(response)
}
```

---

## 🔄 Fetcher 實現

### Mikanani Fetcher

#### 1. 實現 `/can-handle-subscription`

```rust
#[derive(serde::Deserialize)]
pub struct CanHandleRequest {
    pub source_url: String,
}

#[derive(serde::Serialize)]
pub struct CanHandleResponse {
    pub can_handle: bool,
}

pub async fn can_handle_subscription(
    Json(payload): Json<CanHandleRequest>,
) -> Json<CanHandleResponse> {
    let can_handle = payload.source_url.contains("mikanani.me");
    tracing::debug!(
        "Mikanani can_handle_subscription: {} -> {}",
        payload.source_url,
        can_handle
    );
    Json(CanHandleResponse { can_handle })
}
```

#### 2. 在 main.rs 中添加路由

```rust
let app = Router::new()
    .route("/health", get(handlers::health_check))
    .route("/fetch", post(handlers::fetch))
    .route("/subscribe", post(handlers::handle_subscription_broadcast))
    .route("/can-handle-subscription", post(can_handle_subscription))  // 新增
    .with_state(parser);
```

---

## 📋 實施清單

### Phase 1：數據庫遷移 ✅

- [ ] 編寫 migration SQL
  - [ ] 添加 `priority` 到 `fetcher_modules`
  - [ ] 重命名 `rss_subscriptions` → `subscriptions`
  - [ ] 重命名 `rss_url` → `source_url`
  - [ ] 添加 `source_type`, `assignment_status`, `assigned_at`, `auto_selected`
- [ ] 執行遷移
- [ ] 驗證數據完整性

### Phase 2：CORE Service 實現 ✅

- [ ] 實現 `auto_select_fetcher()` 函數
- [ ] 實現 `broadcast_can_handle()` 函數
  - [ ] 並發調用所有 Fetcher
  - [ ] 60 秒超時
  - [ ] 錯誤處理
- [ ] 修改 `create_subscription()` 端點
  - [ ] 支持 `fetcher_id` 參數（可選）
  - [ ] 自動選擇邏輯
  - [ ] 顯式指定邏輯
- [ ] 修改 `notify_fetcher()` 實現

### Phase 3：Fetcher 實現 ✅

- [ ] Mikanani Fetcher
  - [ ] 實現 `POST /can-handle-subscription`
  - [ ] 在路由中添加新端點
  - [ ] 改進 `POST /subscribe` 邏輯
- [ ] 其他 Fetcher（如有）
  - [ ] 實現相同端點

### Phase 4：API 規格更新 ✅

- [ ] 更新 `docs/api/openapi.yaml`
  - [ ] 更新 `POST /subscriptions` 文檔
  - [ ] 添加 `source_url`, `auto_selected` 字段
- [ ] 更新 `docs/api/fetcher-openapi.yaml`
  - [ ] 添加 `POST /can-handle-subscription` 端點
- [ ] 更新 `docs/api/mikanani-fetcher-openapi.yaml`
  - [ ] 同上

### Phase 5：測試

- [ ] 單元測試
  - [ ] 優先級選擇邏輯
  - [ ] 廣播機制
  - [ ] 超時處理
- [ ] 集成測試
  - [ ] 自動選擇流程
  - [ ] 顯式指定流程
  - [ ] 多 Fetcher 場景
- [ ] 手動測試
  - [ ] 真實環境驗證

---

## 🚀 實施優先級

1. **高優先級（必須）**
   - [ ] Phase 1: DB 遷移
   - [ ] Phase 2: CORE 核心邏輯
   - [ ] Phase 3: Fetcher 實現

2. **中優先級（推薦）**
   - [ ] Phase 4: API 規格更新
   - [ ] Phase 5: 測試

3. **低優先級（之後）**
   - [ ] 監控和告警
   - [ ] 管理儀表板

---

## 📝 相關文檔

- [原始設計](./2026-01-26-subscription-system-redesign.md)
- [API 規格](../API-SPECIFICATIONS.md)
- [架構文檔](../ARCHITECTURE_RSS_SUBSCRIPTIONS.md)

---

**準備開始實施？** 🚀

我建議的順序：
1. 先做 Phase 1 (DB 遷移腳本)
2. 再做 Phase 2 (CORE 代碼)
3. 再做 Phase 3 (Fetcher 適配)
4. 最後更新文檔和測試

你想從哪個部分開始？
