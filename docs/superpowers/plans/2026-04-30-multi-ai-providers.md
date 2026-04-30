# 多 AI Provider 與 Fallback Chain 實作計畫

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 將「單組 AI 設定」改為「多 provider 列表，支援啟用/停用、排序、fallback chain」，並把現有 `ai_settings` 一筆資料無縫搬遷到新 `ai_providers` 表。

**Architecture:** 新表 `ai_providers` 取代 `ai_settings`；定義 `AiError::ProviderUnavailable` 與 `is_retryable()` 區分可 fallback 錯誤；新增 `AiProviderChain` 集中 fallback 邏輯，呼叫端從 `build_ai_client()` 改用 `build_ai_chain()`；REST API 從 `/ai-settings` 換成 `/ai-providers` 系列；前端用列表 + dialog + 拖曳排序。

**Tech Stack:** Rust (Axum, Diesel 2.1, tokio, tracing, async-trait, thiserror), PostgreSQL, React + TypeScript (Effect schema, @dnd-kit), Bun。

**Spec:** `docs/superpowers/specs/2026-04-30-multi-ai-providers-design.md`

---

## File Structure

**Create**
- `core-service/migrations/2026-04-30-000000-multi-ai-providers/up.sql`
- `core-service/migrations/2026-04-30-000000-multi-ai-providers/down.sql`
- `core-service/src/ai/chain.rs` — `AiProviderChain`、`ChainEntry`、`AttemptRecord`、`build_ai_chain`
- `core-service/src/ai/factory.rs` — `build_provider(&AiProvider) -> Box<dyn AiClient>`
- `core-service/src/handlers/ai_providers.rs` — REST handlers
- `core-service/tests/ai_providers_api_test.rs` — integration tests
- `frontend/src/pages/settings/ai-providers/AiProvidersSection.tsx`
- `frontend/src/pages/settings/ai-providers/AiProviderList.tsx`
- `frontend/src/pages/settings/ai-providers/AiProviderRow.tsx`
- `frontend/src/pages/settings/ai-providers/AiProviderEditDialog.tsx`

**Modify**
- `core-service/src/ai/client.rs` — `AiError` 擴充 `ProviderUnavailable` + `is_retryable()`
- `core-service/src/ai/openai.rs` — 錯誤分類（5xx/網路/timeout/429 → `ProviderUnavailable`；4xx auth/bad → `ApiError`）
- `core-service/src/ai/mod.rs` — pub use 新模組
- `core-service/src/ai/parser_generator.rs` — 移除 `build_ai_client`，改用 `build_ai_chain` + 解構元組回傳
- `core-service/src/ai/filter_generator.rs` — 同上
- `core-service/src/handlers/mod.rs` — 註冊 `ai_providers`，移除 `ai_settings` 中的 `*ai_settings*` handler 但保留 `*ai_prompt_settings*`
- `core-service/src/handlers/ai_settings.rs` — 移除 `get_ai_settings`/`update_ai_settings`/`test_ai_connection` 三個 handler 與其相關 import；保留 prompt 相關 handler（也可以更名為 `ai_prompts.rs`，但本計畫只移除 handler 函數，保留檔名以縮小改動面）
- `core-service/src/main.rs` — 移除舊三個 `/ai-settings` 路由，新增 `/ai-providers` 系列
- `core-service/src/models/db.rs` — 移除 `AiSettings`/`UpdateAiSettings`，新增 `AiProvider`/`NewAiProvider`/`UpdateAiProvider`
- `core-service/src/schema.rs` — `diesel print-schema` 後重生
- `docs/api/openapi.yaml` — 移除 `/ai-settings` 端點，新增 `/ai-providers` 端點
- `frontend/src/schemas/ai.ts` — 移除 `AiSettings`，新增 `AiProvider`/`CreateAiProviderRequest`/`UpdateAiProviderRequest`
- `frontend/src/layers/ApiLayer.ts` — 移除 `getAiSettings/updateAiSettings/testAiConnection`，新增 6 個 provider API
- `frontend/src/pages/settings/SettingsPage.tsx` — AI Card 段落整段換成 `<AiProvidersSection />`
- `frontend/src/locales/zh-TW.json`（與其他語系） — 新增 `settings.aiProviders.*` 條目
- `frontend/package.json` — 新增 `@dnd-kit/core`、`@dnd-kit/sortable`（若尚未安裝）

---

## Task 1: 建立 Diesel migration

**Files:**
- Create: `core-service/migrations/2026-04-30-000000-multi-ai-providers/up.sql`
- Create: `core-service/migrations/2026-04-30-000000-multi-ai-providers/down.sql`

- [ ] **Step 1: 建立 up.sql**

```sql
CREATE TABLE ai_providers (
    id            SERIAL PRIMARY KEY,
    name          TEXT NOT NULL,
    provider_kind TEXT NOT NULL,
    base_url      TEXT NOT NULL DEFAULT '',
    api_key       TEXT NOT NULL DEFAULT '',
    model_name    TEXT NOT NULL DEFAULT '',
    max_tokens    INT  NOT NULL DEFAULT 4096,
    response_format_mode TEXT NOT NULL DEFAULT 'non_strict',
    is_enabled    BOOLEAN NOT NULL DEFAULT TRUE,
    priority      INT NOT NULL DEFAULT 0,
    created_at    TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ai_providers_enabled_priority
    ON ai_providers (is_enabled, priority);

INSERT INTO ai_providers
    (name, provider_kind, base_url, api_key, model_name,
     max_tokens, response_format_mode, is_enabled, priority)
SELECT 'Default', 'openai_compatible', base_url, api_key, model_name,
       max_tokens, response_format_mode, TRUE, 0
FROM ai_settings;

DROP TABLE ai_settings;
```

- [ ] **Step 2: 建立 down.sql**

```sql
CREATE TABLE ai_settings (
    id         SERIAL PRIMARY KEY,
    base_url   TEXT NOT NULL DEFAULT '',
    api_key    TEXT NOT NULL DEFAULT '',
    model_name TEXT NOT NULL DEFAULT 'gpt-4o-mini',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    max_tokens INT NOT NULL DEFAULT 4096,
    response_format_mode TEXT NOT NULL DEFAULT 'non_strict'
);

INSERT INTO ai_settings (base_url, api_key, model_name, max_tokens, response_format_mode)
SELECT base_url, api_key, model_name, max_tokens, response_format_mode
FROM ai_providers
WHERE is_enabled = TRUE
ORDER BY priority ASC, id ASC
LIMIT 1;

-- 若 ai_providers 沒有任何啟用 row，補一筆預設空值
INSERT INTO ai_settings (base_url, api_key, model_name)
SELECT '', '', 'gpt-4o-mini'
WHERE NOT EXISTS (SELECT 1 FROM ai_settings);

DROP TABLE ai_providers;
```

- [ ] **Step 3: 執行 migration 並重生 schema.rs**

Run: `cd /workspace/core-service && diesel migration run`
Expected: 看到 `Running migration 2026-04-30-000000-multi-ai-providers`，schema.rs 自動更新。

- [ ] **Step 4: 驗證資料搬遷**

Run: `psql -h localhost -U postgres -d bangumi -c "SELECT id, name, provider_kind, base_url, model_name, is_enabled, priority FROM ai_providers;"`
Expected: 看到一列 `Default | openai_compatible | <你原本的 base_url> | <你原本的 model_name> | t | 0`。

- [ ] **Step 5: 驗證 down 邏輯**

Run: `cd /workspace/core-service && diesel migration redo`
Expected: down.sql 執行成功（會 drop ai_providers 並重建 ai_settings）；接著 up.sql 再跑一次也成功。確認 `\d ai_providers` 仍存在。

- [ ] **Step 6: 確認 schema.rs 變更**

Run: `git diff core-service/src/schema.rs | head -40`
Expected: 看到 `ai_settings` 表定義被移除，`ai_providers` 新表定義被加入。

- [ ] **Step 7: Commit**

```bash
cd /workspace
git add core-service/migrations/2026-04-30-000000-multi-ai-providers core-service/src/schema.rs
git commit -m "feat(db): replace ai_settings with ai_providers table"
```

---

## Task 2: 模型層 — AiProvider / NewAiProvider / UpdateAiProvider

**Files:**
- Modify: `core-service/src/models/db.rs`

- [ ] **Step 1: 移除舊 AiSettings 結構**

在 `core-service/src/models/db.rs` 找到 `// ============ AiSettings ============` 區塊與其下兩個 struct（`AiSettings`、`UpdateAiSettings`），整段刪除。

- [ ] **Step 2: 新增 AiProvider 結構**

在原本 `AiSettings` 區塊位置加入：

```rust
// ============ AiProvider ============
#[derive(Queryable, Selectable, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[diesel(table_name = crate::schema::ai_providers)]
pub struct AiProvider {
    pub id: i32,
    pub name: String,
    pub provider_kind: String,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub max_tokens: i32,
    pub response_format_mode: String,
    pub is_enabled: bool,
    pub priority: i32,
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::ai_providers)]
pub struct NewAiProvider<'a> {
    pub name: &'a str,
    pub provider_kind: &'a str,
    pub base_url: &'a str,
    pub api_key: &'a str,
    pub model_name: &'a str,
    pub max_tokens: i32,
    pub response_format_mode: &'a str,
    pub is_enabled: bool,
    pub priority: i32,
}

#[derive(AsChangeset, Debug, Default)]
#[diesel(table_name = crate::schema::ai_providers)]
pub struct UpdateAiProvider {
    pub name: Option<String>,
    pub provider_kind: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub max_tokens: Option<i32>,
    pub response_format_mode: Option<String>,
    pub is_enabled: Option<bool>,
    pub priority: Option<i32>,
    pub updated_at: NaiveDateTime,
}
```

- [ ] **Step 3: 同步 models/mod.rs 的 re-export（若有）**

Run: `rg "AiSettings|UpdateAiSettings" /workspace/core-service/src/models/`
若有 `pub use ...AiSettings...` 行，刪除；新增 `pub use db::{AiProvider, NewAiProvider, UpdateAiProvider};`（保留與其他模型一致的格式）。

- [ ] **Step 4: 確認 cargo check 編譯失敗位置**

Run: `cd /workspace && cargo check -p core-service 2>&1 | head -40`
Expected: 出現 `AiSettings`/`UpdateAiSettings` 找不到的錯誤——這些將在後續 task 修復。先記下哪些檔案有錯。

- [ ] **Step 5: Commit**

```bash
cd /workspace
git add core-service/src/models/db.rs core-service/src/models/mod.rs
git commit -m "feat(models): replace AiSettings with AiProvider model" --allow-empty
```

（編譯尚未通過——後續 task 會修復；commit 加 `--allow-empty` 不需要——這裡 commit 是因為 models 變更獨立，後續 task 會把編譯修通。）

---

## Task 3: 擴充 AiError 並加 is_retryable

**Files:**
- Modify: `core-service/src/ai/client.rs`

- [ ] **Step 1: 改寫 client.rs**

完整覆蓋 `core-service/src/ai/client.rs`：

```rust
use async_trait::async_trait;
use thiserror::Error;

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

    /// Provider 正常回應但內容問題（4xx auth / bad request 等）。不 fallback。
    #[error("provider error: {0}")]
    ApiError(String),
}

impl AiError {
    /// 是否應該 fallback 到下一個 provider
    pub fn is_retryable(&self) -> bool {
        matches!(self, AiError::Http(_) | AiError::ProviderUnavailable(_))
    }
}

#[async_trait]
pub trait AiClient: Send + Sync {
    async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, AiError>;

    async fn chat_completion_structured(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        _schema: &serde_json::Value,
    ) -> Result<String, AiError> {
        self.chat_completion(system_prompt, user_prompt).await
    }
}
```

- [ ] **Step 2: 寫單元測試**

新增測試到同檔最後（或 `core-service/src/ai/client.rs` 內 `#[cfg(test)] mod tests {}`）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_unavailable_is_retryable() {
        assert!(AiError::ProviderUnavailable("503".into()).is_retryable());
    }

    #[test]
    fn api_error_is_not_retryable() {
        assert!(!AiError::ApiError("401".into()).is_retryable());
    }

    #[test]
    fn invalid_json_is_not_retryable() {
        assert!(!AiError::InvalidJson("oops".into()).is_retryable());
    }

    #[test]
    fn not_configured_is_not_retryable() {
        assert!(!AiError::NotConfigured.is_retryable());
    }
}
```

- [ ] **Step 3: 跑測試**

Run: `cd /workspace && cargo test -p core-service ai::client::tests`
Expected: 4 tests passed。

- [ ] **Step 4: Commit**

```bash
cd /workspace
git add core-service/src/ai/client.rs
git commit -m "feat(ai): add ProviderUnavailable variant and is_retryable"
```

---

## Task 4: OpenAiClient 改錯誤分類

**Files:**
- Modify: `core-service/src/ai/openai.rs`

- [ ] **Step 1: 修改 do_request 的錯誤路徑**

打開 `core-service/src/ai/openai.rs`，在 `do_request` 函數內，找到 `if !resp.status().is_success() {` 那段，整段替換為：

```rust
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();

            // 偵測 rate limit（OpenAI 風格 JSON）
            if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&text) {
                if err_json.pointer("/error/code").and_then(|v| v.as_str())
                    == Some("rate_limit_exceeded")
                {
                    let msg = err_json
                        .pointer("/error/message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Rate limit exceeded");
                    let retry_secs = extract_retry_after_secs(msg);
                    let prefix = match retry_secs {
                        Some(s) => format!("[rate_limit_exceeded:{}]", s),
                        None => "[rate_limit_exceeded]".to_string(),
                    };
                    return Err(AiError::ProviderUnavailable(format!("{} {}", prefix, msg)));
                }
            }

            // HTTP 狀態碼分類：5xx 與 429 → ProviderUnavailable（可 fallback）
            // 4xx 其餘（400/401/403/404 等）→ ApiError（不 fallback）
            return if status.is_server_error() || status.as_u16() == 429 {
                Err(AiError::ProviderUnavailable(format!("HTTP {}: {}", status, text)))
            } else {
                Err(AiError::ApiError(format!("HTTP {}: {}", status, text)))
            };
        }
```

- [ ] **Step 2: 確認 cargo check 通過此檔**

Run: `cd /workspace && cargo check -p core-service 2>&1 | rg "openai\.rs" | head`
Expected: 無 error 訊息（`reqwest::Error` 因 `#[from]` 自動轉為 `AiError::Http`，仍可 fallback 因為 `is_retryable()` 涵蓋它）。

- [ ] **Step 3: 補單元測試 — extract_retry_after_secs 仍正常**

`core-service/src/ai/openai.rs` 末尾若無 `#[cfg(test)]` 則新增：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_retry_seconds() {
        let msg = "Rate limit. Please try again in 28.5675s. Try later.";
        assert_eq!(extract_retry_after_secs(msg), Some(29));
    }

    #[test]
    fn no_retry_marker_returns_none() {
        assert_eq!(extract_retry_after_secs("nothing here"), None);
    }
}
```

- [ ] **Step 4: 跑測試**

Run: `cd /workspace && cargo test -p core-service ai::openai::tests`
Expected: 2 tests passed。

- [ ] **Step 5: Commit**

```bash
cd /workspace
git add core-service/src/ai/openai.rs
git commit -m "feat(ai): classify 5xx/429 as ProviderUnavailable for fallback"
```

---

## Task 5: Provider 工廠

**Files:**
- Create: `core-service/src/ai/factory.rs`
- Modify: `core-service/src/ai/mod.rs`

- [ ] **Step 1: 建立 factory.rs**

```rust
use crate::models::AiProvider;
use super::client::AiClient;
use super::openai::OpenAiClient;

pub fn build_provider(p: &AiProvider) -> Result<Box<dyn AiClient>, String> {
    match p.provider_kind.as_str() {
        "openai_compatible" => Ok(Box::new(OpenAiClient::new(
            &p.base_url,
            &p.api_key,
            &p.model_name,
            p.max_tokens,
            &p.response_format_mode,
        ))),
        other => Err(format!("unknown provider_kind: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn provider(kind: &str) -> AiProvider {
        AiProvider {
            id: 1,
            name: "x".into(),
            provider_kind: kind.into(),
            base_url: "https://example.com".into(),
            api_key: "k".into(),
            model_name: "m".into(),
            max_tokens: 4096,
            response_format_mode: "non_strict".into(),
            is_enabled: true,
            priority: 0,
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
        }
    }

    #[test]
    fn openai_compatible_builds() {
        assert!(build_provider(&provider("openai_compatible")).is_ok());
    }

    #[test]
    fn unknown_kind_errors() {
        let err = build_provider(&provider("anthropic")).unwrap_err();
        assert!(err.contains("anthropic"));
    }
}
```

- [ ] **Step 2: 在 mod.rs 註冊**

打開 `core-service/src/ai/mod.rs`，在 `pub mod openai;` 後加 `pub mod factory;`。

- [ ] **Step 3: 跑測試**

Run: `cd /workspace && cargo test -p core-service ai::factory`
Expected: 2 tests passed。

- [ ] **Step 4: Commit**

```bash
cd /workspace
git add core-service/src/ai/factory.rs core-service/src/ai/mod.rs
git commit -m "feat(ai): add provider factory dispatching by provider_kind"
```

---

## Task 6: AiProviderChain（含 fallback 單元測試）

**Files:**
- Create: `core-service/src/ai/chain.rs`
- Modify: `core-service/src/ai/mod.rs`

- [ ] **Step 1: 建立 chain.rs（核心邏輯 + builder）**

```rust
use crate::models::AiProvider;
use crate::schema::ai_providers;
use diesel::prelude::*;
use serde_json::Value;

use super::client::{AiClient, AiError};
use super::factory::build_provider;

pub struct ChainEntry {
    pub id: i32,
    pub name: String,
    pub client: Box<dyn AiClient>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AttemptRecord {
    pub provider_id: i32,
    pub provider_name: String,
    pub error: String,
}

pub struct AiProviderChain {
    entries: Vec<ChainEntry>,
}

impl AiProviderChain {
    pub fn new(entries: Vec<ChainEntry>) -> Self { Self { entries } }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn len(&self) -> usize { self.entries.len() }

    async fn run<'a, F, Fut>(
        &'a self,
        op: F,
    ) -> Result<(String, Vec<AttemptRecord>), (AiError, Vec<AttemptRecord>)>
    where
        F: Fn(&'a dyn AiClient) -> Fut,
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
        Err((
            AiError::ProviderUnavailable(format!("all providers failed: {last}")),
            attempts,
        ))
    }

    pub async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<(String, Vec<AttemptRecord>), (AiError, Vec<AttemptRecord>)> {
        self.run(|c| c.chat_completion(system_prompt, user_prompt)).await
    }

    pub async fn chat_completion_structured(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &Value,
    ) -> Result<(String, Vec<AttemptRecord>), (AiError, Vec<AttemptRecord>)> {
        self.run(|c| c.chat_completion_structured(system_prompt, user_prompt, schema)).await
    }
}

pub fn build_ai_chain(conn: &mut PgConnection) -> Result<Option<AiProviderChain>, String> {
    let providers = ai_providers::table
        .filter(ai_providers::is_enabled.eq(true))
        .order(ai_providers::priority.asc())
        .then_order_by(ai_providers::id.asc())
        .load::<AiProvider>(conn)
        .map_err(|e| e.to_string())?;

    let entries: Vec<ChainEntry> = providers
        .into_iter()
        .filter(|p| !p.api_key.is_empty() && !p.base_url.is_empty())
        .map(|p| {
            build_provider(&p).map(|client| ChainEntry {
                id: p.id,
                name: p.name.clone(),
                client,
            })
        })
        .collect::<Result<_, _>>()?;

    Ok((!entries.is_empty()).then_some(AiProviderChain::new(entries)))
}
```

- [ ] **Step 2: 註冊到 mod.rs**

打開 `core-service/src/ai/mod.rs`，於 `pub mod factory;` 後加 `pub mod chain;`，並在檔末加：

```rust
pub use chain::{AiProviderChain, AttemptRecord, build_ai_chain};
```

- [ ] **Step 3: 撰寫 fallback 單元測試**

在 `core-service/src/ai/chain.rs` 末尾加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// MockAiClient：依序回傳預先設定好的結果
    struct MockAiClient {
        results: Mutex<std::vec::IntoIter<Result<String, AiError>>>,
    }

    impl MockAiClient {
        fn new(results: Vec<Result<String, AiError>>) -> Self {
            Self { results: Mutex::new(results.into_iter()) }
        }
    }

    #[async_trait]
    impl AiClient for MockAiClient {
        async fn chat_completion(&self, _: &str, _: &str) -> Result<String, AiError> {
            self.results
                .lock()
                .unwrap()
                .next()
                .unwrap_or(Err(AiError::ApiError("exhausted".into())))
        }
    }

    fn entry(id: i32, name: &str, results: Vec<Result<String, AiError>>) -> ChainEntry {
        ChainEntry {
            id,
            name: name.into(),
            client: Box::new(MockAiClient::new(results)),
        }
    }

    #[tokio::test]
    async fn empty_chain_returns_not_configured() {
        let chain = AiProviderChain::new(vec![]);
        let err = chain.chat_completion("s", "u").await.unwrap_err();
        assert!(matches!(err.0, AiError::NotConfigured));
        assert!(err.1.is_empty());
    }

    #[tokio::test]
    async fn first_provider_succeeds() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Ok("hello".into())]),
            entry(2, "b", vec![Ok("ignored".into())]),
        ]);
        let (resp, attempts) = chain.chat_completion("s", "u").await.unwrap();
        assert_eq!(resp, "hello");
        assert!(attempts.is_empty());
    }

    #[tokio::test]
    async fn falls_back_on_retryable() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Err(AiError::ProviderUnavailable("503".into()))]),
            entry(2, "b", vec![Ok("ok".into())]),
        ]);
        let (resp, attempts) = chain.chat_completion("s", "u").await.unwrap();
        assert_eq!(resp, "ok");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].provider_name, "a");
    }

    #[tokio::test]
    async fn all_retryable_fail() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Err(AiError::ProviderUnavailable("503".into()))]),
            entry(2, "b", vec![Ok("never".into())]),
        ]);
        // 第二個 provider 也設失敗
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Err(AiError::ProviderUnavailable("503".into()))]),
            entry(2, "b", vec![Err(AiError::ProviderUnavailable("502".into()))]),
        ]);
        let _ = chain;
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Err(AiError::ProviderUnavailable("503".into()))]),
            entry(2, "b", vec![Err(AiError::ProviderUnavailable("502".into()))]),
        ]);
        let err = chain.chat_completion("s", "u").await.unwrap_err();
        assert!(matches!(err.0, AiError::ProviderUnavailable(_)));
        assert_eq!(err.1.len(), 2);
    }

    #[tokio::test]
    async fn non_retryable_stops_immediately() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Err(AiError::ApiError("401".into()))]),
            entry(2, "b", vec![Ok("never".into())]),
        ]);
        let err = chain.chat_completion("s", "u").await.unwrap_err();
        assert!(matches!(err.0, AiError::ApiError(_)));
        assert_eq!(err.1.len(), 1);
        assert_eq!(err.1[0].provider_name, "a");
    }
}
```

- [ ] **Step 4: 跑測試**

Run: `cd /workspace && cargo test -p core-service ai::chain`
Expected: 5 tests passed。

- [ ] **Step 5: Commit**

```bash
cd /workspace
git add core-service/src/ai/chain.rs core-service/src/ai/mod.rs
git commit -m "feat(ai): add AiProviderChain with fallback on retryable errors"
```

---

## Task 7: 移除 build_ai_client，呼叫端改用 chain（parser_generator）

**Files:**
- Modify: `core-service/src/ai/parser_generator.rs`

- [ ] **Step 1: 移除 build_ai_client 函數**

打開 `core-service/src/ai/parser_generator.rs`，刪除整個 `pub fn build_ai_client(...)` 函數定義（含 import `OpenAiClient`、`crate::schema::ai_settings` 若僅此一處使用，一併清理）。

- [ ] **Step 2: 改 import**

確認檔案頂端 imports 含：
```rust
use crate::ai::{build_ai_chain, AiProviderChain, AttemptRecord};
use crate::ai::client::AiError;
```

- [ ] **Step 3: 將原本 `build_ai_client(&mut conn)` 呼叫換為 `build_ai_chain(&mut conn)`，並改用 chain 的方法**

找到 `let client_result = { ... build_ai_client(&mut conn) };` 這段，整段替換為 `let chain_result = { let mut conn = pool.get()?; build_ai_chain(&mut conn) };`。

接著找到 `let ai_result = match client_result { Ok(Some(client)) => { ... client.chat_completion_structured(...).await } Ok(None) => Err(AiError::NotConfigured), Err(e) => Err(AiError::ApiError(e)), };` 這段，替換為：

```rust
let ai_result: Result<(String, Vec<AttemptRecord>), AiError> = match chain_result {
    Ok(Some(chain)) => {
        let system = build_system_prompt(Some(&fixed_prompt));
        let user = build_parser_user_prompt(&source_title, custom_prompt.as_deref());
        match chain.chat_completion_structured(&system, &user, &parser_schema()).await {
            Ok((resp, attempts)) => {
                if !attempts.is_empty() {
                    tracing::info!(
                        pending_id,
                        attempts = ?attempts,
                        "AI parser fell back through providers before success"
                    );
                }
                Ok((resp, attempts))
            }
            Err((e, attempts)) => {
                if !attempts.is_empty() {
                    tracing::warn!(pending_id, attempts = ?attempts, "AI parser fallback chain exhausted");
                }
                Err(e)
            }
        }
    }
    Ok(None) => Err(AiError::NotConfigured),
    Err(e) => Err(AiError::ApiError(e)),
};
```

接著把後續 `match ai_result { Ok(json_str) => { let extracted = super::extract_json(&json_str); ... }` 中的 `Ok(json_str)` 改為 `Ok((json_str, _attempts))`，其餘不動。

- [ ] **Step 4: 編譯**

Run: `cd /workspace && cargo check -p core-service 2>&1 | rg "parser_generator|error\[" | head -30`
Expected: 此檔無錯（其餘檔錯誤稍後處理）。

- [ ] **Step 5: Commit**

```bash
cd /workspace
git add core-service/src/ai/parser_generator.rs
git commit -m "feat(ai): parser_generator uses fallback chain"
```

---

## Task 8: 同步改 filter_generator

**Files:**
- Modify: `core-service/src/ai/filter_generator.rs`

- [ ] **Step 1: 重複 Task 7 的相同改造方式**

在 `core-service/src/ai/filter_generator.rs`：
1. 移除 `use ... OpenAiClient` / `ai_settings::table` 等不再用到的 import。
2. 加 `use crate::ai::{build_ai_chain, AttemptRecord};`、`use crate::ai::client::AiError;`。
3. 把 `build_ai_client(&mut conn)` 改成 `build_ai_chain(&mut conn)`。
4. 將原本對 `client.chat_completion_structured(...)` 的呼叫，改為 `chain.chat_completion_structured(...)` 並承接元組 `(json_str, attempts)`，attempts 寫入 `tracing::info!`。
5. 後續對 `ai_result` 的 `Ok(json_str)` 模式改為 `Ok((json_str, _attempts))`。

完整改法可參考 Task 7 的程式碼結構，把 `pending_id` / `parser_schema()` / `build_parser_user_prompt` 換成 filter 對應的變數與函數（在原檔可看到）。

- [ ] **Step 2: 編譯**

Run: `cd /workspace && cargo check -p core-service 2>&1 | rg "filter_generator|error\[" | head -20`
Expected: 此檔無錯。

- [ ] **Step 3: Commit**

```bash
cd /workspace
git add core-service/src/ai/filter_generator.rs
git commit -m "feat(ai): filter_generator uses fallback chain"
```

---

## Task 9: 移除舊 ai_settings 三個 handler

**Files:**
- Modify: `core-service/src/handlers/ai_settings.rs`

- [ ] **Step 1: 刪除三個 handler 與其 request struct**

在 `core-service/src/handlers/ai_settings.rs` 中：
1. 刪除 `pub async fn get_ai_settings(...)`
2. 刪除 `pub struct UpdateAiSettingsRequest { ... }` 與 `pub async fn update_ai_settings(...)`
3. 刪除 `pub async fn test_ai_connection(...)`
4. 移除頂端不再使用的 import：`AiSettings`、`UpdateAiSettings`、`ai_settings`、`crate::ai::parser_generator::build_ai_client`。
5. 保留：`AiPromptSettings`、`ai_prompt_settings`、`get_ai_prompt_settings`、`update_ai_prompt_settings`、`revert_parser_prompt`、`revert_filter_prompt`、`DEFAULT_FIXED_*_PROMPT`。

- [ ] **Step 2: 編譯**

Run: `cd /workspace && cargo check -p core-service 2>&1 | rg "ai_settings\.rs|error\[" | head -20`
Expected: 此檔無錯（main.rs 路由仍指向被刪除的 handler，下個 task 處理）。

- [ ] **Step 3: Commit**

```bash
cd /workspace
git add core-service/src/handlers/ai_settings.rs
git commit -m "refactor(handlers): drop legacy ai_settings handlers"
```

---

## Task 10: 新增 ai_providers handler

**Files:**
- Create: `core-service/src/handlers/ai_providers.rs`
- Modify: `core-service/src/handlers/mod.rs`

- [ ] **Step 1: 建立 handler 檔案**

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::{AiProvider, NewAiProvider, UpdateAiProvider};
use crate::schema::ai_providers;
use crate::state::AppState;

const ALLOWED_KINDS: &[&str] = &["openai_compatible"];
const ALLOWED_FORMAT_MODES: &[&str] = &["strict", "non_strict", "inject_schema"];
const MASKED_API_KEY: &str = "••••••••";

fn mask(p: AiProvider) -> AiProvider {
    AiProvider { api_key: MASKED_API_KEY.into(), ..p }
}

fn validate_kind(kind: &str) -> Result<(), (StatusCode, String)> {
    if !ALLOWED_KINDS.contains(&kind) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("invalid provider_kind: {kind}"),
        ));
    }
    Ok(())
}

fn validate_mode(mode: &str) -> Result<(), (StatusCode, String)> {
    if !ALLOWED_FORMAT_MODES.contains(&mode) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("invalid response_format_mode: {mode}"),
        ));
    }
    Ok(())
}

pub async fn list_ai_providers(
    State(state): State<AppState>,
) -> Result<Json<Vec<AiProvider>>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let providers = ai_providers::table
        .order(ai_providers::priority.asc())
        .then_order_by(ai_providers::id.asc())
        .load::<AiProvider>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(providers.into_iter().map(mask).collect()))
}

pub async fn get_ai_provider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<AiProvider>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let p = ai_providers::table
        .find(id)
        .first::<AiProvider>(&mut conn)
        .optional()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, format!("provider {id} not found")))?;
    Ok(Json(mask(p)))
}

#[derive(Debug, Deserialize)]
pub struct CreateAiProviderRequest {
    pub name: String,
    pub provider_kind: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model_name: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i32,
    #[serde(default = "default_format_mode")]
    pub response_format_mode: String,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

fn default_max_tokens() -> i32 { 4096 }
fn default_format_mode() -> String { "non_strict".into() }
fn default_true() -> bool { true }

pub async fn create_ai_provider(
    State(state): State<AppState>,
    Json(req): Json<CreateAiProviderRequest>,
) -> Result<Json<AiProvider>, (StatusCode, String)> {
    validate_kind(&req.provider_kind)?;
    validate_mode(&req.response_format_mode)?;

    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let next_priority: Option<i32> = ai_providers::table
        .select(diesel::dsl::max(ai_providers::priority))
        .first::<Option<i32>>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let priority = next_priority.map(|v| v + 1).unwrap_or(0);

    let new_p = NewAiProvider {
        name: &req.name,
        provider_kind: &req.provider_kind,
        base_url: &req.base_url,
        api_key: &req.api_key,
        model_name: &req.model_name,
        max_tokens: req.max_tokens,
        response_format_mode: &req.response_format_mode,
        is_enabled: req.is_enabled,
        priority,
    };
    let inserted: AiProvider = diesel::insert_into(ai_providers::table)
        .values(&new_p)
        .get_result(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(mask(inserted)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateAiProviderRequest {
    pub name: Option<String>,
    pub provider_kind: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub max_tokens: Option<i32>,
    pub response_format_mode: Option<String>,
    pub is_enabled: Option<bool>,
}

pub async fn update_ai_provider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<UpdateAiProviderRequest>,
) -> Result<Json<AiProvider>, (StatusCode, String)> {
    if let Some(ref kind) = req.provider_kind {
        validate_kind(kind)?;
    }
    if let Some(ref mode) = req.response_format_mode {
        validate_mode(mode)?;
    }

    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // api_key 為空字串視為「不更新」（避免遮罩值覆蓋真實值）
    let api_key = req.api_key.filter(|k| !k.is_empty());

    let changes = UpdateAiProvider {
        name: req.name,
        provider_kind: req.provider_kind,
        base_url: req.base_url,
        api_key,
        model_name: req.model_name,
        max_tokens: req.max_tokens,
        response_format_mode: req.response_format_mode,
        is_enabled: req.is_enabled,
        priority: None,
        updated_at: Utc::now().naive_utc(),
    };

    let updated: AiProvider = diesel::update(ai_providers::table.find(id))
        .set(&changes)
        .get_result(&mut conn)
        .optional()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, format!("provider {id} not found")))?;
    Ok(Json(mask(updated)))
}

pub async fn delete_ai_provider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let n = diesel::delete(ai_providers::table.find(id))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, format!("provider {id} not found")));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct ReorderRequest {
    pub ordered_ids: Vec<i32>,
}

pub async fn reorder_ai_providers(
    State(state): State<AppState>,
    Json(req): Json<ReorderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = Utc::now().naive_utc();

    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        for (idx, id) in req.ordered_ids.iter().enumerate() {
            diesel::update(ai_providers::table.find(id))
                .set((
                    ai_providers::priority.eq(idx as i32),
                    ai_providers::updated_at.eq(now),
                ))
                .execute(conn)?;
        }
        Ok(())
    })
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Serialize)]
pub struct TestResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn test_ai_provider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<TestResponse>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let provider: AiProvider = ai_providers::table
        .find(id)
        .first(&mut conn)
        .optional()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, format!("provider {id} not found")))?;

    let client = crate::ai::factory::build_provider(&provider)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    use crate::ai::client::AiClient;
    let result = client
        .chat_completion("", "Reply with json: {\"ok\": true}")
        .await;
    match result {
        Ok(_) => Ok(Json(TestResponse { ok: true, error: None })),
        Err(e) => Ok(Json(TestResponse { ok: false, error: Some(e.to_string()) })),
    }
}
```

- [ ] **Step 2: 註冊 handler 模組**

打開 `core-service/src/handlers/mod.rs`，加 `pub mod ai_providers;` 一行。

- [ ] **Step 3: 編譯**

Run: `cd /workspace && cargo check -p core-service 2>&1 | rg "ai_providers|error\[" | head -30`
Expected: 此檔無錯（main.rs 路由尚未註冊，下個 task 處理）。

- [ ] **Step 4: Commit**

```bash
cd /workspace
git add core-service/src/handlers/ai_providers.rs core-service/src/handlers/mod.rs
git commit -m "feat(handlers): CRUD/reorder/test endpoints for ai_providers"
```

---

## Task 11: 註冊路由（main.rs）

**Files:**
- Modify: `core-service/src/main.rs`

- [ ] **Step 1: 移除舊 /ai-settings 三條路由**

在 `core-service/src/main.rs` 中找到：
```rust
.route("/ai-settings", get(handlers::ai_settings::get_ai_settings).put(handlers::ai_settings::update_ai_settings))
.route("/ai-settings/test", post(handlers::ai_settings::test_ai_connection))
```
（語法可能略有不同）整段刪除。`/ai-prompt-settings` 系列保留。

- [ ] **Step 2: 新增 /ai-providers 路由**

在原舊路由位置加入：

```rust
.route(
    "/ai-providers",
    get(handlers::ai_providers::list_ai_providers)
        .post(handlers::ai_providers::create_ai_provider),
)
.route(
    "/ai-providers/:id",
    get(handlers::ai_providers::get_ai_provider)
        .put(handlers::ai_providers::update_ai_provider)
        .delete(handlers::ai_providers::delete_ai_provider),
)
.route("/ai-providers/reorder", post(handlers::ai_providers::reorder_ai_providers))
.route("/ai-providers/:id/test", post(handlers::ai_providers::test_ai_provider))
```

注意：Axum 0.7 路由排序需 `:id` 固定段優先匹配，`reorder` 必須放在 `:id` 之後且無歧義。實際上 Axum 0.7+ 的 matchit router 對 `/:id` 與 `/reorder` 區分明確，但建議 `/ai-providers/reorder` 放在 `/ai-providers/:id` **之前**避免與 `id="reorder"` 路徑解析歧義。請以「reorder 在前，:id 在後」順序註冊。

- [ ] **Step 3: 編譯整個 service**

Run: `cd /workspace && cargo check -p core-service 2>&1 | tail -20`
Expected: `Finished`，無 error。

- [ ] **Step 4: 跑現有所有測試確認沒打破**

Run: `cd /workspace && cargo test -p core-service --lib`
Expected: All tests pass（含 task 3/4/5/6 的新測試）。

- [ ] **Step 5: Commit**

```bash
cd /workspace
git add core-service/src/main.rs
git commit -m "feat(routes): register /ai-providers and remove /ai-settings"
```

---

## Task 12: Integration tests（CRUD + reorder + masking + validation）

**Files:**
- Create: `core-service/tests/ai_providers_api_test.rs`

- [ ] **Step 1: 觀察現有 integration test 啟動方式**

Run: `head -60 /workspace/core-service/tests/integration_test_subscriptions.rs`
記下：怎麼建 app/conn pool/migration、用什麼 helper 函數。新測試遵循同樣樣板。

- [ ] **Step 2: 建立測試檔**

骨架（依現有專案的測試 helper 風格填入；以下示範用 `axum::Router` + `tower::ServiceExt::oneshot` 直接呼叫）：

```rust
// 沿用 integration_test_subscriptions.rs 的 setup 函數樣板
mod common;
use common::{spawn_app, TestApp};
use axum::http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn full_crud_lifecycle() {
    let app = spawn_app().await;

    // CREATE
    let created: serde_json::Value = app
        .post_json("/ai-providers", json!({
            "name": "OpenAI",
            "provider_kind": "openai_compatible",
            "base_url": "https://api.openai.com/v1",
            "api_key": "sk-test",
            "model_name": "gpt-4o-mini"
        }))
        .await
        .json_value(StatusCode::OK)
        .await;
    let id = created["id"].as_i64().unwrap();
    assert_eq!(created["api_key"], "••••••••");

    // LIST
    let list = app.get("/ai-providers").await.json_array(StatusCode::OK).await;
    assert!(list.iter().any(|p| p["id"] == id));

    // GET
    let one = app.get(&format!("/ai-providers/{id}")).await.json_value(StatusCode::OK).await;
    assert_eq!(one["api_key"], "••••••••");
    assert_eq!(one["name"], "OpenAI");

    // PUT — 不傳 api_key（保留舊值）
    app.put_json(&format!("/ai-providers/{id}"), json!({ "name": "OpenAI 改" }))
        .await
        .expect_status(StatusCode::OK);
    let renamed = app.get(&format!("/ai-providers/{id}")).await.json_value(StatusCode::OK).await;
    assert_eq!(renamed["name"], "OpenAI 改");

    // DELETE
    app.delete(&format!("/ai-providers/{id}"))
        .await
        .expect_status(StatusCode::OK);

    let list_after = app.get("/ai-providers").await.json_array(StatusCode::OK).await;
    assert!(!list_after.iter().any(|p| p["id"] == id));
}

#[tokio::test]
async fn reorder_changes_priority() {
    let app = spawn_app().await;
    let mut ids = vec![];
    for n in &["a", "b", "c"] {
        let p = app.post_json("/ai-providers", json!({
            "name": n,
            "provider_kind": "openai_compatible",
            "base_url": "https://x", "api_key": "k", "model_name": "m"
        })).await.json_value(StatusCode::OK).await;
        ids.push(p["id"].as_i64().unwrap() as i32);
    }
    // reorder: c, a, b
    app.post_json("/ai-providers/reorder", json!({
        "ordered_ids": [ids[2], ids[0], ids[1]]
    })).await.expect_status(StatusCode::OK);

    let list = app.get("/ai-providers").await.json_array(StatusCode::OK).await;
    let names: Vec<&str> = list.iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["c", "a", "b"]);
}

#[tokio::test]
async fn rejects_unknown_provider_kind() {
    let app = spawn_app().await;
    let resp = app.post_json("/ai-providers", json!({
        "name": "x", "provider_kind": "anthropic",
        "base_url": "x", "api_key": "k", "model_name": "m"
    })).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_invalid_response_format_mode() {
    let app = spawn_app().await;
    let resp = app.post_json("/ai-providers", json!({
        "name": "x", "provider_kind": "openai_compatible",
        "base_url": "x", "api_key": "k", "model_name": "m",
        "response_format_mode": "wrong"
    })).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn put_empty_api_key_preserves_old() {
    let app = spawn_app().await;
    let p = app.post_json("/ai-providers", json!({
        "name": "x", "provider_kind": "openai_compatible",
        "base_url": "x", "api_key": "secret-original", "model_name": "m"
    })).await.json_value(StatusCode::OK).await;
    let id = p["id"].as_i64().unwrap();

    app.put_json(&format!("/ai-providers/{id}"), json!({ "api_key": "" }))
        .await
        .expect_status(StatusCode::OK);

    // 透過 raw DB query 驗證
    let raw = app.db_query_one_string(
        "SELECT api_key FROM ai_providers WHERE id = $1", id as i32).await;
    assert_eq!(raw, "secret-original");
}

#[tokio::test]
async fn test_endpoint_404_when_missing() {
    let app = spawn_app().await;
    let resp = app.post_json("/ai-providers/99999/test", json!({})).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
```

> 註：本檔以 `spawn_app()` 樣板示意。實作時依 `integration_test_subscriptions.rs` 已存在的 helper 函數命名與簽名替換（例如 `make_app()`、`setup_app()`、或 `axum_test::TestServer`）。如果現有測試沒有 `db_query_one_string` 之類 helper，可改用：先 `GET /ai-providers/{id}` 再讀回 `api_key`——但 GET 是遮罩，無法驗證原值；此時改寫成「PUT 空 api_key、再用新 api_key 嘗試 test 端點 200」當間接驗證。請依該 helper 條件擇一。

- [ ] **Step 3: 跑 integration tests**

Run: `cd /workspace && cargo test -p core-service --test ai_providers_api_test`
Expected: 6 tests passed。

- [ ] **Step 4: Commit**

```bash
cd /workspace
git add core-service/tests/ai_providers_api_test.rs
git commit -m "test(ai-providers): integration tests for CRUD/reorder/validation"
```

---

## Task 13: 更新 OpenAPI spec

**Files:**
- Modify: `docs/api/openapi.yaml`

- [ ] **Step 1: 移除 /ai-settings 端點定義**

在 `docs/api/openapi.yaml` 找到 `/ai-settings:` 與 `/ai-settings/test:` 兩個 path 條目，整段刪除。`/ai-prompt-settings*` 保留。

- [ ] **Step 2: 新增 /ai-providers 端點**

加入：

```yaml
  /ai-providers:
    get:
      summary: List AI providers
      tags: [ai]
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: "#/components/schemas/AiProvider"
    post:
      summary: Create AI provider
      tags: [ai]
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/CreateAiProviderRequest"
      responses:
        "200": { description: Created, content: { application/json: { schema: { $ref: "#/components/schemas/AiProvider" } } } }
        "400": { description: Validation error }

  /ai-providers/{id}:
    parameters:
      - name: id
        in: path
        required: true
        schema: { type: integer }
    get:
      summary: Get AI provider
      tags: [ai]
      responses:
        "200": { description: OK, content: { application/json: { schema: { $ref: "#/components/schemas/AiProvider" } } } }
        "404": { description: Not found }
    put:
      summary: Update AI provider
      tags: [ai]
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/UpdateAiProviderRequest"
      responses:
        "200": { description: OK, content: { application/json: { schema: { $ref: "#/components/schemas/AiProvider" } } } }
        "400": { description: Validation error }
        "404": { description: Not found }
    delete:
      summary: Delete AI provider
      tags: [ai]
      responses:
        "200": { description: OK }
        "404": { description: Not found }

  /ai-providers/reorder:
    post:
      summary: Reorder AI providers
      tags: [ai]
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [ordered_ids]
              properties:
                ordered_ids:
                  type: array
                  items: { type: integer }
      responses:
        "200": { description: OK }

  /ai-providers/{id}/test:
    parameters:
      - name: id
        in: path
        required: true
        schema: { type: integer }
    post:
      summary: Test AI provider connection
      tags: [ai]
      responses:
        "200":
          description: Test result
          content:
            application/json:
              schema:
                type: object
                properties:
                  ok: { type: boolean }
                  error: { type: string, nullable: true }
        "404": { description: Not found }
```

- [ ] **Step 3: 在 components.schemas 新增三個 schema**

```yaml
    AiProvider:
      type: object
      required: [id, name, provider_kind, base_url, api_key, model_name, max_tokens, response_format_mode, is_enabled, priority, created_at, updated_at]
      properties:
        id: { type: integer }
        name: { type: string }
        provider_kind: { type: string, enum: [openai_compatible] }
        base_url: { type: string }
        api_key: { type: string, description: "已遮罩的 API key" }
        model_name: { type: string }
        max_tokens: { type: integer }
        response_format_mode: { type: string, enum: [strict, non_strict, inject_schema] }
        is_enabled: { type: boolean }
        priority: { type: integer }
        created_at: { type: string, format: date-time }
        updated_at: { type: string, format: date-time }

    CreateAiProviderRequest:
      type: object
      required: [name, provider_kind]
      properties:
        name: { type: string }
        provider_kind: { type: string, enum: [openai_compatible] }
        base_url: { type: string }
        api_key: { type: string }
        model_name: { type: string }
        max_tokens: { type: integer, default: 4096 }
        response_format_mode: { type: string, enum: [strict, non_strict, inject_schema], default: non_strict }
        is_enabled: { type: boolean, default: true }

    UpdateAiProviderRequest:
      type: object
      properties:
        name: { type: string }
        provider_kind: { type: string, enum: [openai_compatible] }
        base_url: { type: string }
        api_key: { type: string, description: "空字串 = 不更新" }
        model_name: { type: string }
        max_tokens: { type: integer }
        response_format_mode: { type: string, enum: [strict, non_strict, inject_schema] }
        is_enabled: { type: boolean }
```

也記得移除 `AiSettings`、`UpdateAiSettingsRequest` schema（若曾存在）。

- [ ] **Step 4: lint 驗證 OpenAPI（若專案有對應命令；無則跳過）**

Run: `rg "openapi" /workspace/Makefile /workspace/package.json 2>/dev/null | head`
若有 `make openapi-lint` 之類就跑；沒有則跳過。

- [ ] **Step 5: Commit**

```bash
cd /workspace
git add docs/api/openapi.yaml
git commit -m "docs(openapi): replace /ai-settings with /ai-providers endpoints"
```

---

## Task 14: 前端 schema 與 API Layer

**Files:**
- Modify: `frontend/src/schemas/ai.ts`
- Modify: `frontend/src/layers/ApiLayer.ts`

- [ ] **Step 1: 更新 schemas/ai.ts**

替換 `AiSettings` 接口為 `AiProvider` 與兩個 request 形狀（保留 `ResponseFormatMode`、`AiPromptSettings`、`PendingAiResult` 不動）：

```typescript
export type ResponseFormatMode = "strict" | "non_strict" | "inject_schema"
export type AiProviderKind = "openai_compatible"

export interface AiProvider {
  id: number
  name: string
  provider_kind: AiProviderKind
  base_url: string
  api_key: string // already masked when fetched
  model_name: string
  max_tokens: number
  response_format_mode: ResponseFormatMode
  is_enabled: boolean
  priority: number
  created_at: string
  updated_at: string
}

export interface CreateAiProviderRequest {
  name: string
  provider_kind: AiProviderKind
  base_url?: string
  api_key?: string
  model_name?: string
  max_tokens?: number
  response_format_mode?: ResponseFormatMode
  is_enabled?: boolean
}

export interface UpdateAiProviderRequest {
  name?: string
  provider_kind?: AiProviderKind
  base_url?: string
  api_key?: string // 空字串 = 不更新
  model_name?: string
  max_tokens?: number
  response_format_mode?: ResponseFormatMode
  is_enabled?: boolean
}

export interface TestAiProviderResult {
  ok: boolean
  error?: string
}
```

刪除原本 `AiSettings` interface。

- [ ] **Step 2: 更新 ApiLayer.ts**

在 `frontend/src/layers/ApiLayer.ts`：
1. 移除 `getAiSettings`、`updateAiSettings`、`testAiConnection` 方法（含其 import 的 `AiSettings` type）。
2. 改成 import：`import type { AiProvider, CreateAiProviderRequest, UpdateAiProviderRequest, TestAiProviderResult, AiPromptSettings, PendingAiResult, ConfirmPendingRequest, RegenerateRequest } from "@/schemas/ai"`。
3. 在 service 物件中加入：

```typescript
listAiProviders: client.execute(HttpClientRequest.get("/api/core/ai-providers")).pipe(
  Effect.map((r) => r as readonly AiProvider[]),
),

createAiProvider: (req: CreateAiProviderRequest) =>
  client.execute(
    HttpClientRequest.post("/api/core/ai-providers").pipe(
      HttpClientRequest.bodyUnsafeJson(req),
    ),
  ).pipe(Effect.map((r) => r as AiProvider)),

updateAiProvider: (id: number, req: UpdateAiProviderRequest) =>
  client.execute(
    HttpClientRequest.put(`/api/core/ai-providers/${id}`).pipe(
      HttpClientRequest.bodyUnsafeJson(req),
    ),
  ).pipe(Effect.map((r) => r as AiProvider)),

deleteAiProvider: (id: number) =>
  client.execute(HttpClientRequest.del(`/api/core/ai-providers/${id}`)),

reorderAiProviders: (ordered_ids: readonly number[]) =>
  client.execute(
    HttpClientRequest.post("/api/core/ai-providers/reorder").pipe(
      HttpClientRequest.bodyUnsafeJson({ ordered_ids }),
    ),
  ),

testAiProvider: (id: number) =>
  client.execute(
    HttpClientRequest.post(`/api/core/ai-providers/${id}/test`),
  ).pipe(Effect.map((r) => r as TestAiProviderResult)),
```

（具體寫法依該檔現有風格調整 —— 現有 `getAiSettings` 等寫法可作為樣板。）

- [ ] **Step 3: TypeScript 檢查**

Run: `cd /workspace/frontend && bun tsc --noEmit 2>&1 | head -30`
Expected: 無 error 影響本變動範圍（其他檔仍使用 `getAiSettings` 的會列出，由下個 task 修復）。

- [ ] **Step 4: Commit**

```bash
cd /workspace
git add frontend/src/schemas/ai.ts frontend/src/layers/ApiLayer.ts
git commit -m "feat(frontend): AiProvider schema and API layer methods"
```

---

## Task 15: 安裝 dnd-kit 依賴並建立 AiProvidersSection / Row / List

**Files:**
- Modify: `frontend/package.json`
- Create: `frontend/src/pages/settings/ai-providers/AiProvidersSection.tsx`
- Create: `frontend/src/pages/settings/ai-providers/AiProviderList.tsx`
- Create: `frontend/src/pages/settings/ai-providers/AiProviderRow.tsx`

- [ ] **Step 1: 確認/安裝 dnd-kit**

Run: `rg "@dnd-kit" /workspace/frontend/package.json`
若無：

```bash
cd /workspace/frontend && bun add @dnd-kit/core @dnd-kit/sortable @dnd-kit/utilities
```

- [ ] **Step 2: 建立 AiProviderRow.tsx**

```typescript
import { useSortable } from "@dnd-kit/sortable"
import { CSS } from "@dnd-kit/utilities"
import { GripVertical, Loader2, Trash2, Pencil } from "lucide-react"
import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import { Badge } from "@/components/ui/badge"
import type { AiProvider, TestAiProviderResult } from "@/schemas/ai"

export interface AiProviderRowProps {
  provider: AiProvider
  index: number
  onEdit: (p: AiProvider) => void
  onDelete: (id: number) => void
  onToggle: (id: number, is_enabled: boolean) => void
  onTest: (id: number) => Promise<TestAiProviderResult>
}

export function AiProviderRow({ provider, index, onEdit, onDelete, onToggle, onTest }: AiProviderRowProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } =
    useSortable({ id: provider.id })
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestAiProviderResult | null>(null)

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : provider.is_enabled ? 1 : 0.5,
  }

  const handleTest = async () => {
    setTesting(true); setTestResult(null)
    try {
      setTestResult(await onTest(provider.id))
    } finally { setTesting(false) }
  }

  return (
    <div ref={setNodeRef} style={style}
      className="flex items-center gap-2 rounded border bg-card p-2">
      <button {...attributes} {...listeners} className="cursor-grab text-muted-foreground">
        <GripVertical className="size-4" />
      </button>
      <Badge variant="outline">#{index + 1}</Badge>
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <span className="font-medium">{provider.name}</span>
          <Badge>{provider.provider_kind}</Badge>
        </div>
        <div className="text-xs text-muted-foreground">{provider.model_name}</div>
        {testResult && (
          <div className={`text-xs ${testResult.ok ? "text-green-600" : "text-destructive"}`}>
            {testResult.ok ? "✓ OK" : `✗ ${testResult.error ?? "failed"}`}
          </div>
        )}
      </div>
      <Switch checked={provider.is_enabled} onCheckedChange={(v) => onToggle(provider.id, v)} />
      <Button variant="outline" size="sm" disabled={testing} onClick={handleTest}>
        {testing && <Loader2 className="mr-1 size-3 animate-spin" />}測試
      </Button>
      <Button variant="outline" size="sm" onClick={() => onEdit(provider)}>
        <Pencil className="size-3" />
      </Button>
      <Button variant="destructive" size="sm" onClick={() => onDelete(provider.id)}>
        <Trash2 className="size-3" />
      </Button>
    </div>
  )
}
```

- [ ] **Step 3: 建立 AiProviderList.tsx**

```typescript
import {
  DndContext, closestCenter, KeyboardSensor, PointerSensor, useSensor, useSensors,
  type DragEndEvent,
} from "@dnd-kit/core"
import {
  SortableContext, verticalListSortingStrategy, sortableKeyboardCoordinates, arrayMove,
} from "@dnd-kit/sortable"
import type { AiProvider, TestAiProviderResult } from "@/schemas/ai"
import { AiProviderRow } from "./AiProviderRow"

export interface AiProviderListProps {
  providers: readonly AiProvider[]
  onReorder: (ordered_ids: number[]) => Promise<void>
  onEdit: (p: AiProvider) => void
  onDelete: (id: number) => void
  onToggle: (id: number, is_enabled: boolean) => void
  onTest: (id: number) => Promise<TestAiProviderResult>
}

export function AiProviderList(props: AiProviderListProps) {
  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )
  const handleDragEnd = (e: DragEndEvent) => {
    if (!e.over || e.active.id === e.over.id) return
    const ids = props.providers.map((p) => p.id)
    const oldIdx = ids.indexOf(Number(e.active.id))
    const newIdx = ids.indexOf(Number(e.over.id))
    const next = arrayMove(ids, oldIdx, newIdx)
    void props.onReorder(next)
  }

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext items={props.providers.map((p) => p.id)} strategy={verticalListSortingStrategy}>
        <div className="space-y-2">
          {props.providers.map((p, i) => (
            <AiProviderRow key={p.id} provider={p} index={i}
              onEdit={props.onEdit} onDelete={props.onDelete}
              onToggle={props.onToggle} onTest={props.onTest} />
          ))}
        </div>
      </SortableContext>
    </DndContext>
  )
}
```

- [ ] **Step 4: 建立 AiProvidersSection.tsx 骨架**

```typescript
import { Effect } from "effect"
import { Plus } from "lucide-react"
import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { CoreApi } from "@/layers/ApiLayer"
import type { AiProvider, CreateAiProviderRequest, UpdateAiProviderRequest } from "@/schemas/ai"
import { useEffectMutation, useEffectQuery } from "@/hooks/useEffectQuery"
import { AiProviderList } from "./AiProviderList"
import { AiProviderEditDialog } from "./AiProviderEditDialog"

export function AiProvidersSection() {
  const { data: providers, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.listAiProviders),
    [],
  )

  const [editing, setEditing] = useState<AiProvider | null | undefined>(undefined)
  // undefined = closed; null = creating; AiProvider = editing existing

  const { mutate: doCreate } = useEffectMutation((req: CreateAiProviderRequest) =>
    Effect.flatMap(CoreApi, (api) => api.createAiProvider(req)))
  const { mutate: doUpdate } = useEffectMutation(
    ({ id, req }: { id: number; req: UpdateAiProviderRequest }) =>
      Effect.flatMap(CoreApi, (api) => api.updateAiProvider(id, req)))
  const { mutate: doDelete } = useEffectMutation((id: number) =>
    Effect.flatMap(CoreApi, (api) => api.deleteAiProvider(id)))
  const { mutate: doReorder } = useEffectMutation((ordered_ids: readonly number[]) =>
    Effect.flatMap(CoreApi, (api) => api.reorderAiProviders(ordered_ids)))
  const { mutate: doTest } = useEffectMutation((id: number) =>
    Effect.flatMap(CoreApi, (api) => api.testAiProvider(id)))

  const list = providers ?? []

  return (
    <Card>
      <CardHeader>
        <CardTitle>AI Providers</CardTitle>
        <CardDescription>
          按優先順序由上至下嘗試；遇到 provider 端故障會 fallback 到下一個。
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {list.length === 0 ? (
          <div className="rounded border border-dashed p-6 text-center text-sm text-muted-foreground">
            尚無 provider，請新增第一個。
          </div>
        ) : (
          <AiProviderList
            providers={list}
            onReorder={async (ids) => { await doReorder(ids); await refetch() }}
            onEdit={(p) => setEditing(p)}
            onDelete={async (id) => { await doDelete(id); await refetch() }}
            onToggle={async (id, is_enabled) => {
              await doUpdate({ id, req: { is_enabled } }); await refetch()
            }}
            onTest={async (id) => (await doTest(id)) ?? { ok: false, error: "no response" }}
          />
        )}

        <Button size="sm" onClick={() => setEditing(null)}>
          <Plus className="mr-1 size-3" /> 新增 Provider
        </Button>

        {editing !== undefined && (
          <AiProviderEditDialog
            provider={editing}
            onClose={() => setEditing(undefined)}
            onSubmit={async (req) => {
              if (editing) {
                await doUpdate({ id: editing.id, req })
              } else {
                await doCreate(req as CreateAiProviderRequest)
              }
              setEditing(undefined)
              await refetch()
            }}
          />
        )}
      </CardContent>
    </Card>
  )
}
```

- [ ] **Step 5: TypeScript 檢查**

Run: `cd /workspace/frontend && bun tsc --noEmit 2>&1 | rg "ai-providers/" | head`
Expected: 因 `AiProviderEditDialog` 尚未建立，會有 import 錯誤——下個 task 處理。

- [ ] **Step 6: Commit**

```bash
cd /workspace
git add frontend/package.json frontend/bun.lock frontend/src/pages/settings/ai-providers/
git commit -m "feat(frontend): AiProvidersSection list + draggable row"
```

---

## Task 16: AiProviderEditDialog（新增 / 編輯共用）

**Files:**
- Create: `frontend/src/pages/settings/ai-providers/AiProviderEditDialog.tsx`

- [ ] **Step 1: 建立 dialog**

```typescript
import { useEffect, useState } from "react"
import { Button } from "@/components/ui/button"
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select"
import type {
  AiProvider, AiProviderKind, CreateAiProviderRequest, ResponseFormatMode, UpdateAiProviderRequest,
} from "@/schemas/ai"

const KINDS: AiProviderKind[] = ["openai_compatible"]
const MODES: ResponseFormatMode[] = ["strict", "non_strict", "inject_schema"]

export interface AiProviderEditDialogProps {
  provider: AiProvider | null
  onClose: () => void
  onSubmit: (req: CreateAiProviderRequest | UpdateAiProviderRequest) => Promise<void>
}

export function AiProviderEditDialog({ provider, onClose, onSubmit }: AiProviderEditDialogProps) {
  const isEdit = provider !== null
  const [name, setName] = useState("")
  const [kind, setKind] = useState<AiProviderKind>("openai_compatible")
  const [baseUrl, setBaseUrl] = useState("")
  const [apiKey, setApiKey] = useState("")
  const [modelName, setModelName] = useState("")
  const [maxTokens, setMaxTokens] = useState("4096")
  const [mode, setMode] = useState<ResponseFormatMode>("non_strict")
  const [enabled, setEnabled] = useState(true)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    if (provider) {
      setName(provider.name)
      setKind(provider.provider_kind)
      setBaseUrl(provider.base_url)
      setApiKey("") // 編輯模式留白；空字串會被視為「不更新」
      setModelName(provider.model_name)
      setMaxTokens(String(provider.max_tokens))
      setMode(provider.response_format_mode)
      setEnabled(provider.is_enabled)
    } else {
      setName(""); setKind("openai_compatible"); setBaseUrl(""); setApiKey("")
      setModelName(""); setMaxTokens("4096"); setMode("non_strict"); setEnabled(true)
    }
  }, [provider])

  const handleSubmit = async () => {
    setSaving(true)
    try {
      if (isEdit) {
        const req: UpdateAiProviderRequest = {
          name, base_url: baseUrl, model_name: modelName,
          max_tokens: Number(maxTokens) || 4096,
          response_format_mode: mode, is_enabled: enabled,
          api_key: apiKey, // 空字串 → 後端保留原值
        }
        await onSubmit(req)
      } else {
        const req: CreateAiProviderRequest = {
          name, provider_kind: kind, base_url: baseUrl,
          api_key: apiKey, model_name: modelName,
          max_tokens: Number(maxTokens) || 4096,
          response_format_mode: mode, is_enabled: enabled,
        }
        await onSubmit(req)
      }
    } finally { setSaving(false) }
  }

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose() }}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{isEdit ? "編輯 Provider" : "新增 Provider"}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1">
            <Label>名稱</Label>
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="space-y-1">
            <Label>協議</Label>
            <Select value={kind} onValueChange={(v) => setKind(v as AiProviderKind)} disabled={isEdit}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {KINDS.map((k) => (<SelectItem key={k} value={k}>{k}</SelectItem>))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label>Base URL</Label>
            <Input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.openai.com/v1" />
          </div>
          <div className="space-y-1">
            <Label>API Key</Label>
            <Input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)}
              placeholder={isEdit ? "••••••••（留空表示不變更）" : "sk-..."} />
          </div>
          <div className="space-y-1">
            <Label>Model 名稱</Label>
            <Input value={modelName} onChange={(e) => setModelName(e.target.value)}
              placeholder="gpt-4o-mini" />
          </div>
          <div className="space-y-1">
            <Label>Max Tokens</Label>
            <Input type="number" value={maxTokens} onChange={(e) => setMaxTokens(e.target.value)}
              min={256} max={128000} className="w-40" />
          </div>
          <div className="space-y-1">
            <Label>Response Format</Label>
            <Select value={mode} onValueChange={(v) => setMode(v as ResponseFormatMode)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {MODES.map((m) => (<SelectItem key={m} value={m}>{m}</SelectItem>))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-2">
            <Switch checked={enabled} onCheckedChange={setEnabled} />
            <Label>啟用</Label>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose} disabled={saving}>取消</Button>
          <Button onClick={handleSubmit} disabled={saving || !name}>儲存</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

- [ ] **Step 2: TypeScript 檢查**

Run: `cd /workspace/frontend && bun tsc --noEmit 2>&1 | rg "ai-providers/" | head`
Expected: 無 error。

- [ ] **Step 3: Commit**

```bash
cd /workspace
git add frontend/src/pages/settings/ai-providers/AiProviderEditDialog.tsx
git commit -m "feat(frontend): AiProviderEditDialog for create/edit"
```

---

## Task 17: 改 SettingsPage 整合新區塊

**Files:**
- Modify: `frontend/src/pages/settings/SettingsPage.tsx`

- [ ] **Step 1: 找出 AI 區塊範圍**

打開 `frontend/src/pages/settings/SettingsPage.tsx`。當前約第 169-300 行內定義了 AI 設定的 state、effect、save/test mutation 與整段 `<Card>...</Card>` JSX，包含：
- `responseFormatOptions` const（第 ~152-167 行）
- `useEffectQuery(getAiSettings)` 與相關 state（第 ~169-188 行）
- `useEffectMutation(updateAiSettings)`、`useEffectMutation(testAiConnection)`（第 ~190-205 行）
- `<Card>` 整段 JSX（第 ~209-301 行）

整段刪除（含 `import type { ResponseFormatMode } from "@/schemas/ai"`、`ai.*` 相關 i18n 用 useTranslation 的 key 仍可保留，但若僅 AI 區塊用到則移除）。

- [ ] **Step 2: 加入新區塊**

頂端 import：
```typescript
import { AiProvidersSection } from "./ai-providers/AiProvidersSection"
```

在原本 AI Card 的位置改成：
```typescript
<AiProvidersSection />
```

- [ ] **Step 3: TypeScript 檢查 + 啟動前端**

Run: `cd /workspace/frontend && bun tsc --noEmit 2>&1 | tail -20`
Expected: 無 error。

Run（背景）：`cd /workspace/frontend && bun run dev`
打開瀏覽器 → `/settings`，預期看到 AI Providers 區塊取代舊的單一表單。手動驗證：
- 列表為空時顯示空狀態
- 「新增 Provider」開 dialog
- 新增後出現在列表，每列有拖曳 handle、enabled switch、測試/編輯/刪除按鈕
- 拖曳兩列重新排序、reload 後順序保留
- 編輯時 api_key 留空儲存後仍可正常 GET（仍遮罩）
- 「測試」按鈕在 base_url 錯誤時顯示 ✗ 訊息

- [ ] **Step 4: Commit**

```bash
cd /workspace
git add frontend/src/pages/settings/SettingsPage.tsx
git commit -m "feat(frontend): swap AI single-form for AiProvidersSection"
```

---

## Task 18: i18n 字串（zh-TW + 其他語系）

**Files:**
- Modify: `frontend/src/locales/*.json`

- [ ] **Step 1: 列出 locale 檔**

Run: `eza /workspace/frontend/src/locales/`

- [ ] **Step 2: 移除舊 `settings.ai.*` 用不到的鍵（若該語言檔仍存）**

只刪除「現在沒有任何元件使用」的鍵；可先 `rg "settings.ai\." /workspace/frontend/src` 找出仍被引用者，避免誤刪。
（`responseFormat_*_label` / `responseFormat_*_desc` 此次改動後未必還用——若 dialog 未引用就刪。）

- [ ] **Step 3: 視需要新增 `settings.aiProviders.*`**

本計畫的元件目前用的是直接寫死的中文字串（"AI Providers"、"新增 Provider"、"按優先順序由上至下嘗試…"）。如需 i18n，把這些字串抽到 locale json：

```json
{
  "settings": {
    "aiProviders": {
      "title": "AI Providers",
      "description": "按優先順序由上至下嘗試；遇到 provider 端故障會 fallback 到下一個。",
      "empty": "尚無 provider，請新增第一個。",
      "addButton": "新增 Provider",
      "edit": "編輯 Provider",
      "create": "新增 Provider",
      "test": "測試",
      "kind": "協議",
      "name": "名稱",
      "baseUrl": "Base URL",
      "apiKey": "API Key",
      "modelName": "Model 名稱",
      "maxTokens": "Max Tokens",
      "responseFormat": "Response Format",
      "enabled": "啟用",
      "save": "儲存",
      "cancel": "取消",
      "apiKeyEditPlaceholder": "••••••••（留空表示不變更）"
    }
  }
}
```

並把 `AiProvidersSection.tsx` / `AiProviderEditDialog.tsx` 中文字串改成 `t("settings.aiProviders.xxx")`。

> 此 step 為可選優化。若專案目前其他 settings 區塊也是寫死中文，可同樣不抽字串以保持一致；最終以「跟現有風格一致」為原則。

- [ ] **Step 4: Commit**

```bash
cd /workspace
git add frontend/src/locales frontend/src/pages/settings/ai-providers
git commit -m "i18n(settings): aiProviders strings"
```

---

## Task 19: End-to-end 整合驗證

**Files:** 無檔案改動，純執行驗證。

- [ ] **Step 1: 整體編譯與測試**

```bash
cd /workspace
cargo fmt --all
cargo clippy -p core-service -- -D warnings
cargo test -p core-service
```
Expected: All pass，無 clippy warning。

- [ ] **Step 2: 開啟 dev 環境跑端到端**

```bash
docker-compose -f docker-compose.dev.yaml up -d
cargo run --bin core-service &
cd frontend && bun run dev &
```

手動驗證：
1. 設定兩個 provider：第一個 base_url 故意填錯（例 `https://invalid.example.com`），第二個填正確的 OpenAI-compatible 端點。
2. 在 raw items 頁面觸發一次「AI 解析」。
3. 觀察 core-service 日誌，預期看到第一個 provider 的 warn log（fallback）然後第二個成功。
4. 翻轉順序（用 UI 拖曳），再觸發一次解析；觀察日誌是反過來的。
5. 把兩個 provider 都 disable，觸發解析應回 `NotConfigured`。
6. 把第一個改成回 4xx auth 錯誤（用無效 api_key），觸發 → 應**不**fallback，直接失敗（`ApiError`，因為 api_key 錯多半會被 OpenAI 回 401）。

- [ ] **Step 3: Commit（如果驗證中發現任何小修補）**

如有 hotfix：

```bash
cd /workspace
git add <changed-files>
git commit -m "fix(ai): <issue>"
```

---

## Task 20: 寫入 PROGRESS.md 與整理

**Files:**
- Modify: `docs/PROGRESS.md`

- [ ] **Step 1: 在 docs/PROGRESS.md 加上一條目**

依現有格式追加：
```markdown
## 2026-04-30 — Multi AI providers + fallback chain

- DB: `ai_settings` → `ai_providers`（migration 自動搬遷）
- Backend: `AiProviderChain` 實作 fallback；錯誤分類 ProviderUnavailable vs ApiError
- API: `/ai-providers` CRUD/reorder/test
- Frontend: 列表 + dialog + 拖曳排序
```

- [ ] **Step 2: 最終 commit**

```bash
cd /workspace
git add docs/PROGRESS.md
git commit -m "docs(progress): multi AI providers feature"
```

---

## Self-review checklist

- [x] 所有 spec 章節（1-6）皆對應到 task：DB(1) → AiProviderChain & errors(3-6) → API(9-11) → 前端(14-17) → 測試(3,4,5,6,12) → migration(1,19)
- [x] 無 placeholder / "TBD" / "TODO"
- [x] 每個 step 給了完整程式碼或精確指令
- [x] 函數/型別命名一致：`build_ai_chain`、`AiProviderChain`、`AttemptRecord`、`ChainEntry`、`AiProvider`、`UpdateAiProvider`、`is_retryable`
- [x] 路由、handler 名稱前後一致：`list_ai_providers` / `create_ai_provider` / `get_ai_provider` / `update_ai_provider` / `delete_ai_provider` / `reorder_ai_providers` / `test_ai_provider`
- [x] 前端 API 方法名稱與 layer 一致：`listAiProviders` / `createAiProvider` / `updateAiProvider` / `deleteAiProvider` / `reorderAiProviders` / `testAiProvider`
- [x] Migration up/down 對稱；無資料時也能跑（`SELECT ... FROM ai_settings` 0 列安全）
- [x] 路由註冊順序：reorder 放在 `:id` 之前（避免 matchit 歧義）
