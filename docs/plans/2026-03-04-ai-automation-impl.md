# AI 自動化 Parser/Filter 生成實作計劃

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 整合 OpenAI-compatible AI 自動生成 parser/filter，解析失敗或 conflict 時自動觸發，使用者在統一待確認區確認後套用。

**Architecture:** AI 模組嵌入 core-service（trait 抽象），新增 `pending_ai_results` 表作為統一佇列；parser/filter 建立後以 `pending_result_id` 標記未確認狀態；前端新增 `/pending` 與 `/settings` 頁面，訂閱新增改為三步驟 Wizard。

**Tech Stack:** Rust/Axum/Diesel (backend), React/TypeScript/Effect.ts/shadcn-ui (frontend), PostgreSQL, OpenAI-compatible API (reqwest 0.12)

**Design Doc:** `docs/plans/2026-03-04-ai-automation-design.md`

---

## Phase 1: DB Migrations

### Task 1: 新增三個新表的 Migration

**Files:**
- Create: `core-service/migrations/2026-03-04-000000-add-ai-tables/up.sql`
- Create: `core-service/migrations/2026-03-04-000000-add-ai-tables/down.sql`

**Step 1: 建立 migration 目錄和 up.sql**

```sql
-- up.sql
CREATE TABLE ai_settings (
    id         SERIAL PRIMARY KEY,
    base_url   TEXT NOT NULL DEFAULT '',
    api_key    TEXT NOT NULL DEFAULT '',
    model_name TEXT NOT NULL DEFAULT 'gpt-4o-mini',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);
-- 確保只有一筆記錄
INSERT INTO ai_settings (base_url, api_key, model_name) VALUES ('', '', 'gpt-4o-mini');

CREATE TABLE ai_prompt_settings (
    id                   SERIAL PRIMARY KEY,
    fixed_parser_prompt  TEXT,
    fixed_filter_prompt  TEXT,
    custom_parser_prompt TEXT,
    custom_filter_prompt TEXT,
    created_at           TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP NOT NULL DEFAULT NOW()
);
-- 確保只有一筆記錄（預設值由應用層 revert 寫入）
INSERT INTO ai_prompt_settings DEFAULT VALUES;

CREATE TABLE pending_ai_results (
    id                 SERIAL PRIMARY KEY,
    result_type        TEXT NOT NULL CHECK (result_type IN ('parser', 'filter')),
    source_title       TEXT NOT NULL,
    generated_data     JSONB,
    status             TEXT NOT NULL DEFAULT 'generating'
                           CHECK (status IN ('generating', 'pending', 'confirmed', 'failed')),
    error_message      TEXT,
    raw_item_id        INT REFERENCES raw_anime_items(item_id) ON DELETE SET NULL,
    used_fixed_prompt  TEXT NOT NULL DEFAULT '',
    used_custom_prompt TEXT,
    expires_at         TIMESTAMP,
    created_at         TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMP NOT NULL DEFAULT NOW()
);
```

**Step 2: 建立 down.sql**

```sql
-- down.sql
DROP TABLE IF EXISTS pending_ai_results;
DROP TABLE IF EXISTS ai_prompt_settings;
DROP TABLE IF EXISTS ai_settings;
```

**Step 3: Commit**

```bash
git add core-service/migrations/2026-03-04-000000-add-ai-tables/
git commit -m "feat(db): add ai_settings, ai_prompt_settings, pending_ai_results tables"
```

---

### Task 2: 修改 title_parsers 和 filter_rules，移除 Catch-All parser

**Files:**
- Create: `core-service/migrations/2026-03-04-000001-add-pending-result-id/up.sql`
- Create: `core-service/migrations/2026-03-04-000001-add-pending-result-id/down.sql`
- Create: `core-service/migrations/2026-03-04-000002-remove-catchall-parser/up.sql`
- Create: `core-service/migrations/2026-03-04-000002-remove-catchall-parser/down.sql`

**Step 1: 建立 pending_result_id migration**

```sql
-- 2026-03-04-000001-add-pending-result-id/up.sql
ALTER TABLE title_parsers
    ADD COLUMN pending_result_id INT REFERENCES pending_ai_results(id) ON DELETE SET NULL;

ALTER TABLE filter_rules
    ADD COLUMN pending_result_id INT REFERENCES pending_ai_results(id) ON DELETE SET NULL;
```

```sql
-- 2026-03-04-000001-add-pending-result-id/down.sql
ALTER TABLE title_parsers DROP COLUMN IF EXISTS pending_result_id;
ALTER TABLE filter_rules  DROP COLUMN IF EXISTS pending_result_id;
```

**Step 2: 建立移除 Catch-All migration**

```sql
-- 2026-03-04-000002-remove-catchall-parser/up.sql
-- 移除 Catch-All 解析器（priority=0，條件 .+，名稱含「全匹配」）
DELETE FROM title_parsers WHERE name = 'Catch-All 全匹配';
```

```sql
-- 2026-03-04-000002-remove-catchall-parser/down.sql
-- 無法回復刪除的種子資料，僅記錄
```

**Step 3: Commit**

```bash
git add core-service/migrations/2026-03-04-000001-add-pending-result-id/
git add core-service/migrations/2026-03-04-000002-remove-catchall-parser/
git commit -m "feat(db): add pending_result_id to parsers/filters, remove catch-all parser"
```

---

## Phase 2: Rust Models & Schema

### Task 3: 更新 schema.rs 和 models/db.rs

**Files:**
- Modify: `core-service/src/schema.rs`
- Modify: `core-service/src/models/db.rs`

**Step 1: 在 schema.rs 新增三個新表**

在 `schema.rs` 末尾加入（在 `allow_tables_to_appear_in_same_query!` 之前）：

```rust
diesel::table! {
    ai_settings (id) {
        id         -> Int4,
        base_url   -> Text,
        api_key    -> Text,
        model_name -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    ai_prompt_settings (id) {
        id                   -> Int4,
        fixed_parser_prompt  -> Nullable<Text>,
        fixed_filter_prompt  -> Nullable<Text>,
        custom_parser_prompt -> Nullable<Text>,
        custom_filter_prompt -> Nullable<Text>,
        created_at           -> Timestamp,
        updated_at           -> Timestamp,
    }
}

diesel::table! {
    pending_ai_results (id) {
        id                 -> Int4,
        result_type        -> Text,
        source_title       -> Text,
        generated_data     -> Nullable<Jsonb>,
        status             -> Text,
        error_message      -> Nullable<Text>,
        raw_item_id        -> Nullable<Int4>,
        used_fixed_prompt  -> Text,
        used_custom_prompt -> Nullable<Text>,
        expires_at         -> Nullable<Timestamp>,
        created_at         -> Timestamp,
        updated_at         -> Timestamp,
    }
}
```

在 title_parsers 表定義最後加入：
```rust
pending_result_id -> Nullable<Int4>,
```

在 filter_rules 表定義最後加入：
```rust
pending_result_id -> Nullable<Int4>,
```

**Step 2: 在 models/db.rs 新增 Model structs**

```rust
// AiSettings
#[derive(Queryable, Selectable, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[diesel(table_name = crate::schema::ai_settings)]
pub struct AiSettings {
    pub id: i32,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(AsChangeset, Debug, serde::Deserialize)]
#[diesel(table_name = crate::schema::ai_settings)]
pub struct UpdateAiSettings {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub updated_at: NaiveDateTime,
}

// AiPromptSettings
#[derive(Queryable, Selectable, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[diesel(table_name = crate::schema::ai_prompt_settings)]
pub struct AiPromptSettings {
    pub id: i32,
    pub fixed_parser_prompt: Option<String>,
    pub fixed_filter_prompt: Option<String>,
    pub custom_parser_prompt: Option<String>,
    pub custom_filter_prompt: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(AsChangeset, Debug)]
#[diesel(table_name = crate::schema::ai_prompt_settings)]
pub struct UpdateAiPromptSettings {
    pub fixed_parser_prompt: Option<Option<String>>,
    pub fixed_filter_prompt: Option<Option<String>>,
    pub custom_parser_prompt: Option<Option<String>>,
    pub custom_filter_prompt: Option<Option<String>>,
    pub updated_at: NaiveDateTime,
}

// PendingAiResult
#[derive(Queryable, Selectable, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[diesel(table_name = crate::schema::pending_ai_results)]
pub struct PendingAiResult {
    pub id: i32,
    pub result_type: String,
    pub source_title: String,
    pub generated_data: Option<serde_json::Value>,
    pub status: String,
    pub error_message: Option<String>,
    pub raw_item_id: Option<i32>,
    pub used_fixed_prompt: String,
    pub used_custom_prompt: Option<String>,
    pub expires_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::pending_ai_results)]
pub struct NewPendingAiResult {
    pub result_type: String,
    pub source_title: String,
    pub generated_data: Option<serde_json::Value>,
    pub status: String,
    pub error_message: Option<String>,
    pub raw_item_id: Option<i32>,
    pub used_fixed_prompt: String,
    pub used_custom_prompt: Option<String>,
    pub expires_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
```

**Step 3: 更新 TitleParser 和 FilterRule structs，新增 pending_result_id 欄位**

在 `TitleParser` struct 末尾（`episode_end_value` 之後）加入：
```rust
pub pending_result_id: Option<i32>,
```

在 `FilterRule` struct 末尾加入：
```rust
pub pending_result_id: Option<i32>,
```

同樣更新各自的 `NewTitleParser` / `NewFilterRule` insertable structs。

**Step 4: 確認編譯成功**

```bash
cd core-service && cargo check 2>&1 | head -50
```
Expected: 0 errors（可能有 warnings 關於未使用欄位，可忽略）

**Step 5: Commit**

```bash
git add core-service/src/schema.rs core-service/src/models/db.rs
git commit -m "feat(models): add AI tables and pending_result_id to parsers/filters"
```

---

## Phase 3: AI 模組

### Task 4: AI Client Trait 與 OpenAI 實作

**Files:**
- Create: `core-service/src/ai/mod.rs`
- Create: `core-service/src/ai/client.rs`
- Create: `core-service/src/ai/openai.rs`
- Modify: `core-service/src/main.rs`（加 `mod ai;`）

**Step 1: 建立 `src/ai/client.rs`**

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
    #[error("AI error: {0}")]
    ApiError(String),
}

#[async_trait]
pub trait AiClient: Send + Sync {
    async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, AiError>;
}
```

**Step 2: 建立 `src/ai/openai.rs`**

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::client::{AiClient, AiError};

pub struct OpenAiClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiClient {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    fmt_type: &'static str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: AssistantMessage,
}

#[derive(Deserialize)]
struct AssistantMessage {
    content: String,
}

#[async_trait]
impl AiClient for OpenAiClient {
    async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, AiError> {
        if self.api_key.is_empty() {
            return Err(AiError::NotConfigured);
        }

        let mut messages = vec![];
        if !system_prompt.is_empty() {
            messages.push(Message { role: "system", content: system_prompt });
        }
        messages.push(Message { role: "user", content: user_prompt });

        let body = ChatRequest {
            model: &self.model,
            messages,
            response_format: ResponseFormat { fmt_type: "json_object" },
        };

        let resp = self.http
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AiError::ApiError(text));
        }

        let chat: ChatResponse = resp.json().await?;
        chat.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| AiError::ApiError("Empty choices".into()))
    }
}
```

**Step 3: 建立 `src/ai/mod.rs`**

```rust
pub mod client;
pub mod openai;
pub mod prompts;
pub mod parser_generator;
pub mod filter_generator;

pub use client::{AiClient, AiError};
pub use openai::OpenAiClient;
```

**Step 4: 在 `main.rs` 加入 `mod ai;`**

在其他 mod 宣告後加入：
```rust
mod ai;
```

**Step 5: 確認編譯**

```bash
cd core-service && cargo check 2>&1 | head -30
```

**Step 6: Commit**

```bash
git add core-service/src/ai/
git add core-service/src/main.rs
git commit -m "feat(ai): add AiClient trait and OpenAI-compatible implementation"
```

---

### Task 5: Prompt 組裝與 Parser/Filter 生成器

**Files:**
- Create: `core-service/src/ai/prompts.rs`
- Create: `core-service/src/ai/parser_generator.rs`
- Create: `core-service/src/ai/filter_generator.rs`

**Step 1: 建立 `src/ai/prompts.rs`**

```rust
/// Parser 固定 Prompt 預設值（revert 時使用）
pub const DEFAULT_FIXED_PARSER_PROMPT: &str = r#"你是一個動畫資料解析專家。根據提供的動畫標題，生成一個正則表達式解析器設定。
返回 JSON 格式，包含以下欄位：
- name: 解析器名稱（字串）
- condition_regex: 標題匹配條件（正則表達式字串）
- parse_regex: 解析用正則表達式，使用命名群組（字串）
- anime_title_source: "regex" 或 "static"
- anime_title_value: 如果是 regex，填命名群組名稱；如果是 static，填固定值
- episode_no_source: "regex" 或 "static"
- episode_no_value: 集數來源
- subtitle_group_source: "regex" 或 "static" 或 null
- subtitle_group_value: 字幕組來源或 null
- resolution_source: "regex" 或 "static" 或 null
- resolution_value: 解析度來源或 null
確保 parse_regex 的命名群組與對應的 *_value 欄位匹配。"#;

/// Filter 固定 Prompt 預設值
pub const DEFAULT_FIXED_FILTER_PROMPT: &str = r#"你是一個動畫過濾規則專家。根據提供的衝突動畫標題列表，生成過濾規則。
返回 JSON 格式，包含 rules 陣列，每個規則包含：
- regex_pattern: 過濾用正則表達式（字串）
- is_positive: true 表示保留匹配項，false 表示排除匹配項（布林值）
- rule_order: 規則順序，從 1 開始（整數）
目標是讓每個訂閱只保留最符合的集數。"#;

/// 組裝最終的 system prompt
pub fn build_system_prompt(fixed: Option<&str>) -> String {
    fixed.unwrap_or("").to_string()
}

/// 組裝 parser 的 user prompt
pub fn build_parser_user_prompt(title: &str, custom: Option<&str>) -> String {
    let mut s = format!("動畫標題：{}", title);
    if let Some(c) = custom {
        if !c.is_empty() {
            s.push_str("\n\n");
            s.push_str(c);
        }
    }
    s
}

/// 組裝 filter 的 user prompt（多個衝突標題）
pub fn build_filter_user_prompt(titles: &[String], custom: Option<&str>) -> String {
    let titles_str = titles.iter()
        .enumerate()
        .map(|(i, t)| format!("{}. {}", i + 1, t))
        .collect::<Vec<_>>()
        .join("\n");
    let mut s = format!("衝突的動畫標題列表：\n{}", titles_str);
    if let Some(c) = custom {
        if !c.is_empty() {
            s.push_str("\n\n");
            s.push_str(c);
        }
    }
    s
}
```

**Step 2: 建立 `src/ai/parser_generator.rs`**

```rust
use chrono::Utc;
use diesel::prelude::*;
use serde_json::Value;
use std::sync::Arc;

use crate::db::DbPool;
use crate::models::{NewPendingAiResult, NewTitleParser, PendingAiResult};
use crate::schema::{ai_prompt_settings, ai_settings, pending_ai_results, title_parsers};
use super::client::{AiClient, AiError};
use super::openai::OpenAiClient;
use super::prompts::*;

/// 從 DB 取得 AiClient，如果未設定則回傳 None
pub fn build_ai_client(conn: &mut PgConnection) -> Result<Option<OpenAiClient>, String> {
    let settings = ai_settings::table
        .first::<crate::models::AiSettings>(conn)
        .optional()
        .map_err(|e| e.to_string())?;

    match settings {
        Some(s) if !s.api_key.is_empty() && !s.base_url.is_empty() => {
            Ok(Some(OpenAiClient::new(&s.base_url, &s.api_key, &s.model_name)))
        }
        _ => Ok(None),
    }
}

/// 為單一動畫標題生成 parser（背景非同步觸發）
pub async fn generate_parser_for_title(
    pool: Arc<DbPool>,
    source_title: String,
    raw_item_id: Option<i32>,
    temp_custom_prompt: Option<String>,  // None = 使用 DB 設定
) -> Result<PendingAiResult, String> {
    let now = Utc::now().naive_utc();

    // 取得 prompt 設定
    let (fixed_prompt, custom_prompt) = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let prompt_settings = ai_prompt_settings::table
            .first::<crate::models::AiPromptSettings>(&mut conn)
            .optional()
            .map_err(|e| e.to_string())?;
        let fixed = prompt_settings.as_ref()
            .and_then(|p| p.fixed_parser_prompt.clone())
            .unwrap_or_else(|| DEFAULT_FIXED_PARSER_PROMPT.to_string());
        let custom = temp_custom_prompt.or_else(|| {
            prompt_settings.and_then(|p| p.custom_parser_prompt)
        });
        (fixed, custom)
    };

    // 建立 pending record（status=generating）
    let pending = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        diesel::insert_into(pending_ai_results::table)
            .values(NewPendingAiResult {
                result_type: "parser".to_string(),
                source_title: source_title.clone(),
                generated_data: None,
                status: "generating".to_string(),
                error_message: None,
                raw_item_id,
                used_fixed_prompt: fixed_prompt.clone(),
                used_custom_prompt: custom_prompt.clone(),
                expires_at: None,
                created_at: now,
                updated_at: now,
            })
            .get_result::<PendingAiResult>(&mut conn)
            .map_err(|e| e.to_string())?
    };

    let pending_id = pending.id;

    // 建立 AI client 並呼叫
    let client_result = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        build_ai_client(&mut conn)
    };

    let ai_result = match client_result {
        Ok(Some(client)) => {
            let system = build_system_prompt(Some(&fixed_prompt));
            let user = build_parser_user_prompt(&source_title, custom_prompt.as_deref());
            client.chat_completion(&system, &user).await
        }
        Ok(None) => Err(AiError::NotConfigured),
        Err(e) => Err(AiError::ApiError(e)),
    };

    match ai_result {
        Ok(json_str) => {
            match serde_json::from_str::<Value>(&json_str) {
                Ok(data) => {
                    // 驗證必要欄位
                    if data.get("condition_regex").is_none() || data.get("parse_regex").is_none() {
                        let err = "AI 返回的 JSON 缺少必要欄位 condition_regex/parse_regex".to_string();
                        return update_pending_failed(&pool, pending_id, &err).await;
                    }
                    // 建立未確認 parser
                    let parser = create_unconfirmed_parser(&pool, &data, pending_id).await?;
                    tracing::info!("parser_id={} 已建立（未確認）", parser);
                    // 更新 pending status=pending
                    update_pending_success(&pool, pending_id, data).await
                }
                Err(e) => {
                    update_pending_failed(&pool, pending_id, &format!("JSON 解析失敗: {}", e)).await
                }
            }
        }
        Err(e) => {
            update_pending_failed(&pool, pending_id, &e.to_string()).await
        }
    }
}

async fn create_unconfirmed_parser(
    pool: &Arc<DbPool>,
    data: &Value,
    pending_id: i32,
) -> Result<i32, String> {
    let now = Utc::now().naive_utc();
    let mut conn = pool.get().map_err(|e| e.to_string())?;

    let get_str = |key: &str| -> String {
        data.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
    };
    let get_opt_str = |key: &str| -> Option<String> {
        data.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string())
    };

    use crate::models::ParserSourceType;
    let parse_source = |key: &str| -> ParserSourceType {
        match data.get(key).and_then(|v| v.as_str()) {
            Some("static") => ParserSourceType::Static,
            _ => ParserSourceType::Regex,
        }
    };

    let new_parser = NewTitleParser {
        name: get_str("name"),
        description: None,
        priority: 50,  // 預設中等優先級
        is_enabled: true,
        condition_regex: get_str("condition_regex"),
        parse_regex: get_str("parse_regex"),
        anime_title_source: parse_source("anime_title_source"),
        anime_title_value: get_str("anime_title_value"),
        episode_no_source: parse_source("episode_no_source"),
        episode_no_value: get_str("episode_no_value"),
        series_no_source: None,
        series_no_value: None,
        subtitle_group_source: get_opt_str("subtitle_group_source").map(|s| if s == "static" { ParserSourceType::Static } else { ParserSourceType::Regex }),
        subtitle_group_value: get_opt_str("subtitle_group_value"),
        resolution_source: get_opt_str("resolution_source").map(|s| if s == "static" { ParserSourceType::Static } else { ParserSourceType::Regex }),
        resolution_value: get_opt_str("resolution_value"),
        season_source: None,
        season_value: None,
        year_source: None,
        year_value: None,
        created_at: now,
        updated_at: now,
        created_from_type: None,
        created_from_id: None,
        episode_end_source: None,
        episode_end_value: None,
        pending_result_id: Some(pending_id),
    };

    diesel::insert_into(title_parsers::table)
        .values(&new_parser)
        .returning(title_parsers::parser_id)
        .get_result::<i32>(&mut conn)
        .map_err(|e| e.to_string())
}

async fn update_pending_success(
    pool: &Arc<DbPool>,
    pending_id: i32,
    data: Value,
) -> Result<PendingAiResult, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    diesel::update(pending_ai_results::table.find(pending_id))
        .set((
            pending_ai_results::status.eq("pending"),
            pending_ai_results::generated_data.eq(Some(data)),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| e.to_string())
}

async fn update_pending_failed(
    pool: &Arc<DbPool>,
    pending_id: i32,
    error: &str,
) -> Result<PendingAiResult, String> {
    tracing::warn!("AI parser 生成失敗 pending_id={}: {}", pending_id, error);
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    diesel::update(pending_ai_results::table.find(pending_id))
        .set((
            pending_ai_results::status.eq("failed"),
            pending_ai_results::error_message.eq(error),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| e.to_string())
}
```

**Step 3: 建立 `src/ai/filter_generator.rs`**

邏輯與 `parser_generator.rs` 類似，輸入改為衝突的動畫標題列表，寫入 filter_rules：

```rust
use chrono::Utc;
use diesel::prelude::*;
use serde_json::Value;
use std::sync::Arc;

use crate::db::DbPool;
use crate::models::{FilterTargetType, NewFilterRule, NewPendingAiResult, PendingAiResult};
use crate::schema::{ai_prompt_settings, ai_settings, filter_rules, pending_ai_results};
use super::client::AiError;
use super::parser_generator::build_ai_client;
use super::prompts::*;

pub async fn generate_filter_for_conflict(
    pool: Arc<DbPool>,
    conflict_titles: Vec<String>,   // 所有衝突的動畫標題
    source_title: String,           // 主要來源標題（用於顯示）
    temp_custom_prompt: Option<String>,
) -> Result<PendingAiResult, String> {
    let now = Utc::now().naive_utc();

    let (fixed_prompt, custom_prompt) = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let prompt_settings = ai_prompt_settings::table
            .first::<crate::models::AiPromptSettings>(&mut conn)
            .optional()
            .map_err(|e| e.to_string())?;
        let fixed = prompt_settings.as_ref()
            .and_then(|p| p.fixed_filter_prompt.clone())
            .unwrap_or_else(|| DEFAULT_FIXED_FILTER_PROMPT.to_string());
        let custom = temp_custom_prompt.or_else(|| {
            prompt_settings.and_then(|p| p.custom_filter_prompt)
        });
        (fixed, custom)
    };

    let pending = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        diesel::insert_into(pending_ai_results::table)
            .values(NewPendingAiResult {
                result_type: "filter".to_string(),
                source_title: source_title.clone(),
                generated_data: None,
                status: "generating".to_string(),
                error_message: None,
                raw_item_id: None,
                used_fixed_prompt: fixed_prompt.clone(),
                used_custom_prompt: custom_prompt.clone(),
                expires_at: None,
                created_at: now,
                updated_at: now,
            })
            .get_result::<PendingAiResult>(&mut conn)
            .map_err(|e| e.to_string())?
    };

    let pending_id = pending.id;

    let client_result = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        build_ai_client(&mut conn)
    };

    let ai_result = match client_result {
        Ok(Some(client)) => {
            let system = build_system_prompt(Some(&fixed_prompt));
            let user = build_filter_user_prompt(&conflict_titles, custom_prompt.as_deref());
            client.chat_completion(&system, &user).await
        }
        Ok(None) => Err(AiError::NotConfigured),
        Err(e) => Err(AiError::ApiError(e)),
    };

    match ai_result {
        Ok(json_str) => {
            match serde_json::from_str::<Value>(&json_str) {
                Ok(data) => {
                    if data.get("rules").and_then(|r| r.as_array()).is_none() {
                        let err = "AI 返回的 JSON 缺少 rules 陣列".to_string();
                        return update_filter_pending_failed(&pool, pending_id, &err).await;
                    }
                    create_unconfirmed_filter_rules(&pool, &data, pending_id).await?;
                    update_filter_pending_success(&pool, pending_id, data).await
                }
                Err(e) => {
                    update_filter_pending_failed(&pool, pending_id, &format!("JSON 解析失敗: {}", e)).await
                }
            }
        }
        Err(e) => update_filter_pending_failed(&pool, pending_id, &e.to_string()).await,
    }
}

async fn create_unconfirmed_filter_rules(
    pool: &Arc<DbPool>,
    data: &Value,
    pending_id: i32,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let mut conn = pool.get().map_err(|e| e.to_string())?;

    let rules = data["rules"].as_array().unwrap();
    for rule in rules {
        let new_rule = NewFilterRule {
            rule_order: rule.get("rule_order").and_then(|v| v.as_i64()).unwrap_or(1) as i32,
            regex_pattern: rule.get("regex_pattern").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            is_positive: rule.get("is_positive").and_then(|v| v.as_bool()).unwrap_or(true),
            target_type: FilterTargetType::Global,
            target_id: None,
            created_at: now,
            updated_at: now,
            pending_result_id: Some(pending_id),
        };
        diesel::insert_into(filter_rules::table)
            .values(&new_rule)
            .execute(&mut conn)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn update_filter_pending_success(pool: &Arc<DbPool>, id: i32, data: Value) -> Result<PendingAiResult, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    diesel::update(pending_ai_results::table.find(id))
        .set((
            pending_ai_results::status.eq("pending"),
            pending_ai_results::generated_data.eq(Some(data)),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| e.to_string())
}

async fn update_filter_pending_failed(pool: &Arc<DbPool>, id: i32, error: &str) -> Result<PendingAiResult, String> {
    tracing::warn!("AI filter 生成失敗 pending_id={}: {}", id, error);
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    diesel::update(pending_ai_results::table.find(id))
        .set((
            pending_ai_results::status.eq("failed"),
            pending_ai_results::error_message.eq(error),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| e.to_string())
}
```

**Step 4: 確認編譯**

```bash
cd core-service && cargo check 2>&1 | head -30
```

**Step 5: Commit**

```bash
git add core-service/src/ai/
git commit -m "feat(ai): add prompts, parser_generator, filter_generator"
```

---

## Phase 4: Handlers & Routes

### Task 6: 新增 AI 相關 Handlers

**Files:**
- Create: `core-service/src/handlers/ai_settings.rs`
- Create: `core-service/src/handlers/pending_ai_results.rs`
- Modify: `core-service/src/handlers/mod.rs`（新增 pub mod）

**Step 1: 建立 `src/handlers/ai_settings.rs`**

```rust
use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::{AiPromptSettings, AiSettings, UpdateAiSettings};
use crate::schema::{ai_prompt_settings, ai_settings};
use crate::state::AppState;
use crate::ai::prompts::{DEFAULT_FIXED_FILTER_PROMPT, DEFAULT_FIXED_PARSER_PROMPT};

// GET /ai-settings
pub async fn get_ai_settings(
    State(state): State<AppState>,
) -> Result<Json<AiSettings>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let settings = ai_settings::table
        .first::<AiSettings>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // 遮罩 api_key
    Ok(Json(AiSettings { api_key: "•".repeat(8), ..settings }))
}

#[derive(Debug, Deserialize)]
pub struct UpdateAiSettingsRequest {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
}

// PUT /ai-settings
pub async fn update_ai_settings(
    State(state): State<AppState>,
    Json(req): Json<UpdateAiSettingsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = Utc::now().naive_utc();
    diesel::update(ai_settings::table)
        .set(UpdateAiSettings {
            base_url: req.base_url,
            api_key: req.api_key,
            model_name: req.model_name,
            updated_at: now,
        })
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// GET /ai-prompt-settings
pub async fn get_ai_prompt_settings(
    State(state): State<AppState>,
) -> Result<Json<AiPromptSettings>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let settings = ai_prompt_settings::table
        .first::<AiPromptSettings>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(settings))
}

#[derive(Debug, Deserialize)]
pub struct UpdateAiPromptSettingsRequest {
    pub fixed_parser_prompt: Option<String>,
    pub fixed_filter_prompt: Option<String>,
    pub custom_parser_prompt: Option<String>,
    pub custom_filter_prompt: Option<String>,
}

// PUT /ai-prompt-settings
pub async fn update_ai_prompt_settings(
    State(state): State<AppState>,
    Json(req): Json<UpdateAiPromptSettingsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = Utc::now().naive_utc();
    diesel::update(ai_prompt_settings::table)
        .set((
            ai_prompt_settings::fixed_parser_prompt.eq(req.fixed_parser_prompt),
            ai_prompt_settings::fixed_filter_prompt.eq(req.fixed_filter_prompt),
            ai_prompt_settings::custom_parser_prompt.eq(req.custom_parser_prompt),
            ai_prompt_settings::custom_filter_prompt.eq(req.custom_filter_prompt),
            ai_prompt_settings::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// POST /ai-prompt-settings/revert-parser
pub async fn revert_parser_prompt(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    diesel::update(ai_prompt_settings::table)
        .set((
            ai_prompt_settings::fixed_parser_prompt.eq(Some(DEFAULT_FIXED_PARSER_PROMPT)),
            ai_prompt_settings::updated_at.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true, "value": DEFAULT_FIXED_PARSER_PROMPT })))
}

// POST /ai-prompt-settings/revert-filter
pub async fn revert_filter_prompt(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    diesel::update(ai_prompt_settings::table)
        .set((
            ai_prompt_settings::fixed_filter_prompt.eq(Some(DEFAULT_FIXED_FILTER_PROMPT)),
            ai_prompt_settings::updated_at.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true, "value": DEFAULT_FIXED_FILTER_PROMPT })))
}

// POST /ai-settings/test
pub async fn test_ai_connection(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match crate::ai::parser_generator::build_ai_client(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        Some(client) => {
            use crate::ai::client::AiClient;
            match client.chat_completion("", "test").await {
                Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
                Err(e) => Ok(Json(serde_json::json!({ "ok": false, "error": e.to_string() }))),
            }
        }
        None => Ok(Json(serde_json::json!({ "ok": false, "error": "AI 未設定" }))),
    }
}
```

**Step 2: 建立 `src/handlers/pending_ai_results.rs`**

```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::models::{FilterTargetType, PendingAiResult};
use crate::schema::{filter_rules, pending_ai_results, title_parsers};
use crate::state::AppState;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ListPendingQuery {
    pub result_type: Option<String>,
    pub status: Option<String>,
}

// GET /pending-ai-results
pub async fn list_pending(
    State(state): State<AppState>,
    Query(q): Query<ListPendingQuery>,
) -> Result<Json<Vec<PendingAiResult>>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut query = pending_ai_results::table
        .filter(pending_ai_results::expires_at.is_null()
            .or(pending_ai_results::expires_at.gt(Utc::now().naive_utc())))
        .into_boxed();

    if let Some(t) = q.result_type {
        query = query.filter(pending_ai_results::result_type.eq(t));
    }
    if let Some(s) = q.status {
        query = query.filter(pending_ai_results::status.eq(s));
    }

    let results = query
        .order(pending_ai_results::created_at.desc())
        .load::<PendingAiResult>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(results))
}

// GET /pending-ai-results/:id
pub async fn get_pending(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<PendingAiResult>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let result = pending_ai_results::table
        .find(id)
        .first::<PendingAiResult>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Not found".to_string()))?;
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct UpdatePendingRequest {
    pub generated_data: serde_json::Value,
}

// PUT /pending-ai-results/:id
pub async fn update_pending(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<UpdatePendingRequest>,
) -> Result<Json<PendingAiResult>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let result = diesel::update(pending_ai_results::table.find(id))
        .set((
            pending_ai_results::generated_data.eq(Some(req.generated_data)),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct ConfirmRequest {
    pub level: String,  // "global" | "subscription" | "anime_work"
    pub target_id: Option<i32>,
}

// POST /pending-ai-results/:id/confirm
pub async fn confirm_pending(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<ConfirmRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let pool = state.db.clone();
    let now = Utc::now().naive_utc();
    let expires_at = now + chrono::Duration::days(7);

    let mut conn = pool.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let pending = pending_ai_results::table
        .find(id)
        .first::<PendingAiResult>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Not found".to_string()))?;

    let target_type = match req.level.as_str() {
        "subscription" => FilterTargetType::Subscription,
        "anime_work" => FilterTargetType::AnimeWork,
        _ => FilterTargetType::Global,
    };

    match pending.result_type.as_str() {
        "parser" => {
            // 更新 title_parsers：清除 pending_result_id，設定 created_from_type/id
            diesel::update(title_parsers::table
                .filter(title_parsers::pending_result_id.eq(id)))
                .set((
                    title_parsers::pending_result_id.eq(None::<i32>),
                    title_parsers::created_from_type.eq(Some(target_type)),
                    title_parsers::created_from_id.eq(req.target_id),
                    title_parsers::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            // 觸發 re-run（背景非同步）
            let pool_arc = Arc::new(pool.clone());
            tokio::spawn(async move {
                if let Err(e) = rerun_unmatched_raw_items(pool_arc).await {
                    tracing::warn!("rerun_unmatched_raw_items 失敗: {}", e);
                }
            });
        }
        "filter" => {
            // 更新 filter_rules：清除 pending_result_id，設定 target_type/id
            diesel::update(filter_rules::table
                .filter(filter_rules::pending_result_id.eq(id)))
                .set((
                    filter_rules::pending_result_id.eq(None::<i32>),
                    filter_rules::target_type.eq(target_type),
                    filter_rules::target_id.eq(req.target_id),
                    filter_rules::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        _ => {}
    }

    // 更新 pending status
    diesel::update(pending_ai_results::table.find(id))
        .set((
            pending_ai_results::status.eq("confirmed"),
            pending_ai_results::expires_at.eq(Some(expires_at)),
            pending_ai_results::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// POST /pending-ai-results/:id/reject
pub async fn reject_pending(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state.db.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = Utc::now().naive_utc();
    let expires_at = now + chrono::Duration::days(7);

    let pending = pending_ai_results::table
        .find(id)
        .first::<PendingAiResult>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Not found".to_string()))?;

    match pending.result_type.as_str() {
        "parser" => {
            diesel::delete(title_parsers::table
                .filter(title_parsers::pending_result_id.eq(id)))
                .execute(&mut conn)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        "filter" => {
            diesel::delete(filter_rules::table
                .filter(filter_rules::pending_result_id.eq(id)))
                .execute(&mut conn)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        _ => {}
    }

    diesel::update(pending_ai_results::table.find(id))
        .set((
            pending_ai_results::status.eq("failed"),
            pending_ai_results::expires_at.eq(Some(expires_at)),
            pending_ai_results::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct RegenerateRequest {
    pub custom_prompt: Option<String>,
}

// POST /pending-ai-results/:id/regenerate
pub async fn regenerate_pending(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<RegenerateRequest>,
) -> Result<Json<PendingAiResult>, (StatusCode, String)> {
    let pool = Arc::new(state.db.clone());

    let (result_type, source_title) = {
        let mut conn = pool.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let p = pending_ai_results::table
            .find(id)
            .first::<PendingAiResult>(&mut conn)
            .map_err(|_| (StatusCode::NOT_FOUND, "Not found".to_string()))?;

        // 先刪除舊的 parser/filter（如果有的話）
        match p.result_type.as_str() {
            "parser" => { diesel::delete(title_parsers::table.filter(title_parsers::pending_result_id.eq(id))).execute(&mut conn).ok(); }
            "filter" => { diesel::delete(filter_rules::table.filter(filter_rules::pending_result_id.eq(id))).execute(&mut conn).ok(); }
            _ => {}
        }

        // 重設為 generating
        diesel::update(pending_ai_results::table.find(id))
            .set((
                pending_ai_results::status.eq("generating"),
                pending_ai_results::error_message.eq(None::<String>),
                pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        (p.result_type, p.source_title)
    };

    // 非同步重新生成，但同步等待結果
    let result = match result_type.as_str() {
        "parser" => {
            crate::ai::parser_generator::generate_parser_for_title(
                pool,
                source_title,
                None,
                req.custom_prompt,
            )
            .await
        }
        "filter" => {
            crate::ai::filter_generator::generate_filter_for_conflict(
                pool,
                vec![source_title.clone()],
                source_title,
                req.custom_prompt,
            )
            .await
        }
        _ => Err("未知的 result_type".to_string()),
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(result))
}

/// 重新解析所有未匹配的 raw_items，對第一個仍然失敗的觸發新一輪 AI 生成
async fn rerun_unmatched_raw_items(pool: Arc<DbPool>) -> Result<(), String> {
    use crate::schema::raw_anime_items;
    use crate::models::RawAnimeItem;
    use crate::services::TitleParserService;

    let items: Vec<RawAnimeItem> = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        raw_anime_items::table
            .filter(raw_anime_items::parse_status.eq("no_match")
                .or(raw_anime_items::parse_status.eq("failed")))
            .load::<RawAnimeItem>(&mut conn)
            .map_err(|e| e.to_string())?
    };

    for item in &items {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        match TitleParserService::parse_title(&mut conn, &item.title) {
            Ok(Some(_)) => {
                // 解析成功，更新狀態
                TitleParserService::update_raw_item_status(
                    &mut conn,
                    item.item_id,
                    crate::services::ParseStatus::Parsed,
                    None,
                ).ok();
            }
            Ok(None) => {
                // 仍然失敗，觸發 AI（只觸發第一筆，避免大量觸發）
                let already_pending: bool = {
                    pending_ai_results::table
                        .filter(pending_ai_results::result_type.eq("parser"))
                        .filter(pending_ai_results::source_title.eq(&item.title))
                        .filter(pending_ai_results::status.eq_any(vec!["generating", "pending"]))
                        .count()
                        .get_result::<i64>(&mut conn)
                        .unwrap_or(0) > 0
                };

                if !already_pending {
                    let pool_clone = pool.clone();
                    let title = item.title.clone();
                    let item_id = item.item_id;
                    tokio::spawn(async move {
                        if let Err(e) = crate::ai::parser_generator::generate_parser_for_title(
                            pool_clone, title, Some(item_id), None,
                        ).await {
                            tracing::warn!("AI parser 生成觸發失敗: {}", e);
                        }
                    });
                    break; // 只觸發第一筆
                }
            }
            Err(_) => {}
        }
    }
    Ok(())
}
```

**Step 3: 在 `handlers/mod.rs` 加入新模組**

```rust
pub mod ai_settings;
pub mod pending_ai_results;
```

**Step 4: 在 `main.rs` 註冊新路由**

在現有路由之後、`.with_state(app_state)` 之前加入：

```rust
// AI 設定
.route("/ai-settings", get(handlers::ai_settings::get_ai_settings).put(handlers::ai_settings::update_ai_settings))
.route("/ai-settings/test", post(handlers::ai_settings::test_ai_connection))
// AI Prompt 設定
.route("/ai-prompt-settings", get(handlers::ai_settings::get_ai_prompt_settings).put(handlers::ai_settings::update_ai_prompt_settings))
.route("/ai-prompt-settings/revert-parser", post(handlers::ai_settings::revert_parser_prompt))
.route("/ai-prompt-settings/revert-filter", post(handlers::ai_settings::revert_filter_prompt))
// 待確認管理
.route("/pending-ai-results", get(handlers::pending_ai_results::list_pending))
.route("/pending-ai-results/:id", get(handlers::pending_ai_results::get_pending).put(handlers::pending_ai_results::update_pending))
.route("/pending-ai-results/:id/confirm", post(handlers::pending_ai_results::confirm_pending))
.route("/pending-ai-results/:id/reject", post(handlers::pending_ai_results::reject_pending))
.route("/pending-ai-results/:id/regenerate", post(handlers::pending_ai_results::regenerate_pending))
```

**Step 5: 編譯確認**

```bash
cd core-service && cargo check 2>&1 | head -50
```

**Step 6: Commit**

```bash
git add core-service/src/handlers/ai_settings.rs
git add core-service/src/handlers/pending_ai_results.rs
git add core-service/src/handlers/mod.rs
git add core-service/src/main.rs
git commit -m "feat(handlers): add AI settings and pending AI results endpoints"
```

---

## Phase 5: 服務層整合

### Task 7: 修改 title_parser.rs 觸發 AI 生成

**Files:**
- Modify: `core-service/src/services/title_parser.rs`

**Step 1: 修改 `parse_title` 使其接受 pool 參數並在失敗時觸發 AI**

在 `TitleParserService` 加入：

```rust
/// 解析標題，失敗時觸發 AI 生成（非同步背景）
pub fn parse_title_with_ai_fallback(
    conn: &mut PgConnection,
    pool: std::sync::Arc<crate::db::DbPool>,
    title: &str,
    raw_item_id: Option<i32>,
) -> Result<Option<ParsedResult>, String> {
    let result = Self::parse_title(conn, title)?;

    if result.is_none() {
        // 檢查是否已有 pending 生成中，避免重複觸發
        use crate::schema::pending_ai_results;
        let already_pending: bool = pending_ai_results::table
            .filter(pending_ai_results::result_type.eq("parser"))
            .filter(pending_ai_results::source_title.eq(title))
            .filter(pending_ai_results::status.eq_any(vec!["generating", "pending"]))
            .count()
            .get_result::<i64>(conn)
            .unwrap_or(0) > 0;

        if !already_pending {
            let pool_clone = pool.clone();
            let title_owned = title.to_string();
            tokio::spawn(async move {
                if let Err(e) = crate::ai::parser_generator::generate_parser_for_title(
                    pool_clone,
                    title_owned,
                    raw_item_id,
                    None,
                ).await {
                    tracing::warn!("AI parser 觸發失敗: {}", e);
                }
            });
        }
    }

    Ok(result)
}
```

找到 `save_raw_item` 或 `process_raw_item` 的呼叫點，將 `parse_title` 改為 `parse_title_with_ai_fallback`（需要傳入 pool）。

**Step 2: 確認編譯與邏輯**

```bash
cd core-service && cargo check 2>&1 | head -30
```

**Step 3: Commit**

```bash
git add core-service/src/services/title_parser.rs
git commit -m "feat(services): trigger AI parser generation on parse failure"
```

---

### Task 8: 修改 conflict_detection.rs 觸發 AI Filter 生成

**Files:**
- Modify: `core-service/src/services/conflict_detection.rs`

**Step 1: 在 `detect_and_mark_conflicts` 偵測到新 conflict 時觸發 AI**

在方法末尾，當 `conflicts_found > 0` 時加入：

```rust
// 觸發 AI filter 生成
if result.conflicts_found > 0 {
    // 取得衝突的動畫標題
    let conflict_titles = /* 從 DB 查詢衝突的 anime link 標題 */;
    let source_title = conflict_titles.first().cloned().unwrap_or_default();
    let pool_clone = self.pool.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::ai::filter_generator::generate_filter_for_conflict(
            pool_clone,
            conflict_titles,
            source_title,
            None,
        ).await {
            tracing::warn!("AI filter 觸發失敗: {}", e);
        }
    });
}
```

注意：`ConflictDetectionService` 需要持有 `Arc<DbPool>` 引用。若目前沒有，需在 `new()` 中加入 pool 參數。

**Step 2: Commit**

```bash
git add core-service/src/services/conflict_detection.rs
git commit -m "feat(services): trigger AI filter generation on conflict detection"
```

---

### Task 9: 排程清除過期 pending_ai_results

**Files:**
- Modify: `core-service/src/services/scheduler.rs`

**Step 1: 在 `FetchScheduler` 的 `start()` 迴圈中加入清除邏輯**

或新增 `CleanupScheduler`，在 `main.rs` 以每小時間隔啟動：

```rust
pub struct CleanupScheduler {
    db_pool: DbPool,
}

impl CleanupScheduler {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub async fn start(self: std::sync::Arc<Self>) {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3600)); // 每小時
        loop {
            ticker.tick().await;
            let mut conn = match self.db_pool.get() {
                Ok(c) => c,
                Err(e) => { tracing::warn!("CleanupScheduler DB 連接失敗: {}", e); continue; }
            };
            use crate::schema::pending_ai_results;
            use diesel::prelude::*;
            match diesel::delete(pending_ai_results::table
                .filter(pending_ai_results::expires_at.lt(chrono::Utc::now().naive_utc())))
                .execute(&mut conn)
            {
                Ok(n) => if n > 0 { tracing::info!("清除 {} 筆過期 pending_ai_results", n); },
                Err(e) => tracing::warn!("清除過期記錄失敗: {}", e),
            }
        }
    }
}
```

在 `main.rs` 啟動：
```rust
let cleanup_scheduler = std::sync::Arc::new(services::CleanupScheduler::new(app_state.db.clone()));
let cs_clone = cleanup_scheduler.clone();
tokio::spawn(async move { cs_clone.start().await; });
```

**Step 2: Commit**

```bash
git add core-service/src/services/scheduler.rs core-service/src/main.rs
git commit -m "feat(scheduler): add cleanup scheduler for expired pending_ai_results"
```

---

## Phase 6: 前端

### Task 10: 移除 ConflictsPage，更新路由與導覽

**Files:**
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/components/layout/Sidebar.tsx`（或 Sidebar 相關組件）
- Delete: `frontend/src/pages/conflicts/ConflictsPage.tsx`（或保留但不再路由）

**Step 1: 更新 `App.tsx`**

移除 `ConflictsPage` import，移除 `/conflicts` 路由，加入新路由：

```tsx
import PendingPage from "@/pages/pending/PendingPage"
import SettingsPage from "@/pages/settings/SettingsPage"

// 路由修改
// 移除: <Route path="conflicts" element={<ConflictsPage />} />
// 新增:
<Route path="pending" element={<PendingPage />} />
<Route path="settings" element={<SettingsPage />} />
```

**Step 2: 更新 Sidebar 導覽**

找到衝突頁面的導覽項目，改為「待確認」並指向 `/pending`；新增「設定」項目指向 `/settings`。

**Step 3: Commit**

```bash
git add frontend/src/App.tsx frontend/src/components/layout/
git commit -m "feat(frontend): replace conflicts route with pending, add settings route"
```

---

### Task 11: 新增 API 類型與 CoreApi 方法

**Files:**
- Create: `frontend/src/schemas/ai.ts`
- Modify: `frontend/src/services/CoreApi.ts`
- Modify: `frontend/src/services/CoreApiLive.ts`（如存在）

**Step 1: 建立 `src/schemas/ai.ts`**

```typescript
export interface AiSettings {
  id: number
  base_url: string
  api_key: string  // 已遮罩
  model_name: string
  created_at: string
  updated_at: string
}

export interface AiPromptSettings {
  id: number
  fixed_parser_prompt: string | null
  fixed_filter_prompt: string | null
  custom_parser_prompt: string | null
  custom_filter_prompt: string | null
  created_at: string
  updated_at: string
}

export interface PendingAiResult {
  id: number
  result_type: 'parser' | 'filter'
  source_title: string
  generated_data: Record<string, unknown> | null
  status: 'generating' | 'pending' | 'confirmed' | 'failed'
  error_message: string | null
  raw_item_id: number | null
  used_fixed_prompt: string
  used_custom_prompt: string | null
  expires_at: string | null
  created_at: string
  updated_at: string
}

export interface ConfirmPendingRequest {
  level: 'global' | 'subscription' | 'anime_work'
  target_id?: number
}

export interface RegenerateRequest {
  custom_prompt?: string
}
```

**Step 2: 在 `CoreApi.ts` 加入新方法**

```typescript
// AI 設定
readonly getAiSettings: Effect.Effect<AiSettings>
readonly updateAiSettings: (req: Partial<Pick<AiSettings, 'base_url' | 'api_key' | 'model_name'>>) => Effect.Effect<void>
readonly testAiConnection: Effect.Effect<{ ok: boolean; error?: string }>
readonly getAiPromptSettings: Effect.Effect<AiPromptSettings>
readonly updateAiPromptSettings: (req: Partial<Omit<AiPromptSettings, 'id' | 'created_at' | 'updated_at'>>) => Effect.Effect<void>
readonly revertParserPrompt: Effect.Effect<{ value: string }>
readonly revertFilterPrompt: Effect.Effect<{ value: string }>

// 待確認管理
readonly getPendingAiResults: (params?: { result_type?: string; status?: string }) => Effect.Effect<readonly PendingAiResult[]>
readonly getPendingAiResult: (id: number) => Effect.Effect<PendingAiResult>
readonly updatePendingAiResult: (id: number, generated_data: Record<string, unknown>) => Effect.Effect<PendingAiResult>
readonly confirmPendingAiResult: (id: number, req: ConfirmPendingRequest) => Effect.Effect<void>
readonly rejectPendingAiResult: (id: number) => Effect.Effect<void>
readonly regeneratePendingAiResult: (id: number, req: RegenerateRequest) => Effect.Effect<PendingAiResult>
```

**Step 3: Commit**

```bash
git add frontend/src/schemas/ai.ts frontend/src/services/
git commit -m "feat(frontend): add AI types and CoreApi methods"
```

---

### Task 12: 共用組件 AiResultPanel

**Files:**
- Create: `frontend/src/components/shared/AiResultPanel.tsx`

**Step 1: 建立 `AiResultPanel.tsx`**

```tsx
import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import { Label } from "@/components/ui/label"
import {
  Select, SelectContent, SelectItem,
  SelectTrigger, SelectValue,
} from "@/components/ui/select"
import { Badge } from "@/components/ui/badge"
import { Loader2, RefreshCw } from "lucide-react"
import type { PendingAiResult, ConfirmPendingRequest } from "@/schemas/ai"

interface AiResultPanelProps {
  result: PendingAiResult
  onConfirmed?: () => void
  onRejected?: () => void
  onRegenerated?: (updated: PendingAiResult) => void
  /** 各類型各自獨立的編輯器 */
  children?: React.ReactNode
  /** 預覽比較區（各自提供） */
  previewSlot?: React.ReactNode
}

export function AiResultPanel({
  result,
  onConfirmed,
  onRejected,
  onRegenerated,
  children,
  previewSlot,
}: AiResultPanelProps) {
  const { t } = useTranslation()
  const [tempPrompt, setTempPrompt] = useState("")
  const [level, setLevel] = useState<"global" | "subscription" | "anime_work">("global")
  const [targetId, setTargetId] = useState<string>("")

  const { mutate: confirm, isLoading: confirming } = useEffectMutation(
    (req: ConfirmPendingRequest) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.confirmPendingAiResult(result.id, req)
      }),
    { onSuccess: onConfirmed }
  )

  const { mutate: reject, isLoading: rejecting } = useEffectMutation(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.rejectPendingAiResult(result.id)
      }),
    { onSuccess: onRejected }
  )

  const { mutate: regenerate, isLoading: regenerating } = useEffectMutation(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.regeneratePendingAiResult(result.id, {
          custom_prompt: tempPrompt || undefined,
        })
      }),
    {
      onSuccess: (updated) => {
        setTempPrompt("")
        onRegenerated?.(updated)
      }
    }
  )

  const statusVariant = {
    generating: "secondary",
    pending: "default",
    confirmed: "outline",
    failed: "destructive",
  }[result.status] as "secondary" | "default" | "outline" | "destructive"

  const isPending = result.status === "pending"
  const isFailed = result.status === "failed"
  const isGenerating = result.status === "generating"

  return (
    <div className="space-y-4">
      {/* 標題列 */}
      <div className="flex items-center justify-between">
        <div className="space-y-1">
          <p className="font-medium">{result.source_title}</p>
          <p className="text-xs text-muted-foreground">
            {new Date(result.created_at).toLocaleString()}
          </p>
        </div>
        <Badge variant={statusVariant}>
          {isGenerating && <Loader2 className="mr-1 size-3 animate-spin" />}
          {result.status}
        </Badge>
      </div>

      {/* 錯誤訊息 */}
      {isFailed && result.error_message && (
        <p className="text-sm text-destructive bg-destructive/10 rounded p-2">
          {result.error_message}
        </p>
      )}

      {/* 編輯器（由外部注入） */}
      {(isPending || isFailed) && children && (
        <div className="border rounded-lg p-4">{children}</div>
      )}

      {/* 預覽比較（由外部注入） */}
      {isPending && previewSlot && (
        <div>{previewSlot}</div>
      )}

      {/* 臨時自訂 Prompt */}
      <div className="space-y-2">
        <Label className="text-sm">臨時自訂 Prompt（僅影響本次重新生成）</Label>
        <Textarea
          value={tempPrompt}
          onChange={e => setTempPrompt(e.target.value)}
          placeholder={result.used_custom_prompt ?? "留空使用全局設定"}
          rows={3}
          className="text-sm font-mono"
        />
        <Button
          variant="outline"
          size="sm"
          onClick={() => regenerate(undefined)}
          disabled={regenerating || isGenerating}
        >
          {regenerating ? <Loader2 className="mr-1 size-3 animate-spin" /> : <RefreshCw className="mr-1 size-3" />}
          重新生成
        </Button>
      </div>

      {/* 套用層級 + 確認/拒絕 */}
      {isPending && (
        <div className="flex items-center gap-3 pt-2 border-t">
          <div className="flex items-center gap-2 flex-1">
            <Label className="text-sm whitespace-nowrap">套用層級</Label>
            <Select value={level} onValueChange={v => setLevel(v as typeof level)}>
              <SelectTrigger className="w-36 h-8">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="global">全局</SelectItem>
                <SelectItem value="subscription">訂閱</SelectItem>
                <SelectItem value="anime_work">動畫作品</SelectItem>
              </SelectContent>
            </Select>
            {level !== "global" && (
              <input
                type="number"
                placeholder="目標 ID"
                value={targetId}
                onChange={e => setTargetId(e.target.value)}
                className="h-8 w-24 rounded border px-2 text-sm"
              />
            )}
          </div>
          <Button variant="outline" size="sm" onClick={() => reject(undefined)} disabled={rejecting}>
            拒絕
          </Button>
          <Button
            size="sm"
            onClick={() => confirm({
              level,
              target_id: targetId ? parseInt(targetId) : undefined,
            })}
            disabled={confirming}
          >
            {confirming && <Loader2 className="mr-1 size-3 animate-spin" />}
            確認套用
          </Button>
        </div>
      )}
    </div>
  )
}
```

**Step 2: Commit**

```bash
git add frontend/src/components/shared/AiResultPanel.tsx
git commit -m "feat(frontend): add shared AiResultPanel component"
```

---

### Task 13: 待確認頁面（PendingPage）

**Files:**
- Create: `frontend/src/pages/pending/PendingPage.tsx`

**Step 1: 建立 `PendingPage.tsx`**

```tsx
import { useState } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { PageHeader } from "@/components/shared/PageHeader"
import { AiResultPanel } from "@/components/shared/AiResultPanel"
import { ParserForm } from "@/components/shared/ParserForm"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Loader2 } from "lucide-react"
import type { PendingAiResult } from "@/schemas/ai"

export default function PendingPage() {
  const [activeTab, setActiveTab] = useState("all")
  const [expandedId, setExpandedId] = useState<number | null>(null)

  const { data: results, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getPendingAiResults()
      }),
    []
  )

  const filtered = (results ?? []).filter(r => {
    if (activeTab === "parser") return r.result_type === "parser"
    if (activeTab === "filter") return r.result_type === "filter"
    return true
  })

  const handleDone = () => {
    setExpandedId(null)
    refetch()
  }

  return (
    <div className="space-y-6">
      <PageHeader title="待確認" description="AI 生成的解析器與過濾規則，確認後套用" />

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList variant="line">
          <TabsTrigger value="all">全部 {results ? `(${results.length})` : ""}</TabsTrigger>
          <TabsTrigger value="parser">
            Parser {results ? `(${results.filter(r => r.result_type === "parser").length})` : ""}
          </TabsTrigger>
          <TabsTrigger value="filter">
            Filter {results ? `(${results.filter(r => r.result_type === "filter").length})` : ""}
          </TabsTrigger>
        </TabsList>

        <TabsContent value={activeTab} className="mt-4">
          {isLoading ? (
            <div className="flex justify-center py-8">
              <Loader2 className="size-6 animate-spin text-muted-foreground" />
            </div>
          ) : filtered.length === 0 ? (
            <p className="text-center text-muted-foreground py-8">目前沒有待確認的項目</p>
          ) : (
            <div className="space-y-3">
              {filtered.map(result => (
                <PendingResultRow
                  key={result.id}
                  result={result}
                  expanded={expandedId === result.id}
                  onToggle={() => setExpandedId(expandedId === result.id ? null : result.id)}
                  onDone={handleDone}
                />
              ))}
            </div>
          )}
        </TabsContent>
      </Tabs>
    </div>
  )
}

function PendingResultRow({
  result,
  expanded,
  onToggle,
  onDone,
}: {
  result: PendingAiResult
  expanded: boolean
  onToggle: () => void
  onDone: () => void
}) {
  const [localResult, setLocalResult] = useState(result)

  return (
    <div className="border rounded-lg overflow-hidden">
      {/* 列表行 */}
      <button
        className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-muted/50 transition-colors"
        onClick={onToggle}
      >
        <span className="text-xs px-2 py-0.5 rounded bg-muted font-mono uppercase">
          {result.result_type}
        </span>
        <span className="flex-1 text-sm">{result.source_title}</span>
        <span className="text-xs text-muted-foreground">
          {new Date(result.created_at).toLocaleDateString()}
        </span>
        <StatusDot status={result.status} />
      </button>

      {/* 展開內容 */}
      {expanded && (
        <div className="border-t px-4 py-4 bg-muted/20">
          <AiResultPanel
            result={localResult}
            onConfirmed={onDone}
            onRejected={onDone}
            onRegenerated={updated => setLocalResult(updated)}
          >
            {result.result_type === "parser" && localResult.generated_data && (
              <ParserForm
                value={localResult.generated_data as any}
                onChange={data => setLocalResult(prev => ({ ...prev, generated_data: data }))}
              />
            )}
            {result.result_type === "filter" && localResult.generated_data && (
              <FilterRuleEditor
                value={localResult.generated_data as any}
                onChange={data => setLocalResult(prev => ({ ...prev, generated_data: data }))}
              />
            )}
          </AiResultPanel>
        </div>
      )}
    </div>
  )
}

function StatusDot({ status }: { status: string }) {
  const colors = {
    generating: "bg-yellow-400 animate-pulse",
    pending: "bg-blue-500",
    confirmed: "bg-green-500",
    failed: "bg-red-500",
  }
  return (
    <span className={`size-2 rounded-full ${colors[status as keyof typeof colors] ?? "bg-gray-400"}`} />
  )
}
```

**Step 2: Commit**

```bash
git add frontend/src/pages/pending/
git commit -m "feat(frontend): add PendingPage for AI-generated parser/filter confirmation"
```

---

### Task 14: 設定頁面（SettingsPage）

**Files:**
- Create: `frontend/src/pages/settings/SettingsPage.tsx`

**Step 1: 建立 `SettingsPage.tsx`**

```tsx
import { useState, useEffect } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { PageHeader } from "@/components/shared/PageHeader"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Separator } from "@/components/ui/separator"
import { Loader2, RotateCcw } from "lucide-react"

export default function SettingsPage() {
  return (
    <div className="space-y-6 max-w-2xl">
      <PageHeader title="設定" description="系統設定與 AI 整合設定" />
      <AiConnectionSection />
      <Separator />
      <ParserPromptSection />
      <Separator />
      <FilterPromptSection />
    </div>
  )
}

function AiConnectionSection() {
  const { data: settings } = useEffectQuery(
    () => Effect.gen(function* () { const api = yield* CoreApi; return yield* api.getAiSettings }),
    []
  )

  const [baseUrl, setBaseUrl] = useState("")
  const [apiKey, setApiKey] = useState("")
  const [modelName, setModelName] = useState("")
  const [testResult, setTestResult] = useState<{ ok: boolean; error?: string } | null>(null)

  useEffect(() => {
    if (settings) {
      setBaseUrl(settings.base_url)
      setModelName(settings.model_name)
    }
  }, [settings])

  const { mutate: save, isLoading: saving } = useEffectMutation(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.updateAiSettings({
          base_url: baseUrl,
          api_key: apiKey || undefined,
          model_name: modelName,
        })
      })
  )

  const { mutate: test, isLoading: testing } = useEffectMutation(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.testAiConnection
      }),
    { onSuccess: setTestResult }
  )

  return (
    <Card>
      <CardHeader>
        <CardTitle>AI 連線設定</CardTitle>
        <CardDescription>設定 OpenAI-compatible API 連線資訊</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <Label>Base URL</Label>
          <Input value={baseUrl} onChange={e => setBaseUrl(e.target.value)}
            placeholder="https://api.openai.com/v1" />
        </div>
        <div className="space-y-2">
          <Label>API Key</Label>
          <Input type="password" value={apiKey} onChange={e => setApiKey(e.target.value)}
            placeholder="輸入新 API Key（留空保持不變）" />
        </div>
        <div className="space-y-2">
          <Label>Model Name</Label>
          <Input value={modelName} onChange={e => setModelName(e.target.value)}
            placeholder="gpt-4o-mini" />
        </div>
        {testResult && (
          <p className={`text-sm ${testResult.ok ? "text-green-600" : "text-destructive"}`}>
            {testResult.ok ? "連線成功" : `連線失敗: ${testResult.error}`}
          </p>
        )}
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => test(undefined)} disabled={testing}>
            {testing && <Loader2 className="mr-1 size-3 animate-spin" />}
            測試連線
          </Button>
          <Button size="sm" onClick={() => save(undefined)} disabled={saving}>
            {saving && <Loader2 className="mr-1 size-3 animate-spin" />}
            儲存
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

function ParserPromptSection() {
  const { data: settings, refetch } = useEffectQuery(
    () => Effect.gen(function* () { const api = yield* CoreApi; return yield* api.getAiPromptSettings }),
    []
  )
  const [fixed, setFixed] = useState("")
  const [custom, setCustom] = useState("")

  useEffect(() => {
    if (settings) {
      setFixed(settings.fixed_parser_prompt ?? "")
      setCustom(settings.custom_parser_prompt ?? "")
    }
  }, [settings])

  const { mutate: save, isLoading: saving } = useEffectMutation(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.updateAiPromptSettings({ fixed_parser_prompt: fixed, custom_parser_prompt: custom })
      })
  )

  const { mutate: revert, isLoading: reverting } = useEffectMutation(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.revertParserPrompt
      }),
    { onSuccess: (r) => { setFixed(r.value); refetch() } }
  )

  return (
    <Card>
      <CardHeader>
        <CardTitle>Parser Prompt 設定</CardTitle>
        <CardDescription>AI 生成解析器時使用的 Prompt</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label>固定 Prompt</Label>
            <Button variant="ghost" size="sm" onClick={() => revert(undefined)} disabled={reverting}>
              <RotateCcw className="mr-1 size-3" /> Revert 預設值
            </Button>
          </div>
          <Textarea value={fixed} onChange={e => setFixed(e.target.value)}
            rows={6} className="font-mono text-sm" placeholder="留空則不使用固定 Prompt" />
        </div>
        <div className="space-y-2">
          <Label>自訂 Prompt（追加在固定 Prompt 之後）</Label>
          <Textarea value={custom} onChange={e => setCustom(e.target.value)}
            rows={3} className="font-mono text-sm" placeholder="留空" />
        </div>
        <Button size="sm" onClick={() => save(undefined)} disabled={saving}>
          {saving && <Loader2 className="mr-1 size-3 animate-spin" />}
          儲存
        </Button>
      </CardContent>
    </Card>
  )
}

function FilterPromptSection() {
  // 同 ParserPromptSection，欄位改為 fixed_filter_prompt / custom_filter_prompt
  // 略（結構完全一致，只改 key 名稱和 revert 呼叫）
  return (
    <Card>
      <CardHeader>
        <CardTitle>Filter Prompt 設定</CardTitle>
        <CardDescription>AI 生成過濾規則時使用的 Prompt</CardDescription>
      </CardHeader>
      <CardContent>
        {/* 同 ParserPromptSection，欄位改 filter */}
      </CardContent>
    </Card>
  )
}
```

**Step 2: Commit**

```bash
git add frontend/src/pages/settings/
git commit -m "feat(frontend): add SettingsPage with AI connection and prompt configuration"
```

---

### Task 15: 訂閱建立 Wizard（三步驟）

**Files:**
- Create: `frontend/src/pages/subscriptions/CreateSubscriptionWizard.tsx`
- Modify: `frontend/src/pages/subscriptions/SubscriptionsPage.tsx`

**Step 1: 建立 `CreateSubscriptionWizard.tsx`**

三步驟 Wizard，步驟狀態以 `step: 1 | 2 | 3` 控制：

```tsx
import { useState } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { AiResultPanel } from "@/components/shared/AiResultPanel"
import { ParserForm } from "@/components/shared/ParserForm"
import { Loader2, CheckCircle2 } from "lucide-react"
import type { PendingAiResult } from "@/schemas/ai"

interface CreateSubscriptionWizardProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onCreated?: () => void
}

type WizardStep = 1 | 2 | 3

export function CreateSubscriptionWizard({
  open, onOpenChange, onCreated,
}: CreateSubscriptionWizardProps) {
  const [step, setStep] = useState<WizardStep>(1)
  const [subscriptionId, setSubscriptionId] = useState<number | null>(null)
  const [pendingParser, setPendingParser] = useState<PendingAiResult | null>(null)
  const [pendingFilter, setPendingFilter] = useState<PendingAiResult | null>(null)
  const [parseSuccess, setParseSuccess] = useState(false)
  const [filterSuccess, setFilterSuccess] = useState(false)

  // Step 1 表單
  const [url, setUrl] = useState("")
  const [name, setName] = useState("")
  const [interval, setInterval] = useState("30")

  const reset = () => {
    setStep(1)
    setSubscriptionId(null)
    setPendingParser(null)
    setPendingFilter(null)
    setParseSuccess(false)
    setFilterSuccess(false)
    setUrl("")
    setName("")
    setInterval("30")
  }

  // Step 1: 建立訂閱並進入 Step 2
  const { mutate: createAndFetch, isLoading: creating } = useEffectMutation(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        // 1. 建立訂閱
        const sub = yield* api.createSubscription({
          source_url: url,
          name: name || undefined,
          fetch_interval_minutes: parseInt(interval),
        })
        // 2. 立即觸發抓取並解析（後端返回解析結果或 pending_ai_result）
        const parseResult = yield* api.fetchAndParseSubscription(sub.subscription_id)
        return { sub, parseResult }
      }),
    {
      onSuccess: ({ sub, parseResult }) => {
        setSubscriptionId(sub.subscription_id)
        if (parseResult.parse_failed && parseResult.pending_ai_result) {
          setPendingParser(parseResult.pending_ai_result)
          setParseSuccess(false)
        } else {
          setParseSuccess(true)
        }
        setStep(2)
      }
    }
  )

  // Step 2: 確認 parser 後進入 Step 3
  const handleParserConfirmed = async () => {
    // parser 已在 AiResultPanel 的 confirm 中處理
    // 繼續進入 Step 3，觸發 filter 檢查
    if (subscriptionId) {
      // 後端 endpoint：對該訂閱執行 filter + conflict 檢查
      setStep(3)
      // TODO: 呼叫 checkSubscriptionConflicts
    }
  }

  const stepTitles: Record<WizardStep, string> = {
    1: "基本設定",
    2: "解析驗證",
    3: "Filter 驗證",
  }

  return (
    <Dialog open={open} onOpenChange={v => { if (!v) reset(); onOpenChange(v) }}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>新增訂閱 — {stepTitles[step]}</DialogTitle>
        </DialogHeader>

        {/* Step 指示器 */}
        <div className="flex gap-2 mb-4">
          {([1, 2, 3] as WizardStep[]).map(s => (
            <div key={s} className={`flex-1 h-1 rounded-full ${s <= step ? "bg-primary" : "bg-muted"}`} />
          ))}
        </div>

        {/* Step 1 */}
        {step === 1 && (
          <div className="space-y-4">
            <div className="space-y-2">
              <Label>RSS URL *</Label>
              <Input value={url} onChange={e => setUrl(e.target.value)} placeholder="https://..." />
            </div>
            <div className="space-y-2">
              <Label>名稱</Label>
              <Input value={name} onChange={e => setName(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label>抓取間隔（分鐘）</Label>
              <Input type="number" value={interval} onChange={e => setInterval(e.target.value)} />
            </div>
            <DialogFooter>
              <Button onClick={() => createAndFetch(undefined)} disabled={!url || creating}>
                {creating && <Loader2 className="mr-1 size-4 animate-spin" />}
                下一步
              </Button>
            </DialogFooter>
          </div>
        )}

        {/* Step 2 */}
        {step === 2 && (
          <div className="space-y-4">
            {parseSuccess ? (
              <div className="flex items-center gap-2 text-green-600">
                <CheckCircle2 className="size-4" />
                <span className="text-sm">解析成功，可以繼續</span>
              </div>
            ) : pendingParser ? (
              <AiResultPanel
                result={pendingParser}
                onConfirmed={handleParserConfirmed}
                onRejected={() => setStep(3)}  // 跳過直接到下一步
                onRegenerated={setPendingParser}
              >
                <ParserForm value={pendingParser.generated_data as any} onChange={() => {}} />
              </AiResultPanel>
            ) : (
              <div className="flex justify-center py-4">
                <Loader2 className="size-6 animate-spin text-muted-foreground" />
              </div>
            )}
            {parseSuccess && (
              <DialogFooter>
                <Button onClick={() => setStep(3)}>下一步</Button>
              </DialogFooter>
            )}
          </div>
        )}

        {/* Step 3 */}
        {step === 3 && (
          <div className="space-y-4">
            {filterSuccess ? (
              <div className="flex items-center gap-2 text-green-600">
                <CheckCircle2 className="size-4" />
                <span className="text-sm">無衝突，訂閱建立完成</span>
              </div>
            ) : pendingFilter ? (
              <AiResultPanel
                result={pendingFilter}
                onConfirmed={() => { setFilterSuccess(true) }}
                onRejected={() => { setFilterSuccess(true) }}
                onRegenerated={setPendingFilter}
              />
            ) : (
              <div className="flex items-center gap-2 text-green-600">
                <CheckCircle2 className="size-4" />
                <span className="text-sm">無衝突，訂閱建立完成</span>
              </div>
            )}
            {(filterSuccess || !pendingFilter) && (
              <DialogFooter>
                <Button onClick={() => { onCreated?.(); onOpenChange(false); reset() }}>
                  完成
                </Button>
              </DialogFooter>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
```

**Step 2: 在 `SubscriptionsPage.tsx` 將舊的新增 Dialog 換成 Wizard**

找到新增訂閱的 Dialog（`createOpen` state），替換為：
```tsx
import { CreateSubscriptionWizard } from "./CreateSubscriptionWizard"

// 替換 Dialog 為：
<CreateSubscriptionWizard
  open={createOpen}
  onOpenChange={setCreateOpen}
  onCreated={refetch}
/>
```

**Step 3: 在後端新增 Wizard 所需的端點**

後端需要支援：
- `POST /subscriptions/:id/fetch-and-parse` — 立即抓取並解析，返回 parse 結果或 `pending_ai_result`
- `POST /subscriptions/:id/check-conflicts` — 執行 filter + conflict 檢查，返回 conflict 結果或 `pending_ai_result`

在 `handlers/subscriptions.rs` 加入這兩個 handler，在 `main.rs` 加入路由。

**Step 4: Commit**

```bash
git add frontend/src/pages/subscriptions/
git commit -m "feat(frontend): replace subscription create dialog with 3-step wizard"
```

---

## 最終驗收

### 後端整體編譯

```bash
cd core-service && cargo build 2>&1 | tail -20
```
Expected: `Compiling core-service ... Finished`

### Migration 執行

```bash
cd core-service && diesel migration run
```

### 前端整體建置

```bash
cd frontend && npm run build 2>&1 | tail -20
```
Expected: `✓ built in ...`

### 功能驗收清單

- [ ] `GET /ai-settings` 返回遮罩後的設定
- [ ] `PUT /ai-settings` 儲存連線設定
- [ ] `GET /ai-prompt-settings` 返回 prompt 設定
- [ ] `POST /ai-prompt-settings/revert-parser` 回復預設值
- [ ] 新 raw_item 解析失敗時，`pending_ai_results` 自動建立記錄
- [ ] conflict 被標記時，`pending_ai_results` 自動建立 filter 記錄
- [ ] `POST /pending-ai-results/:id/confirm` 正確清除 `pending_result_id` 並設置 `expires_at`
- [ ] `POST /pending-ai-results/:id/reject` 正確刪除關聯 parser/filter
- [ ] 前端 `/pending` 頁面顯示待確認列表
- [ ] 前端 `/settings` 頁面可儲存 AI 連線和 prompt 設定
- [ ] 訂閱新增 Wizard 三步驟正常運作
- [ ] 舊 `/conflicts` 路由已移除
- [ ] Catch-All parser 不再存在
- [ ] 排程器每小時清除過期 `pending_ai_results`

### 最終 Commit

```bash
git add -A
git commit -m "feat: complete AI automation for parser/filter generation"
```
