# 訂閱系統重設計方案

**日期：** 2026-01-26
**狀態：** 設計階段
**版本：** Final Design Based on Discussion

---

## 📋 設計原則

1. **靈活性** - CORE 可顯式指定 Fetcher，也可自動選擇
2. **簡潔性** - 優先級使用整數，決策結果為布林值
3. **確定性** - Fetcher 使用 REGEX/條件判斷，無模糊性
4. **單選制** - 每個訂閱只分配給一個 Fetcher
5. **前向設計** - 系統未發布，直接使用最新架構，無遺留支持

---

## 🎯 核心流程

### 場景 1：自動選擇 Fetcher

```
1. 用戶創建訂閱（不指定 Fetcher）
   POST /subscriptions
   {
     "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215"
   }

2. CORE 廣播給所有已啟用的 Fetcher
   並發請求到每個 Fetcher：

   POST /can-handle-subscription
   {
     "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215"
   }

3. 每個 Fetcher 回應

   Mikanani Fetcher:
   { "can_handle": true }

   TwitterFetcher:
   { "can_handle": false }

   Generic Fetcher:
   { "can_handle": false }

4. CORE 根據優先級選擇

   接受列表：[Mikanani (priority=80)]

   選擇：Mikanani

   分配訂閱給 Mikanani

5. CORE 通知 Fetcher

   POST /mikanani:8001/subscribe
   {
     "subscription_id": 1,
     "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215",
     "config": { ... }
   }
```

### 場景 2：顯式指定 Fetcher

```
1. 用戶創建訂閱（指定 Fetcher）
   POST /subscriptions
   {
     "source_url": "https://...",
     "fetcher_id": 2  # 明確指定
   }

2. CORE 驗證 Fetcher 存在且已啟用

3. CORE 直接通知指定的 Fetcher

   POST /fetcher-2:port/subscribe
   {
     "subscription_id": 1,
     "source_url": "https://...",
     "config": { ... }
   }

   （跳過廣播和優先級比較）
```

---

## 📊 數據庫設計

### 修改 `fetcher_modules` 表

```sql
ALTER TABLE fetcher_modules ADD COLUMN (
  priority INTEGER NOT NULL DEFAULT 50
  COMMENT '整數優先級，值越大優先級越高。範例: 10, 50, 100'
);

-- 優先級建議值
-- 通用 Fetcher: 10
-- 專用 Fetcher（如 Mikanani）: 80-100
-- 備選 Fetcher: 30-50
```

### 新增/修改 `subscriptions` 表

```sql
-- 改名（邏輯重構）
ALTER TABLE rss_subscriptions RENAME TO subscriptions;

-- 修改欄位
ALTER TABLE subscriptions RENAME COLUMN rss_url TO source_url;
ALTER TABLE subscriptions MODIFY source_url VARCHAR(2048) NOT NULL;

-- 添加新欄位
ALTER TABLE subscriptions ADD COLUMN (
  source_type VARCHAR(50) NOT NULL DEFAULT 'rss'
  COMMENT '源類型: rss, http, custom, etc.',

  assignment_status VARCHAR(20) NOT NULL DEFAULT 'pending'
  COMMENT 'pending, assigned, failed, inactive',

  assigned_at TIMESTAMP NULL
  COMMENT '分配給 Fetcher 的時間',

  auto_selected BOOLEAN NOT NULL DEFAULT false
  COMMENT '是否通過自動選擇分配'
);

-- 唯一約束改動
ALTER TABLE subscriptions DROP CONSTRAINT subscriptions_fetcher_id_rss_url_key;
ALTER TABLE subscriptions ADD CONSTRAINT
  subscriptions_source_url_fetcher_id_key
  UNIQUE(source_url, fetcher_id);
```

### 新表：`subscription_selections`（可選追蹤）

```sql
CREATE TABLE subscription_selections (
  selection_id SERIAL PRIMARY KEY,
  subscription_id INTEGER NOT NULL REFERENCES subscriptions(subscription_id),
  fetcher_id INTEGER NOT NULL REFERENCES fetcher_modules(fetcher_id),

  -- 廣播結果追蹤
  can_handle BOOLEAN NOT NULL,
  candidate_priority INTEGER,  -- 選擇時該 Fetcher 的優先級

  -- 決策信息
  selected BOOLEAN NOT NULL,   -- 是否被選中
  selection_reason VARCHAR(100),  -- "highest_priority", "explicit", etc.
  selected_at TIMESTAMP,

  created_at TIMESTAMP NOT NULL,

  UNIQUE(subscription_id, fetcher_id)
);
```

---

## 🔌 API 規格

### CORE Service

#### 1. 創建訂閱

```yaml
POST /subscriptions

Request:
  source_url*: string (必填)
    示例: "https://mikanani.me/RSS/Bangumi?bangumiId=3215"

  fetcher_id?: integer (可選)
    如果提供，則直接分配給此 Fetcher，跳過廣播

  name?: string
    訂閱名稱，如 "Attack on Titan Season 4"

  description?: string
    詳細描述

  fetch_interval_minutes?: integer (預設: 60)
    抓取間隔

  config?: object
    Fetcher 特定配置

  auto_assign?: boolean (預設: true)
    是否自動選擇 Fetcher（當 fetcher_id 為空時）

Response 201:
  subscription_id: integer
  source_url: string
  fetcher_id: integer (分配的 Fetcher)
  assignment_status: string ("pending", "assigned", "failed")
  auto_selected: boolean
  created_at: timestamp
```

#### 2. 列出訂閱

```yaml
GET /subscriptions?status=assigned&fetcher_id=1

Response 200:
  - subscription_id
  - source_url
  - fetcher_id
  - assignment_status
  - auto_selected
  - created_at
```

#### 3. 獲取訂閱詳情

```yaml
GET /subscriptions/{subscription_id}

Response 200:
  subscription_id
  source_url
  source_type
  fetcher_id
  assignment_status
  auto_selected
  assigned_at
  config
  ...
```

#### 4. 廣播給 Fetcher（內部端點）

```yaml
POST /subscriptions/{subscription_id}/broadcast

# CORE 內部流程，不暴露給外部
# 並發調用所有已啟用的 Fetcher
```

---

### Fetcher Service

#### 1. 判斷是否能處理（新端點）

```yaml
POST /can-handle-subscription

Request:
  source_url*: string
    要判斷的 URL

Response 200:
  can_handle*: boolean
    true: 此 Fetcher 可以處理
    false: 此 Fetcher 不能處理

Response 400/500:
  error: string
```

**Fetcher 實現邏輯示例：**

```rust
// Mikanani Fetcher
pub async fn can_handle_subscription(
    Json(payload): Json<CanHandleRequest>,
) -> Json<CanHandleResponse> {
    let can_handle = payload.source_url.contains("mikanani.me");
    Json(CanHandleResponse { can_handle })
}
```

#### 2. 接收訂閱通知（既有端點改進）

```yaml
POST /subscribe

Request:
  subscription_id*: integer
  source_url*: string
  source_type*: string ("rss", "http", etc.)
  name?: string
  config?: object

Response 200:
  status: string ("accepted", "processing")
  message: string

Response 400/500:
  error: string
```

---

## 🏗️ CORE 實現邏輯

### 偽代碼：創建訂閱

```rust
async fn create_subscription(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> Result<SubscriptionResponse> {
    // 1. 儲存訂閱到數據庫
    let subscription = db::insert_subscription(&payload);

    // 2. 決定 Fetcher
    let assigned_fetcher_id = if let Some(fetcher_id) = payload.fetcher_id {
        // 顯式指定
        verify_fetcher_enabled(fetcher_id)?;
        fetcher_id
    } else {
        // 自動選擇
        let selected_id = auto_select_fetcher(&subscription).await?;
        selected_id
    };

    // 3. 通知 Fetcher
    notify_fetcher(assigned_fetcher_id, &subscription).await?;

    // 4. 更新訂閱狀態
    db::update_subscription_status(
        subscription.id,
        "assigned",
        assigned_fetcher_id
    );

    Ok(response)
}

async fn auto_select_fetcher(subscription: &Subscription) -> Result<i32> {
    // 1. 獲取所有已啟用的 Fetcher
    let fetchers = db::get_enabled_fetchers();

    // 2. 並發廣播給所有 Fetcher
    let futures = fetchers.iter().map(|f| {
        broadcast_can_handle(f.id, &subscription.source_url)
    });

    let responses = futures::future::join_all(futures).await;

    // 3. 過濾能處理的 Fetcher
    let candidates: Vec<_> = fetchers
        .iter()
        .zip(responses.iter())
        .filter(|(_, resp)| resp.can_handle)
        .collect();

    if candidates.is_empty() {
        return Err("No fetcher can handle this URL".into());
    }

    // 4. 按優先級排序，選擇最高的
    let selected = candidates
        .iter()
        .max_by_key(|(fetcher, _)| fetcher.priority)
        .map(|(fetcher, _)| fetcher.id)
        .unwrap();

    Ok(selected)
}

async fn broadcast_can_handle(
    fetcher_id: i32,
    source_url: &str,
) -> CanHandleResponse {
    let fetcher = db::get_fetcher(fetcher_id);
    let client = reqwest::Client::new();

    match client.post(format!("http://{}:{}/can-handle-subscription",
                              fetcher.host, fetcher.port))
        .json(&CanHandleRequest { source_url })
        .send()
        .await {
        Ok(resp) => resp.json().await.unwrap_or(CanHandleResponse {
            can_handle: false
        }),
        Err(_) => CanHandleResponse { can_handle: false }
    }
}
```

---

## 🔄 Fetcher 設計要求

### Mikanani Fetcher 檢查清單

- [ ] 實現 `POST /can-handle-subscription` 端點
- [ ] 使用 REGEX 判斷：`source_url.contains("mikanani.me")`
- [ ] 返回布林結果（不是可信度或其他模糊概念）
- [ ] 實現 `POST /subscribe` 端點改進
  - 接收 `subscription_id`
  - 異步處理（不需要立即回覆結果）
  - 定期主動向 CORE 回報進度（未來版本）

### 通用 Fetcher 示例

```rust
// 檢查是否能處理
pub fn can_handle_url(source_url: &str) -> bool {
    // 使用正則表達式或精確條件
    // 例如：
    // - Mikanani: contains("mikanani.me")
    // - Twitter: contains("twitter.com") && contains("anime")
    // - HTTP: source_url.starts_with("http")
    // - RSS: source_url.ends_with(".xml") || contains("feed")
}
```

---

## 📈 優先級參考值

| Fetcher 類型 | 優先級 | 備註 |
|------------|-------|------|
| 通用/泛型 | 10 | 最後的備選方案 |
| HTTP/通用爬蟲 | 30 | 能處理多數網站 |
| 特定網站（如 Twitter） | 60 | 有專門處理邏輯 |
| 專用爬蟲（Mikanani） | 80 | 特別優化 |
| 定製爬蟲 | 100 | 最高優先級 |

---

## 🔄 關鍵決策點

### Q1: 如果廣播時 Fetcher 沒有回應怎麼辦？

**A:** 設定超時（如 3 秒），視為不能處理。

```rust
let response = tokio::time::timeout(
    Duration::from_secs(3),
    broadcast_can_handle(fetcher_id, url)
).await
.unwrap_or(CanHandleResponse { can_handle: false });
```

### Q2: 如果沒有任何 Fetcher 能處理怎麼辦？

**A:** 返回 400 錯誤，提示沒有合適的 Fetcher。使用者需要：
- 手動指定 Fetcher（使用 `fetcher_id`）
- 或等待新的 Fetcher 加入系統

### Q3: Fetcher 可以動態修改優先級嗎？

**A:** 否。優先級由系統管理員設定。Fetcher 只能回答 "我能/不能處理"。

### Q4: 如果多個訂閱使用同一 URL 怎麼辦？

**A:** 每個訂閱記錄獨立，分別進行廣播和選擇。

---

## 🚀 實施順序

### Phase 1：數據庫和基礎設施

- [ ] 修改 `fetcher_modules` 表，添加 `priority`
- [ ] 重命名/修改 `subscriptions` 表
- [ ] 創建 `subscription_selections` 追蹤表
- [ ] 數據遷移（設置默認優先級值）

### Phase 2：CORE Service 實現

- [ ] 實現 `auto_select_fetcher()` 邏輯
- [ ] 實現廣播機制
- [ ] 修改 `POST /subscriptions` 端點
- [ ] 添加 Fetcher 健康檢查機制

### Phase 3：Fetcher 適配

- [ ] 實現 `POST /can-handle-subscription` 端點
- [ ] 各 Fetcher 實現 URL 判斷邏輯
- [ ] 改進 `POST /subscribe` 端點

### Phase 4：測試和驗證

- [ ] 單元測試
- [ ] 集成測試
- [ ] 手動測試不同場景

---

## 📝 API 規格更新

需要更新的文檔：

- [ ] `docs/api/openapi.yaml` - 新增 `can-handle-subscription`
- [ ] `docs/api/fetcher-openapi.yaml` - 更新 Fetcher API
- [ ] `docs/api/mikanani-fetcher-openapi.yaml` - 更新 Mikanani 規格
- [ ] `docs/ARCHITECTURE_RSS_SUBSCRIPTIONS.md` - 更新架構文檔

---

## ✅ 決策確認

- [x] 支持顯式指定 Fetcher
- [x] 優先級使用整數
- [x] 布林結果（能/不能）
- [x] Fetcher 使用 REGEX 決策
- [x] 單選制
- [x] 前向設計（不考慮舊版本）
- [x] 廣播機制
- [x] CORE 選最高優先級

---

**下一步：** 根據這個設計開始實施，或進一步細化任何方面？
