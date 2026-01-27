# 架構頭腦風暴：訂閱系統重構設計

**日期：** 2026-01-26
**主題：** 改進訂閱系統的靈活性和自動化

---

## 🎯 核心問題與目標

### 當前問題

1. **命名誤導**
   - API 使用 `rss_url` 和 `rss_subscriptions` 表名
   - 實際上應該支持通用 URL（RSS、通用网址等）
   - 導致概念混淆

2. **訂閱流程過於耦合**
   - 創建訂閱時必須指定 `fetcher_id`
   - 使用者需要知道 Fetcher 的內部 ID
   - 無法自動匹配合適的 Fetcher

3. **缺乏 Fetcher 優先級機制**
   - 多個 Fetcher 支持同一 URL 時無優先級
   - 衝突時的解決方案不明確
   - 沒有 priority 字段存儲優先級

4. **Fetcher 過濾邏輯重複**
   - 只有 Mikanani Fetcher 在代碼中判斷 `can_handle_url()`
   - Core Service 應該讓所有 Fetcher 自己決定

---

## ✨ 提議的改進方案

### 1. 重命名與概念澄清

#### API 規格層面
```yaml
# 舊（誤導性）
POST /subscriptions
{
  "rss_url": "https://...",
  "fetcher_id": 123
}

# 新（清晰）
POST /subscriptions
{
  "source_url": "https://...",  # 不限於 RSS
  "auto_select": true,           # 自動選擇 Fetcher
  "preferred_fetcher": "mikanani" # 可選偏好
}
```

#### 表名和字段重命名
```sql
-- 舊
rss_subscriptions:
  - rss_url VARCHAR

-- 新
subscriptions:
  - source_url VARCHAR(2048)       # 通用 URL
  - source_type VARCHAR(50)        # 'rss', 'http', 'custom'
  - fetcher_id INTEGER NULLABLE    # 可為空
```

### 2. 廣播機制：Fetcher 自我篩選

#### 流程設計
```
┌─────────────────────────────────────────────────┐
│ 1. 使用者創建訂閱 (只需提供 URL)                │
│    POST /subscriptions                           │
│    { "source_url": "https://..." }              │
└─────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────┐
│ 2. CORE 廣播給所有已啟用的 FETCHER              │
│    POST /fetchers/*/subscribe                    │
│    {                                             │
│      "subscription_id": 1,                       │
│      "source_url": "https://...",               │
│      "metadata": { ... }                         │
│    }                                             │
└─────────────────────────────────────────────────┘
                        ↓
         ┌──────────────┬──────────────┐
         ↓              ↓              ↓
    ┌────────┐    ┌────────┐    ┌────────┐
    │Fetcher1│    │Fetcher2│    │Fetcher3│
    │Mikanani│    │Anilist │    │Custom  │
    └────────┘    └────────┘    └────────┘
         │              │              │
         ├─ Can I       ├─ Can I       ├─ Can I
         │  handle?     │  handle?     │  handle?
         │              │              │
         YES            NO             YES
         │                             │
         └──────────────┬──────────────┘
                        ↓
        3. 多個 FETCHER 願意處理
           根據優先級選擇一個
```

#### Fetcher API 新端點

```yaml
POST /subscribe
Request:
{
  "subscription_id": 1,
  "source_url": "https://...",
  "source_type": "rss",
  "name": "Attack on Titan 2025",
  "config": { ... }
}

Response:
{
  "status": "accepted" | "declined" | "maybe",  # 是否接受
  "reason": "...",                              # 原因
  "confidence": 0.95                            # 可信度 (0-1)
}
```

### 3. 優先級系統

#### 數據庫設計

```sql
-- 添加到 fetcher_modules 表
ALTER TABLE fetcher_modules ADD COLUMN (
  priority INTEGER DEFAULT 50,      -- 0-100，越高優先級越高
  pattern_type VARCHAR(50),          -- 匹配類型: 'domain', 'regex', 'exact'
  supported_patterns TEXT[],         # 支持的模式列表
  updated_at TIMESTAMP
);

-- 新表：subscription_assignments
CREATE TABLE subscription_assignments (
  assignment_id SERIAL PRIMARY KEY,
  subscription_id INTEGER NOT NULL REFERENCES subscriptions,
  fetcher_id INTEGER NOT NULL REFERENCES fetcher_modules,
  assignment_status VARCHAR(20) NOT NULL,  -- 'assigned', 'rejected', 'pending'
  confidence REAL,                         -- Fetcher 的可信度
  created_at TIMESTAMP NOT NULL,
  updated_at TIMESTAMP NOT NULL,
  UNIQUE(subscription_id, fetcher_id)
);
```

#### 優先級計算邏輯

```
優先級分數 = (Fetcher優先級) × (URL匹配可信度)

例如：
- Mikanani (priority=80) 匹配 mikanani.me (confidence=0.95) = 76
- Anilist  (priority=50) 匹配通用 URL (confidence=0.50) = 25

選擇 Mikanani
```

### 4. CORE 端廣播邏輯

```rust
// 偽代碼
async fn handle_new_subscription(subscription: Subscription) {
    // 1. 查詢所有已啟用的 Fetcher
    let fetchers = fetch_enabled_fetchers();

    // 2. 並發廣播給所有 Fetcher
    let responses = broadcast_to_all_fetchers(subscription, fetchers);

    // 3. 評估回應
    let candidates = responses
        .iter()
        .filter(|r| r.status == "accepted" || r.status == "maybe")
        .map(|r| {
            score: r.fetcher.priority * r.confidence,
            fetcher_id: r.fetcher.id
        })
        .collect();

    // 4. 選擇最高分的 Fetcher
    if let Some(winner) = candidates.max_by_score() {
        assign_subscription(subscription, winner);
        // 通知其他 Fetcher：訂閱已被分配
        notify_others(subscription, winner);
    }
}
```

---

## 📊 對比分析

### 當前架構 vs 提議架構

| 方面 | 當前 | 提議 |
|------|------|------|
| **訂閱 URL** | 必須指定 `fetcher_id` | 自動匹配 |
| **多 Fetcher 支持** | 不支持 | 支持，有優先級 |
| **Fetcher 自主性** | 被動接收 | 主動判斷（能否處理） |
| **命名** | `rss_url` | `source_url` |
| **可擴展性** | 低 | 高 |
| **使用者體驗** | 複雜（需知道 ID） | 簡單（只需 URL） |

---

## 🔄 遷移策略

### Phase 1：添加新字段（向後兼容）
```sql
ALTER TABLE fetcher_modules ADD COLUMN priority INTEGER DEFAULT 50;
ALTER TABLE rss_subscriptions ADD COLUMN source_type VARCHAR(50) DEFAULT 'rss';
ALTER TABLE rss_subscriptions RENAME COLUMN rss_url TO source_url;
```

### Phase 2：實現廣播機制
- 保留現有 `fetcher_id` 欄位（可為空）
- 添加 `/subscriptions/broadcast` 端點
- Fetcher 實現新的 `/subscribe` 端點

### Phase 3：遷移舊訂閱
- 如果 `fetcher_id` 不為空，保持原有行為
- 如果 `fetcher_id` 為空，執行新的廣播流程

### Phase 4：棄用舊系統
- 文檔標記 `fetcher_id` 為棄用
- 建議使用新的自動匹配流程

---

## 🎯 實施清單

### 數據庫更改
- [ ] 添加 `priority` 到 `fetcher_modules`
- [ ] 添加 `pattern_type` 和 `supported_patterns`
- [ ] 重命名 `rss_url` → `source_url`
- [ ] 添加 `source_type` 欄位
- [ ] 創建 `subscription_assignments` 表
- [ ] 創建衝突解決表（如需要）

### Core Service 更改
- [ ] 新增廣播端點: `POST /subscriptions/broadcast`
- [ ] 實現優先級選擇邏輯
- [ ] 實現衝突解決機制
- [ ] 添加 Fetcher 健康檢查

### Fetcher 端更改
- [ ] 實現新的 `/subscribe` 端點
- [ ] 返回 `accepted` / `declined` 狀態
- [ ] 返回可信度分數
- [ ] 實現 URL 模式匹配邏輯

### API 規格更新
- [ ] 更新 OpenAPI 規格
- [ ] 棄用舊的 `/subscriptions` 實現
- [ ] 新增 `/subscriptions/broadcast` 文檔

### 文檔更新
- [ ] 更新架構文檔
- [ ] 添加 Fetcher 開發指南
- [ ] 添加遷移指南

---

## 💡 進階功能（可選）

### 1. 動態優先級調整
```sql
fetcher_modules:
  dynamic_priority_enabled: BOOLEAN
  min_priority: INTEGER
  max_priority: INTEGER

-- 基於成功率動態調整優先級
UPDATE fetcher_modules
SET priority = priority + 5
WHERE success_rate > 0.95
```

### 2. 訂閱策略
```json
{
  "strategy": "single_best" | "multi_fetch" | "priority_list",
  "allow_multiple_fetchers": false,
  "fallback_behavior": "use_next_best" | "fail" | "manual_review"
}
```

### 3. 分析和指標
```sql
-- Fetcher 表現
SELECT fetcher_id,
       COUNT(*) as total_subscriptions,
       COUNT(CASE WHEN status='success' THEN 1 END) as success_count,
       AVG(fetch_duration_ms) as avg_duration
FROM subscription_metrics
GROUP BY fetcher_id
```

---

## 🔐 安全性考慮

1. **驗證 Fetcher 身份**
   - 確保只有已註冊的 Fetcher 可以回應廣播

2. **優先級操縱防護**
   - 限制 Fetcher 自修改優先級的能力

3. **訂閱驗證**
   - 驗證 URL 有效性和安全性

---

## 📈 效益分析

### 優點
1. **靈活性** - 支持任意 URL 格式
2. **自動化** - 自動選擇最合適的 Fetcher
3. **擴展性** - 新 Fetcher 無需修改 Core
4. **可靠性** - 優先級和衝突解決機制
5. **用戶體驗** - 簡化的 API（只需 URL）

### 成本
1. **複雜度增加** - 需要廣播和優先級邏輯
2. **數據庫變更** - 需要遷移現有數據
3. **向後兼容性** - 需要支持舊系統一段時間

---

## 🚀 下一步

1. **評估**
   - 確認這個方向是否符合項目目標
   - 討論優先級系統的具體實現

2. **設計詳化**
   - 編寫詳細的 ER 圖
   - 定義完整的 API 規格

3. **實施規劃**
   - 分解成具體任務
   - 估算開發時間

---

**相關文檔：**
- [API 規格文檔](./API-SPECIFICATIONS.md)
- [RSS 訂閱架構](./ARCHITECTURE_RSS_SUBSCRIPTIONS.md)
- [Core Service 文檔](../core-service/README.md)
