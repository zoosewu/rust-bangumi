# AI 自動化 Parser/Filter 生成設計

**日期**：2026-03-04
**狀態**：已核准，準備實作

---

## 概覽

整合 AI（OpenAI-compatible）自動生成 parser 與 filter，取代手動操作流程。
核心目標：新動畫解析失敗或出現 conflict 時，後端自動送 AI 生成設定，使用者在統一待確認區確認或調整後套用。

---

## 架構方案

採用 **方案 A**：AI 模組嵌入 core-service + `pending_ai_results` 獨立表。

- AI Client 以 trait 抽象，初期實作 OpenAI-compatible，方便未來擴充
- 不新增微服務，降低部署複雜度
- 高聚合低耦合：AI 模組、parser 服務、filter 服務各自獨立

---

## 資料庫 Schema

### 新增表

#### `ai_settings`（全局唯一，API 連線設定）
```sql
id              SERIAL PRIMARY KEY,
base_url        TEXT NOT NULL,
api_key         TEXT NOT NULL,
model_name      TEXT NOT NULL,
created_at      TIMESTAMP NOT NULL DEFAULT NOW(),
updated_at      TIMESTAMP NOT NULL DEFAULT NOW()
```

#### `ai_prompt_settings`（全局唯一，Prompt 管理，獨立關注點）
```sql
id                    SERIAL PRIMARY KEY,
fixed_parser_prompt   TEXT,   -- 可為空；程式常數定義預設值供 revert
fixed_filter_prompt   TEXT,   -- 同上
custom_parser_prompt  TEXT,   -- 預設空，使用者自由調整
custom_filter_prompt  TEXT,   -- 預設空，使用者自由調整
created_at            TIMESTAMP NOT NULL DEFAULT NOW(),
updated_at            TIMESTAMP NOT NULL DEFAULT NOW()
```

#### `pending_ai_results`（統一待確認佇列）
```sql
id                  SERIAL PRIMARY KEY,
result_type         TEXT NOT NULL CHECK (result_type IN ('parser', 'filter')),
source_title        TEXT NOT NULL,       -- 觸發生成的動畫標題
generated_data      JSONB,               -- AI 返回的 parser/filter 設定（失敗時為 NULL）
status              TEXT NOT NULL CHECK (status IN ('generating', 'pending', 'confirmed', 'failed')),
error_message       TEXT,                -- AI 呼叫失敗原因
raw_item_id         INT REFERENCES raw_items(item_id) ON DELETE SET NULL,
used_fixed_prompt   TEXT NOT NULL,       -- 生成當下固定 prompt 快照
used_custom_prompt  TEXT,                -- 生成當下自訂 prompt 快照
expires_at          TIMESTAMP,           -- confirm/reject 後設為 NOW()+7days，排程清除
created_at          TIMESTAMP NOT NULL DEFAULT NOW(),
updated_at          TIMESTAMP NOT NULL DEFAULT NOW()
```

### 修改現有表

#### `title_parsers` 新增欄位
```sql
pending_result_id   INT REFERENCES pending_ai_results(id) ON DELETE SET NULL
-- NULL     = 已確認 parser
-- NOT NULL = AI 生成但尚未確認
```

#### `filter_rules` 新增欄位
```sql
pending_result_id   INT REFERENCES pending_ai_results(id) ON DELETE SET NULL
-- NULL     = 已確認 filter rule
-- NOT NULL = AI 生成但尚未確認（一筆 pending 可對應多筆 rule）
```

### 移除 / 重寫

- Migration seed 移除 `Catch-All 全匹配` parser（所有解析失敗均進入 AI 流程）
- `subscription_conflicts` 表保留（處理 fetcher 選擇衝突，不同業務域）
- `anime_link_conflicts` 表保留（作為 AI filter 生成觸發依據）
- 前端衝突頁面（`/conflicts`）移除，由 `/pending` 取代

### 確認/拒絕資料一致性

| 事件 | parser/filter 操作 | pending_ai_results 操作 | 後續觸發 |
|------|-------------------|------------------------|---------|
| 確認 parser | `pending_result_id = NULL` | `status = confirmed`，`expires_at = NOW()+7d` | re-run 所有 raw_items 解析 |
| 拒絕 parser | DELETE title_parser 記錄 | `status = failed`，`expires_at = NOW()+7d` | — |
| 確認 filter | 所有關聯記錄 `pending_result_id = NULL` | `status = confirmed`，`expires_at = NOW()+7d` | 重新計算 filter |
| 拒絕 filter | DELETE 所有關聯 filter_rule 記錄 | `status = failed`，`expires_at = NOW()+7d` | — |

---

## 後端 AI 模組

### 模組結構

```
core-service/src/ai/
├── mod.rs
├── client.rs           ← AiClient trait
├── openai.rs           ← OpenAI-compatible HTTP 實作
├── prompts.rs          ← Prompt 組裝（fixed + custom 合併）
├── parser_generator.rs ← Parser 生成業務流程
└── filter_generator.rs ← Filter 生成業務流程
```

### AiClient Trait

```rust
#[async_trait]
pub trait AiClient: Send + Sync {
    async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, AiError>;
}
```

### Prompt 組裝規則

```
final_system_prompt = fixed_prompt（可空）
final_user_prompt   = "動畫標題：{title}\n\n" + custom_prompt（可空）
```

### 生成流程（parser_generator / filter_generator）

1. 讀取 `ai_settings`（base_url, api_key, model_name）
2. 讀取 `ai_prompt_settings`，組裝 prompt，快照記錄
3. 寫入 `pending_ai_results`（`status = generating`）
4. 呼叫 `AiClient::chat_completion`
5. 成功：解析 JSON → 驗證欄位 → 寫入 parser/filter（`pending_result_id` 指向此記錄）→ `status = pending`
6. 失敗：`status = failed`，寫入 `error_message`，仍建立待確認記錄供使用者手動重送

### AI 觸發點

| 觸發時機 | 觸發位置 | 方式 |
|---------|---------|------|
| raw_item 所有 parser 均失敗 | `services/title_parser.rs` | 非同步背景 |
| conflict 被標記 | `services/conflict_detection.rs` | 非同步背景 |
| 訂閱建立 Step 2 解析失敗 | `handlers/subscriptions.rs` | 同步等待 |
| 訂閱建立 Step 3 有 conflict | `handlers/subscriptions.rs` | 同步等待 |

### 新增 API Endpoints

```
# AI 連線設定
GET  /ai-settings
PUT  /ai-settings

# Prompt 設定
GET  /ai-prompt-settings
PUT  /ai-prompt-settings
POST /ai-prompt-settings/revert-parser   ← 回復 fixed_parser_prompt 預設值
POST /ai-prompt-settings/revert-filter   ← 回復 fixed_filter_prompt 預設值

# 待確認管理
GET  /pending-ai-results                 ← 列表，支援 ?type=parser|filter&status=pending|failed
GET  /pending-ai-results/:id
PUT  /pending-ai-results/:id             ← 手動編輯 generated_data
POST /pending-ai-results/:id/confirm     ← 確認（body: { level, target_id }）
POST /pending-ai-results/:id/reject      ← 拒絕
POST /pending-ai-results/:id/regenerate  ← 臨時 prompt 重新生成（body: { custom_prompt }）
```

### Parser 預覽比較（利用現有機制）

- **不含待確認項目**：預覽時排除 `pending_result_id = 當前 id` 的 parser
- **含待確認項目**：正常預覽（未確認 parser 已在 DB，engine 直接跑）
- 複用現有 `POST /parsers/preview` endpoint

---

## 資料流

### 1. 新動畫解析流程

```
raw_item 進入
    ↓
title_parser_service.parse_title()
（含 is_confirmed=true 和 pending_result_id IS NOT NULL 的 parser）
    ↓
解析成功 → 正常流程（filter → conflict 檢查）
    ↓
解析失敗 → 觸發 parser_generator（背景非同步）
    → pending_ai_results 建立（status=generating）
    → AI 呼叫
    → 成功：title_parsers 新增未確認記錄，status=pending
    → 失敗：status=failed，等待使用者手動重送
```

### 2. Conflict 觸發 Filter 生成流程

```
conflict_detection_service.detect_and_mark_conflicts()
    ↓
有新 conflict → 觸發 filter_generator（背景非同步）
    → pending_ai_results 建立（status=generating）
    → AI 呼叫
    → 成功：filter_rules 新增未確認記錄（pending_result_id 指向此記錄）
    → 失敗：status=failed
```

### 3. 使用者確認後的迭代流程（Parser）

```
使用者確認 pending parser
    ↓
title_parsers.pending_result_id = NULL
pending_ai_results.status = confirmed，expires_at = NOW()+7d
    ↓
re-run 所有 raw_items
    ↓
若仍有失敗 → 取第一筆失敗 → 觸發新一輪 AI 生成
    ↓
（迭代直到無失敗）
```

### 4. 新增訂閱 Wizard 流程

```
Step 1：URL 輸入 + Fetcher 選擇 + 基本設定
    ↓
Step 2：立即抓取 → 解析
    ├─ 成功：顯示解析結果，使用者確認 → Step 3
    └─ 失敗：同步 AI 生成 → 顯示 AiResultPanel → 使用者確認/編輯 → Step 3
    ↓
Step 3：套用 Filter → 檢查 Conflict
    ├─ 無 conflict：完成建立訂閱
    └─ 有 conflict：同步 AI 生成 → 顯示 AiResultPanel → 使用者確認/編輯 → 完成
```

---

## 前端設計

### 頁面與路由變更

| 變更 | 路由 | 說明 |
|------|------|------|
| 新增 | `/pending` | 統一待確認頁面 |
| 新增 | `/settings` | AI 設定 + Prompt 設定 |
| 移除 | `/conflicts` | 由 `/pending` 取代 |
| 修改 | `/subscriptions` | 新增流程改為三步驟 Wizard |

### 共用組件：`AiResultPanel`

所有待確認結果（parser 和 filter）的外層容器，共用以下功能：
- 來源標題 + 狀態顯示
- 臨時自訂 Prompt textarea + \[重新生成\] 按鈕
- 預覽比較區（不含 vs 含當前待確認項目）
- 層級選擇（Global / AnimeWork / Subscription）+ 目標 ID 選擇
- \[拒絕\] + \[確認套用\] 按鈕

編輯區各自獨立，嵌入 `AiResultPanel` 內：
- Parser → 複用現有 `ParserForm`
- Filter → 複用現有 `FilterRuleEditor`

### 待確認頁面（`/pending`）

- Tab：全部 / Parser / Filter
- 列表欄位：類型、來源標題、狀態、建立時間
- 狀態 `generating`：顯示 spinner
- 狀態 `failed`：顯示錯誤，可直接填臨時 prompt 重試
- 點擊展開 `AiResultPanel`

### 訂閱 Wizard

Step 2 / Step 3 的 AI 生成為**同步等待**（spinner），結果直接使用 `AiResultPanel` 呈現，確認後方可進入下一步。Wizard 確認等同於全局確認，直接寫入 DB。

### `/settings` 頁面

三個 Section 依序排列：
1. **AI 連線設定**：Base URL、API Key（遮罩）、Model Name、\[測試連線\] 按鈕
2. **Parser Prompt 設定**：固定 Prompt textarea + \[Revert\]、自訂 Prompt textarea
3. **Filter Prompt 設定**：固定 Prompt textarea + \[Revert\]、自訂 Prompt textarea

---

## 移除項目

| 項目 | 處理方式 |
|------|---------|
| Catch-All 全匹配 parser（migration seed） | 從 seed SQL 刪除 |
| ConflictsPage（`/conflicts`） | 刪除頁面，路由改為 redirect 至 `/pending` |
| 衝突頁面導覽連結 | 改為「待確認」 |

---

## 固定 Prompt 預設值

固定 Prompt 預設值以 Rust 常數定義於程式碼，`revert` 時寫回 DB：

```rust
pub const DEFAULT_FIXED_PARSER_PROMPT: &str = "...";
pub const DEFAULT_FIXED_FILTER_PROMPT: &str = "...";
```

AI 回傳的 JSON 格式需符合現有 `TitleParser` / `FilterRule` 的欄位定義。

---

## 排程清除

現有排程器（scheduler）新增任務：
- 定期（每小時）刪除 `expires_at < NOW()` 的 `pending_ai_results` 記錄
- 刪除前確認相關 parser/filter 的 `pending_result_id` 已為 NULL（正常情況下已完成）
