# 多 AI Provider 與 Fallback Chain 設計

**日期**：2026-04-30
**作者**：Bangumi Project
**狀態**：設計確認待實作

---

## 目標

把現有「單組 AI 設定」改造成「多 AI provider，可啟用/停用、可排序、支援 fallback」的系統。當呼叫 AI 時，依排序由優先級高到低嘗試各個 enabled provider；遇到 provider 端故障（網路、5xx、timeout、rate limit）就 fallback 到下一個，直到成功或全部失敗。

## 範圍與選擇

| 主題 | 決議 |
|---|---|
| Provider 形態 | 多協議型（OpenAI-compatible / Anthropic / Gemini …），每個 provider kind 有自己的 client 實作 |
| 本次實作協議 | 只實作 `openai_compatible`，schema 與 trait 為未來新協議預留 |
| Fallback 觸發條件 | 只在 provider 端故障時 fallback：HTTP 5xx、網路錯誤、timeout、rate limit；內容問題（4xx auth/bad request、JSON 解析錯誤、schema 不符）不 fallback |
| 舊資料處理 | Migration 自動把現有 `ai_settings` 那一列搬到新 `ai_providers` 表（name=`"Default"`、is_enabled=true、priority=0），然後 drop 舊表 |
| 觀察性 | log warn + 在 chain 回傳值附帶 `attempts` 陣列；呼叫端可選擇性記錄 |
| 前端 UI | 列表 + 編輯 dialog；拖曳排序；每筆 provider 各自有 test 按鈕 |
| Test 端點 | `POST /ai-providers/{id}/test`（每個 provider 獨立測試），舊的 `POST /ai-settings/test` 移除 |

---

## 1. DB Schema 與 Migration

### 1.1 新表 `ai_providers`

```sql
CREATE TABLE ai_providers (
    id            SERIAL PRIMARY KEY,
    name          TEXT NOT NULL,
    provider_kind TEXT NOT NULL,                     -- 目前固定為 'openai_compatible'
    base_url      TEXT NOT NULL DEFAULT '',
    api_key       TEXT NOT NULL DEFAULT '',
    model_name    TEXT NOT NULL DEFAULT '',
    max_tokens    INT  NOT NULL DEFAULT 4096,
    response_format_mode TEXT NOT NULL DEFAULT 'non_strict',
    is_enabled    BOOL NOT NULL DEFAULT TRUE,
    priority      INT  NOT NULL DEFAULT 0,            -- 數字小者優先
    created_at    TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ai_providers_enabled_priority
    ON ai_providers (is_enabled, priority);
```

### 1.2 Migration up.sql

1. 建立 `ai_providers` 表與索引。
2. `INSERT INTO ai_providers (name, provider_kind, base_url, api_key, model_name, max_tokens, response_format_mode, is_enabled, priority) SELECT 'Default', 'openai_compatible', base_url, api_key, model_name, max_tokens, response_format_mode, TRUE, 0 FROM ai_settings;`（舊表為空時也安全，僅插入 0 列）
3. `DROP TABLE ai_settings;`

### 1.3 Migration down.sql

1. 重新建立 `ai_settings` 表（含舊欄位）。
2. 從 `ai_providers` 取 priority 最小那筆寫入 `ai_settings`（若沒有 row 則 insert 預設空字串）。
3. `DROP TABLE ai_providers;`

### 1.4 注意

- `ai_prompt_settings` 表完全不動（與 provider 無關）。
- `provider_kind` 用字串而非 PostgreSQL ENUM，避免 Diesel ENUM 痛處；應用層用常數字串匹配。
- 未來新增協議若需額外欄位（例 Anthropic 的 `anthropic_version`），屆時再加 `extra_config JSONB` 欄位（YAGNI，本次不加）。

---

## 2. AiClient Trait、錯誤分類與 Provider 工廠

### 2.1 錯誤類型

```rust
#[derive(Debug, Error)]
pub enum AiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("AI returned invalid JSON: {0}")]
    InvalidJson(String),

    #[error("AI settings not configured")]
    NotConfigured,

    /// Provider 端故障：HTTP 5xx、網路錯誤、timeout、rate limit。可 fallback。
    #[error("provider unavailable: {0}")]
    ProviderUnavailable(String),

    /// Provider 正常回應但內容問題（4xx 非 rate limit、auth、bad request）。不 fallback。
    #[error("provider error: {0}")]
    ApiError(String),
}

impl AiError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, AiError::Http(_) | AiError::ProviderUnavailable(_))
    }
}
```

### 2.2 對映現況的調整

- `OpenAiClient` 將 5xx / `reqwest::Error`（含 timeout / 連線錯誤）/ 429 rate limit 統一回 `ProviderUnavailable`；rate limit 的 `[rate_limit_exceeded:N] ...` 訊息前綴保留，呼叫端原本的 retry 秒數解析仍可用。
- 4xx 的 auth / bad request 回 `ApiError`（不 fallback）。
- `InvalidJson` 由呼叫端產生，不會經過 chain；fallback 不適用。

### 2.3 Trait 與工廠

`AiClient` trait 介面保持原樣（`chat_completion` / `chat_completion_structured`）。

```rust
// core-service/src/ai/factory.rs
pub fn build_provider(p: &AiProvider) -> Result<Box<dyn AiClient>, String> {
    match p.provider_kind.as_str() {
        "openai_compatible" => Ok(Box::new(OpenAiClient::new(
            &p.base_url, &p.api_key, &p.model_name,
            p.max_tokens, &p.response_format_mode,
        ))),
        other => Err(format!("unknown provider_kind: {other}")),
    }
}
```

未來新增協議只在 match 加分支。

---

## 3. AiProviderChain（Fallback 核心）

### 3.1 結構

```rust
// core-service/src/ai/chain.rs
pub struct ChainEntry {
    pub id: i32,
    pub name: String,
    pub client: Box<dyn AiClient>,
}

pub struct AttemptRecord {
    pub provider_id: i32,
    pub provider_name: String,
    pub error: String,
}

pub struct AiProviderChain {
    entries: Vec<ChainEntry>,
}
```

### 3.2 Fallback 演算法

```rust
async fn run<F, Fut>(&self, op: F)
    -> Result<(String, Vec<AttemptRecord>), (AiError, Vec<AttemptRecord>)>
where
    F: Fn(&dyn AiClient) -> Fut,
    Fut: std::future::Future<Output = Result<String, AiError>>,
{
    if self.entries.is_empty() {
        return Err((AiError::NotConfigured, vec![]));
    }
    let mut attempts: Vec<AttemptRecord> = Vec::new();
    for entry in &self.entries {
        match op(entry.client.as_ref()).await {
            Ok(resp) => return Ok((resp, attempts)),
            Err(e) if e.is_retryable() => {
                tracing::warn!(
                    provider_id = entry.id,
                    provider = %entry.name,
                    error = %e,
                    "AI provider failed, falling back"
                );
                attempts.push(AttemptRecord {
                    provider_id: entry.id,
                    provider_name: entry.name.clone(),
                    error: e.to_string(),
                });
            }
            Err(e) => {
                attempts.push(AttemptRecord {
                    provider_id: entry.id,
                    provider_name: entry.name.clone(),
                    error: e.to_string(),
                });
                return Err((e, attempts));
            }
        }
    }
    let last = attempts.last().map(|a| a.error.clone()).unwrap_or_default();
    Err((AiError::ProviderUnavailable(format!("all providers failed: {last}")), attempts))
}
```

### 3.3 對外方法

```rust
impl AiProviderChain {
    pub async fn chat_completion(&self, sys: &str, user: &str)
        -> Result<(String, Vec<AttemptRecord>), (AiError, Vec<AttemptRecord>)>;

    pub async fn chat_completion_structured(&self, sys: &str, user: &str, schema: &Value)
        -> Result<(String, Vec<AttemptRecord>), (AiError, Vec<AttemptRecord>)>;
}
```

**取捨說明**：原本構想讓 chain 自身實作 `AiClient` trait，但回傳值需含 `attempts` 會破壞 trait 介面。所有現有呼叫端（parser_generator、filter_generator）只有兩處，故改為 chain 不實作 trait，直接提供方法回元組。

### 3.4 建構函式

```rust
pub fn build_ai_chain(conn: &mut PgConnection) -> Result<Option<AiProviderChain>, String> {
    let providers = ai_providers::table
        .filter(ai_providers::is_enabled.eq(true))
        .order(ai_providers::priority.asc())
        .then_order_by(ai_providers::id.asc())
        .load::<AiProvider>(conn)
        .map_err(|e| e.to_string())?;

    let entries: Vec<ChainEntry> = providers.into_iter()
        .filter(|p| !p.api_key.is_empty() && !p.base_url.is_empty())
        .map(|p| build_provider(&p).map(|client| ChainEntry {
            id: p.id, name: p.name.clone(), client
        }))
        .collect::<Result<_, _>>()?;

    Ok((!entries.is_empty()).then_some(AiProviderChain::new(entries)))
}
```

舊的 `build_ai_client(...)` 從 codebase 全面移除，全部改用 `build_ai_chain(...)`。

呼叫端（`parser_generator.rs` / `filter_generator.rs`）改用 chain 方法並承接 `(String, Vec<AttemptRecord>)` 元組；`attempts` 至少寫入 `tracing::info!`，是否寫入 `pending_ai_results` 視欄位空間決定（本次不擴 schema，先 log 為主，未來可加欄位）。

---

## 4. REST API

### 4.1 移除

- `GET /ai-settings`
- `PUT /ai-settings`
- `POST /ai-settings/test`

`/ai-prompt-settings` 全系列保留不動。

### 4.2 新增

| Method | Path | 用途 |
|---|---|---|
| `GET` | `/ai-providers` | 列出所有 providers（priority 升冪、id 升冪），api_key 遮罩 |
| `POST` | `/ai-providers` | 新增 provider（priority 由後端指派） |
| `GET` | `/ai-providers/{id}` | 取得單一 provider（api_key 遮罩） |
| `PUT` | `/ai-providers/{id}` | 部分更新；api_key 為空字串時保留舊值 |
| `DELETE` | `/ai-providers/{id}` | 硬刪除 |
| `POST` | `/ai-providers/reorder` | body: `{"ordered_ids": [3,1,2]}`，依序寫回 priority = 0,1,2... |
| `POST` | `/ai-providers/{id}/test` | minimal chat completion 測試，回 `{ok: bool, error?: string}` |

### 4.3 資料結構

```jsonc
// CreateAiProviderRequest
{
  "name": "OpenAI 官方",
  "provider_kind": "openai_compatible",
  "base_url": "https://api.openai.com/v1",
  "api_key": "sk-...",
  "model_name": "gpt-4o-mini",
  "max_tokens": 4096,
  "response_format_mode": "non_strict",
  "is_enabled": true
}

// UpdateAiProviderRequest 全部欄位 Option；api_key 為空字串視為「不更新」

// AiProviderResponse（GET / list）
{
  "id": 1, "name": "...", "provider_kind": "openai_compatible",
  "base_url": "...", "api_key": "••••••••",
  "model_name": "...", "max_tokens": 4096,
  "response_format_mode": "non_strict",
  "is_enabled": true, "priority": 0,
  "created_at": "...", "updated_at": "..."
}
```

### 4.4 驗證

- `provider_kind` 必須在白名單（目前只 `openai_compatible`）→ 否則 400。
- `response_format_mode` 必須是 `strict|non_strict|inject_schema` → 否則 400。
- 新增時 `priority` 由後端指派（`max(priority)+1`），不接受 client 傳值。
- 刪除：硬刪除（無外鍵，無歷史價值需求）。

### 4.5 OpenAPI

更新 `docs/api/openapi.yaml`：移除舊 `/ai-settings` 端點；新增 `/ai-providers` 系列端點 schema。

---

## 5. 前端 UI（SettingsPage 內 AI 區塊）

### 5.1 結構

```
frontend/src/pages/settings/
  SettingsPage.tsx                  # 既有，AI 區塊改為渲染 AiProvidersSection
  ai-providers/
    AiProvidersSection.tsx          # 標題 + 新增按鈕 + 列表
    AiProviderList.tsx              # 拖曳排序的列表
    AiProviderRow.tsx               # 單列：name / kind / model / enabled toggle / 編輯 / 測試 / 刪除
    AiProviderEditDialog.tsx        # 新增/編輯共用 dialog
```

### 5.2 列表（AiProviderList）

- 用 `@dnd-kit/core` + `@dnd-kit/sortable`（檢查 package.json，未安裝則 `bun add`）。
- 每列：拖曳 handle、name + provider_kind badge、model_name 副標、is_enabled Switch（即時 PUT）、測試 / 編輯 / 刪除 按鈕。
- 拖曳結束 → 樂觀更新本地順序 → `POST /ai-providers/reorder` → 失敗 rollback + toast。
- 測試按鈕：呼叫期間 spinner，結果 toast。
- `is_enabled=false` 整列 opacity-50。

### 5.3 編輯 Dialog

- **新增**模式：所有欄位空白；`provider_kind` select 目前只有一選項。
- **編輯**模式：載入既有資料；`api_key` 欄位 placeholder `••••••••`、value 為空字串；不輸入 = 保留舊值；`provider_kind` 在編輯模式 disable。
- 欄位：`name` 必填、`provider_kind` 必填、`base_url` 必填、`api_key` 編輯時可選、`model_name` 必填、`max_tokens` number 預設 4096、`response_format_mode` select、`is_enabled` switch。
- 儲存後關閉 dialog 並 refetch list。

### 5.4 Schema 與 API Layer

- `frontend/src/schemas/ai.ts`：新增 `AiProviderSchema`、`CreateAiProviderRequest`、`UpdateAiProviderRequest`（effect schema 風格）。移除舊 `AiSettings` schema。
- `frontend/src/layers/ApiLayer.ts`：移除 `getAiSettings/updateAiSettings/testAiConnection`；新增 `listAiProviders / createAiProvider / updateAiProvider / deleteAiProvider / reorderAiProviders / testAiProvider`。

### 5.5 表現細節

- 列表為空：空狀態 + 「新增第一個 Provider」CTA。
- 顯示優先順序徽章「#1, #2 ...」。

---

## 6. 測試策略

### 6.1 Backend Unit Tests

1. **chain.rs fallback 邏輯**（最重要）：用輕量 `MockAiClient` 預設回傳序列。
   - 第一個成功 → 回該結果，attempts 空
   - 第一個 retryable 失敗、第二個成功 → 回第二個，attempts 含第一個
   - 全部 retryable 失敗 → `ProviderUnavailable`，attempts 含全部
   - 第一個非 retryable 失敗 → 立即停止，後續不嘗試
   - 空 chain → `NotConfigured`
2. `AiError::is_retryable()` 各變體覆蓋。
3. `OpenAiClient` 的 rate limit prefix 解析（既有 `extract_retry_after_secs`）。
4. `build_provider`：未知 provider_kind → Err。

### 6.2 Backend Integration Tests

5. CRUD 全程：`POST → GET list → GET single → PUT → DELETE`。
6. `POST /ai-providers/reorder`：列順序正確。
7. `api_key` GET 必為遮罩；PUT 空字串時不覆蓋。
8. 驗證錯誤：未知 `provider_kind` / 非法 `response_format_mode` → 400。
9. `POST /ai-providers/{id}/test`：至少測「id 不存在 → 404」、「呼叫失敗 → ok:false」。

### 6.3 Migration

10. 手動：在 dev DB 留 `ai_settings` 一行，跑 migration up → 驗證 `ai_providers` 有對應一列且 `name='Default'`、舊表已 drop；migration redo 驗證 down 不 panic。
11. Migration 在無 `ai_settings` 列時也不 panic（INSERT 0 列）。

### 6.4 Frontend

不寫單元測試（與專案現況一致）。手動 QA：

- 新增三組 provider、拖曳排序、改 enabled、編輯（不動 api_key 應保留）、刪除、測試按鈕（成功/失敗 toast）皆正常。
- 設定為空時，呼叫 parser/filter 端點應回 `NotConfigured`。
- Enable 一組可用 provider，跑一次 parser 生成，確認流程恢復。

---

## 影響範圍 / 受影響檔案

**Backend**
- 新檔：`core-service/migrations/<ts>_multi_ai_providers/up.sql` + `down.sql`
- 新檔：`core-service/src/ai/chain.rs`、`core-service/src/ai/factory.rs`
- 修改：`core-service/src/ai/mod.rs`（重新導出）、`core-service/src/ai/openai.rs`（錯誤分類）、`core-service/src/ai/client.rs`（`AiError` 擴充 + `is_retryable`）、`core-service/src/ai/parser_generator.rs`（移除 `build_ai_client`、改用 chain）、`core-service/src/ai/filter_generator.rs`（同）
- 修改：`core-service/src/handlers/ai_settings.rs` → 重命名/重寫為 `ai_providers.rs`；註冊路由
- 修改：`core-service/src/models/db.rs`（移除 `AiSettings`/`UpdateAiSettings`、新增 `AiProvider`/`NewAiProvider`/`UpdateAiProvider`）、`core-service/src/schema.rs`（migration 後 `diesel print-schema` 重生）
- 修改：`docs/api/openapi.yaml`

**Frontend**
- 新檔：`frontend/src/pages/settings/ai-providers/*.tsx`
- 修改：`frontend/src/pages/settings/SettingsPage.tsx`、`frontend/src/schemas/ai.ts`、`frontend/src/layers/ApiLayer.ts`
- 可能新增依賴：`@dnd-kit/core`、`@dnd-kit/sortable`
