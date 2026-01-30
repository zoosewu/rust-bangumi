# Fetcher 架構重構實作計畫

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 將解析職責從 Fetcher 移至 Core Service，Fetcher 只負責抓取原始資料，Core 使用可配置的解析器解析標題。

**Architecture:** Fetcher 抓取 RSS 後直接回傳原始資料（title, description, download_url, pub_date）到 Core。Core 儲存到 raw_anime_items 表，然後使用 title_parsers 表中的解析器按優先權嘗試解析，成功則建立 anime_links 記錄。

**Tech Stack:** Rust, Diesel ORM, PostgreSQL (with ENUM types), Axum

**規格文件:** `docs/plans/2026-01-30-fetcher-architecture-refactor-spec.md`

---

## Task 1: 建立資料庫 Migration

**Files:**
- Create: `core-service/migrations/2026-01-30-000001-raw-anime-items/up.sql`
- Create: `core-service/migrations/2026-01-30-000001-raw-anime-items/down.sql`

**Step 1: 建立 migration 目錄**

Run: `mkdir -p /workspace/core-service/migrations/2026-01-30-000001-raw-anime-items`
Expected: 目錄建立成功

**Step 2: 撰寫 up.sql**

```sql
-- 建立 parser_source_type ENUM
CREATE TYPE parser_source_type AS ENUM ('regex', 'static');

-- 建立 title_parsers 表
CREATE TABLE title_parsers (
    parser_id               SERIAL PRIMARY KEY,
    name                    VARCHAR(100) NOT NULL,
    description             TEXT,
    priority                INT NOT NULL DEFAULT 0,
    is_enabled              BOOLEAN NOT NULL DEFAULT TRUE,
    condition_regex         TEXT NOT NULL,
    parse_regex             TEXT NOT NULL,
    anime_title_source      parser_source_type NOT NULL,
    anime_title_value       VARCHAR(255) NOT NULL,
    episode_no_source       parser_source_type NOT NULL,
    episode_no_value        VARCHAR(50) NOT NULL,
    series_no_source        parser_source_type,
    series_no_value         VARCHAR(50),
    subtitle_group_source   parser_source_type,
    subtitle_group_value    VARCHAR(255),
    resolution_source       parser_source_type,
    resolution_value        VARCHAR(50),
    season_source           parser_source_type,
    season_value            VARCHAR(20),
    year_source             parser_source_type,
    year_value              VARCHAR(10),
    created_at              TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_title_parsers_priority
ON title_parsers(priority DESC)
WHERE is_enabled = TRUE;

-- 建立 raw_anime_items 表
CREATE TABLE raw_anime_items (
    item_id             SERIAL PRIMARY KEY,
    title               TEXT NOT NULL,
    description         TEXT,
    download_url        VARCHAR(2048) NOT NULL,
    pub_date            TIMESTAMP,
    subscription_id     INT NOT NULL REFERENCES subscriptions(subscription_id),
    status              VARCHAR(20) NOT NULL DEFAULT 'pending',
    parser_id           INT REFERENCES title_parsers(parser_id),
    error_message       TEXT,
    parsed_at           TIMESTAMP,
    created_at          TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(download_url)
);

CREATE INDEX idx_raw_items_status ON raw_anime_items(status);
CREATE INDEX idx_raw_items_subscription ON raw_anime_items(subscription_id);
CREATE INDEX idx_raw_items_created ON raw_anime_items(created_at DESC);

-- 修改 anime_links 表，新增 raw_item_id 欄位
ALTER TABLE anime_links
ADD COLUMN raw_item_id INT REFERENCES raw_anime_items(item_id);

CREATE INDEX idx_anime_links_raw_item ON anime_links(raw_item_id);

-- 插入預設解析器
INSERT INTO title_parsers (
    name, description, priority,
    condition_regex, parse_regex,
    anime_title_source, anime_title_value,
    episode_no_source, episode_no_value,
    series_no_source, series_no_value,
    subtitle_group_source, subtitle_group_value,
    resolution_source, resolution_value
) VALUES (
    'LoliHouse 標準格式',
    '匹配 [字幕組] 動畫名稱 - 集數 [解析度] 格式',
    100,
    '^\[.+\].+\s-\s\d+',
    '^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)\s*\[.*?(\d{3,4}p)',
    'regex', '2',
    'regex', '3',
    NULL, NULL,
    'regex', '1',
    'regex', '4'
), (
    '六四位元 星號格式',
    '匹配以星號分隔的格式',
    90,
    '^[^★]+★.+★\d+★',
    '^([^★]+)★(.+?)★(\d+)★(\d+x\d+)',
    'regex', '2',
    'regex', '3',
    'static', '1',
    'regex', '1',
    'regex', '4'
), (
    '預設解析器',
    '嘗試匹配任何包含 - 數字 的標題',
    1,
    '.+\s-\s\d+',
    '^(.+?)\s+-\s*(\d+)',
    'regex', '1',
    'regex', '2',
    'static', '1',
    'static', '未知字幕組',
    NULL, NULL
);
```

**Step 3: 撰寫 down.sql**

```sql
-- 移除 anime_links 的 raw_item_id
DROP INDEX IF EXISTS idx_anime_links_raw_item;
ALTER TABLE anime_links DROP COLUMN IF EXISTS raw_item_id;

-- 移除 raw_anime_items
DROP INDEX IF EXISTS idx_raw_items_created;
DROP INDEX IF EXISTS idx_raw_items_subscription;
DROP INDEX IF EXISTS idx_raw_items_status;
DROP TABLE IF EXISTS raw_anime_items;

-- 移除 title_parsers
DROP INDEX IF EXISTS idx_title_parsers_priority;
DROP TABLE IF EXISTS title_parsers;

-- 移除 ENUM
DROP TYPE IF EXISTS parser_source_type;
```

**Step 4: 執行 migration**

Run: `cd /workspace/core-service && diesel migration run`
Expected: Running migration 2026-01-30-000001-raw-anime-items

**Step 5: 驗證 schema 更新**

Run: `cd /workspace/core-service && diesel print-schema > src/schema.rs.new && diff src/schema.rs src/schema.rs.new | head -50`
Expected: 顯示新增的 title_parsers 和 raw_anime_items 表

**Step 6: 更新 schema.rs**

Run: `cd /workspace/core-service && mv src/schema.rs.new src/schema.rs`
Expected: 檔案更新成功

**Step 7: Commit**

```bash
cd /workspace/core-service
git add migrations/2026-01-30-000001-raw-anime-items/ src/schema.rs
git commit -m "feat: add raw_anime_items and title_parsers tables

- Add parser_source_type ENUM
- Add title_parsers table with priority-based parser configuration
- Add raw_anime_items table for storing fetcher raw data
- Add raw_item_id to anime_links for traceability
- Insert default parsers for common formats

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 2: 新增 Core Service ENUM 和 Model 定義

**Files:**
- Modify: `core-service/src/models/db.rs`
- Modify: `core-service/src/schema.rs` (如果需要手動調整)

**Step 1: 在 schema.rs 加入 ParserSourceType SQL type**

在 `pub mod sql_types` 區塊加入：

```rust
#[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
#[diesel(postgres_type(name = "parser_source_type"))]
pub struct ParserSourceType;
```

**Step 2: 在 db.rs 加入 ParserSourceType ENUM 定義**

在 `FilterTargetType` 定義之後加入：

```rust
// ============ ParserSourceType ENUM ============
#[derive(Debug, Clone, Copy, PartialEq, Eq, diesel::deserialize::FromSqlRow, diesel::expression::AsExpression)]
#[diesel(sql_type = crate::schema::sql_types::ParserSourceType)]
pub enum ParserSourceType {
    Regex,
    Static,
}

impl diesel::deserialize::FromSql<crate::schema::sql_types::ParserSourceType, diesel::pg::Pg> for ParserSourceType {
    fn from_sql(bytes: diesel::pg::PgValue<'_>) -> diesel::deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"regex" => Ok(ParserSourceType::Regex),
            b"static" => Ok(ParserSourceType::Static),
            _ => Err("Unrecognized parser_source_type variant".into()),
        }
    }
}

impl diesel::serialize::ToSql<crate::schema::sql_types::ParserSourceType, diesel::pg::Pg> for ParserSourceType {
    fn to_sql<'b>(&'b self, out: &mut diesel::serialize::Output<'b, '_, diesel::pg::Pg>) -> diesel::serialize::Result {
        match *self {
            ParserSourceType::Regex => out.write_all(b"regex")?,
            ParserSourceType::Static => out.write_all(b"static")?,
        }
        Ok(diesel::serialize::IsNull::No)
    }
}

impl std::fmt::Display for ParserSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserSourceType::Regex => write!(f, "regex"),
            ParserSourceType::Static => write!(f, "static"),
        }
    }
}
```

**Step 3: 在 db.rs 加入 TitleParser Model**

```rust
// ============ TitleParsers ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::title_parsers)]
pub struct TitleParser {
    pub parser_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub priority: i32,
    pub is_enabled: bool,
    pub condition_regex: String,
    pub parse_regex: String,
    pub anime_title_source: ParserSourceType,
    pub anime_title_value: String,
    pub episode_no_source: ParserSourceType,
    pub episode_no_value: String,
    pub series_no_source: Option<ParserSourceType>,
    pub series_no_value: Option<String>,
    pub subtitle_group_source: Option<ParserSourceType>,
    pub subtitle_group_value: Option<String>,
    pub resolution_source: Option<ParserSourceType>,
    pub resolution_value: Option<String>,
    pub season_source: Option<ParserSourceType>,
    pub season_value: Option<String>,
    pub year_source: Option<ParserSourceType>,
    pub year_value: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::title_parsers)]
pub struct NewTitleParser {
    pub name: String,
    pub description: Option<String>,
    pub priority: i32,
    pub is_enabled: bool,
    pub condition_regex: String,
    pub parse_regex: String,
    pub anime_title_source: ParserSourceType,
    pub anime_title_value: String,
    pub episode_no_source: ParserSourceType,
    pub episode_no_value: String,
    pub series_no_source: Option<ParserSourceType>,
    pub series_no_value: Option<String>,
    pub subtitle_group_source: Option<ParserSourceType>,
    pub subtitle_group_value: Option<String>,
    pub resolution_source: Option<ParserSourceType>,
    pub resolution_value: Option<String>,
    pub season_source: Option<ParserSourceType>,
    pub season_value: Option<String>,
    pub year_source: Option<ParserSourceType>,
    pub year_value: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
```

**Step 4: 在 db.rs 加入 RawAnimeItem Model**

```rust
// ============ RawAnimeItems ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::raw_anime_items)]
pub struct RawAnimeItem {
    pub item_id: i32,
    pub title: String,
    pub description: Option<String>,
    pub download_url: String,
    pub pub_date: Option<NaiveDateTime>,
    pub subscription_id: i32,
    pub status: String,
    pub parser_id: Option<i32>,
    pub error_message: Option<String>,
    pub parsed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::raw_anime_items)]
pub struct NewRawAnimeItem {
    pub title: String,
    pub description: Option<String>,
    pub download_url: String,
    pub pub_date: Option<NaiveDateTime>,
    pub subscription_id: i32,
    pub status: String,
    pub parser_id: Option<i32>,
    pub error_message: Option<String>,
    pub parsed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}
```

**Step 5: 更新 AnimeLink Model，新增 raw_item_id**

修改現有的 `AnimeLink` 和 `NewAnimeLink` 結構：

```rust
// ============ AnimeLinks ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::anime_links)]
pub struct AnimeLink {
    pub link_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub created_at: NaiveDateTime,
    pub raw_item_id: Option<i32>,  // 新增
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::anime_links)]
pub struct NewAnimeLink {
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub created_at: NaiveDateTime,
    pub raw_item_id: Option<i32>,  // 新增
}
```

**Step 6: 驗證編譯**

Run: `cd /workspace/core-service && cargo check`
Expected: 編譯成功，無錯誤

**Step 7: Commit**

```bash
cd /workspace/core-service
git add src/models/db.rs src/schema.rs
git commit -m "feat: add ParserSourceType enum and models for title_parsers, raw_anime_items

- Add ParserSourceType enum with Diesel serialization
- Add TitleParser and NewTitleParser models
- Add RawAnimeItem and NewRawAnimeItem models
- Add raw_item_id to AnimeLink model

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 3: 更新 Shared 模組的資料結構

**Files:**
- Modify: `shared/src/models.rs`

**Step 1: 新增 RawAnimeItem 結構**

在 `FetchedLink` 定義之後加入：

```rust
// ============ Raw Anime Item (New Architecture) ============

/// 原始動畫項目（單集）- 來自 Fetcher 的原始資料
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAnimeItem {
    pub title: String,                      // RSS <title>
    pub description: Option<String>,        // RSS <description>
    pub download_url: String,               // RSS <enclosure> url
    pub pub_date: Option<DateTime<Utc>>,    // RSS <pubDate>
}

/// Fetcher 回傳的原始結果（新架構）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFetcherResultsPayload {
    pub subscription_id: i32,
    pub items: Vec<RawAnimeItem>,
    pub fetcher_source: String,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Core 處理原始結果的回應
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFetcherResultsResponse {
    pub success: bool,
    pub items_received: usize,
    pub items_parsed: usize,
    pub items_failed: usize,
    pub message: String,
}
```

**Step 2: 驗證編譯**

Run: `cd /workspace/shared && cargo check`
Expected: 編譯成功

**Step 3: Commit**

```bash
cd /workspace/shared
git add src/models.rs
git commit -m "feat: add RawAnimeItem and RawFetcherResultsPayload for new architecture

- Add RawAnimeItem struct for raw fetcher data
- Add RawFetcherResultsPayload for new fetcher response format
- Add RawFetcherResultsResponse for core processing response

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 4: 實作標題解析服務

**Files:**
- Create: `core-service/src/services/title_parser.rs`
- Modify: `core-service/src/services/mod.rs`

**Step 1: 建立 title_parser.rs 檔案**

```rust
//! 標題解析服務
//!
//! 負責使用 title_parsers 表中的解析器解析原始標題

use diesel::prelude::*;
use regex::Regex;
use chrono::{NaiveDateTime, Utc};

use crate::models::{TitleParser, ParserSourceType, RawAnimeItem, NewRawAnimeItem};
use crate::schema::{title_parsers, raw_anime_items};

/// 解析結果
#[derive(Debug, Clone)]
pub struct ParsedResult {
    pub anime_title: String,
    pub episode_no: i32,
    pub series_no: i32,
    pub subtitle_group: Option<String>,
    pub resolution: Option<String>,
    pub season: Option<String>,
    pub year: Option<String>,
    pub parser_id: i32,
}

/// 解析狀態
#[derive(Debug, Clone, PartialEq)]
pub enum ParseStatus {
    Pending,
    Parsed,
    Partial,
    Failed,
    NoMatch,
    Skipped,
}

impl ParseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseStatus::Pending => "pending",
            ParseStatus::Parsed => "parsed",
            ParseStatus::Partial => "partial",
            ParseStatus::Failed => "failed",
            ParseStatus::NoMatch => "no_match",
            ParseStatus::Skipped => "skipped",
        }
    }
}

/// 標題解析服務
pub struct TitleParserService;

impl TitleParserService {
    /// 取得所有啟用的解析器（按 priority 降序）
    pub fn get_enabled_parsers(conn: &mut PgConnection) -> Result<Vec<TitleParser>, String> {
        title_parsers::table
            .filter(title_parsers::is_enabled.eq(true))
            .order(title_parsers::priority.desc())
            .load::<TitleParser>(conn)
            .map_err(|e| format!("Failed to load title parsers: {}", e))
    }

    /// 嘗試使用所有解析器解析標題
    pub fn parse_title(
        conn: &mut PgConnection,
        title: &str,
    ) -> Result<Option<ParsedResult>, String> {
        let parsers = Self::get_enabled_parsers(conn)?;

        for parser in parsers {
            if let Some(result) = Self::try_parser(&parser, title)? {
                return Ok(Some(result));
            }
        }

        Ok(None)
    }

    /// 嘗試使用單一解析器解析標題
    fn try_parser(parser: &TitleParser, title: &str) -> Result<Option<ParsedResult>, String> {
        // 檢查 condition_regex 是否匹配
        let condition_regex = Regex::new(&parser.condition_regex)
            .map_err(|e| format!("Invalid condition_regex for parser {}: {}", parser.parser_id, e))?;

        if !condition_regex.is_match(title) {
            return Ok(None);
        }

        // 執行 parse_regex
        let parse_regex = Regex::new(&parser.parse_regex)
            .map_err(|e| format!("Invalid parse_regex for parser {}: {}", parser.parser_id, e))?;

        let captures = match parse_regex.captures(title) {
            Some(c) => c,
            None => return Ok(None),
        };

        // 提取必要欄位
        let anime_title = Self::extract_value(&parser.anime_title_source, &parser.anime_title_value, &captures)?;
        let episode_str = Self::extract_value(&parser.episode_no_source, &parser.episode_no_value, &captures)?;
        let episode_no: i32 = episode_str.parse()
            .map_err(|_| format!("Failed to parse episode_no '{}' as integer", episode_str))?;

        // 提取 series_no（預設為 1）
        let series_no = match (&parser.series_no_source, &parser.series_no_value) {
            (Some(source), Some(value)) => {
                let s = Self::extract_value(source, value, &captures)?;
                s.parse().unwrap_or(1)
            }
            _ => 1,
        };

        // 提取非必要欄位
        let subtitle_group = Self::extract_optional_value(
            &parser.subtitle_group_source,
            &parser.subtitle_group_value,
            &captures,
        );

        let resolution = Self::extract_optional_value(
            &parser.resolution_source,
            &parser.resolution_value,
            &captures,
        );

        let season = Self::extract_optional_value(
            &parser.season_source,
            &parser.season_value,
            &captures,
        );

        let year = Self::extract_optional_value(
            &parser.year_source,
            &parser.year_value,
            &captures,
        );

        Ok(Some(ParsedResult {
            anime_title,
            episode_no,
            series_no,
            subtitle_group,
            resolution,
            season,
            year,
            parser_id: parser.parser_id,
        }))
    }

    /// 從捕獲組或靜態值提取欄位值
    fn extract_value(
        source: &ParserSourceType,
        value: &str,
        captures: &regex::Captures,
    ) -> Result<String, String> {
        match source {
            ParserSourceType::Regex => {
                let index: usize = value.parse()
                    .map_err(|_| format!("Invalid capture group index: {}", value))?;
                captures.get(index)
                    .map(|m| m.as_str().trim().to_string())
                    .ok_or_else(|| format!("Capture group {} not found", index))
            }
            ParserSourceType::Static => Ok(value.to_string()),
        }
    }

    /// 提取非必要欄位（可能為 None）
    fn extract_optional_value(
        source: &Option<ParserSourceType>,
        value: &Option<String>,
        captures: &regex::Captures,
    ) -> Option<String> {
        match (source, value) {
            (Some(s), Some(v)) => Self::extract_value(s, v, captures).ok(),
            _ => None,
        }
    }

    /// 儲存原始項目到資料庫
    pub fn save_raw_item(
        conn: &mut PgConnection,
        title: &str,
        description: Option<&str>,
        download_url: &str,
        pub_date: Option<NaiveDateTime>,
        subscription_id: i32,
    ) -> Result<RawAnimeItem, String> {
        let now = Utc::now().naive_utc();

        let new_item = NewRawAnimeItem {
            title: title.to_string(),
            description: description.map(|s| s.to_string()),
            download_url: download_url.to_string(),
            pub_date,
            subscription_id,
            status: ParseStatus::Pending.as_str().to_string(),
            parser_id: None,
            error_message: None,
            parsed_at: None,
            created_at: now,
        };

        diesel::insert_into(raw_anime_items::table)
            .values(&new_item)
            .on_conflict(raw_anime_items::download_url)
            .do_nothing()
            .get_result::<RawAnimeItem>(conn)
            .map_err(|e| format!("Failed to save raw item: {}", e))
    }

    /// 更新原始項目的解析狀態
    pub fn update_raw_item_status(
        conn: &mut PgConnection,
        item_id: i32,
        status: ParseStatus,
        parser_id: Option<i32>,
        error_message: Option<&str>,
    ) -> Result<(), String> {
        let now = Utc::now().naive_utc();

        diesel::update(raw_anime_items::table.filter(raw_anime_items::item_id.eq(item_id)))
            .set((
                raw_anime_items::status.eq(status.as_str()),
                raw_anime_items::parser_id.eq(parser_id),
                raw_anime_items::error_message.eq(error_message),
                raw_anime_items::parsed_at.eq(Some(now)),
            ))
            .execute(conn)
            .map_err(|e| format!("Failed to update raw item status: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_as_str() {
        assert_eq!(ParseStatus::Pending.as_str(), "pending");
        assert_eq!(ParseStatus::Parsed.as_str(), "parsed");
        assert_eq!(ParseStatus::NoMatch.as_str(), "no_match");
    }
}
```

**Step 2: 更新 services/mod.rs**

加入：

```rust
pub mod title_parser;
pub use title_parser::TitleParserService;
```

**Step 3: 驗證編譯**

Run: `cd /workspace/core-service && cargo check`
Expected: 編譯成功

**Step 4: Commit**

```bash
cd /workspace/core-service
git add src/services/title_parser.rs src/services/mod.rs
git commit -m "feat: implement TitleParserService for configurable title parsing

- Add TitleParserService with priority-based parser matching
- Support regex and static value extraction
- Add methods for saving and updating raw_anime_items
- Include ParseStatus enum and ParsedResult struct

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 5: 實作新的 Fetcher Results Handler

**Files:**
- Modify: `core-service/src/handlers/fetcher_results.rs`

**Step 1: 新增處理原始結果的 handler**

在檔案末尾加入新的 handler：

```rust
use crate::services::TitleParserService;
use crate::services::title_parser::{ParseStatus, ParsedResult};
use shared::models::{RawAnimeItem as SharedRawAnimeItem, RawFetcherResultsPayload, RawFetcherResultsResponse};

/// 處理新架構的原始 fetcher 結果
pub async fn receive_raw_fetcher_results(
    State(state): State<AppState>,
    Json(payload): Json<RawFetcherResultsPayload>,
) -> (StatusCode, Json<serde_json::Value>) {
    tracing::info!(
        "Received raw fetcher results from {}: {} items, subscription_id: {}",
        payload.fetcher_source,
        payload.items.len(),
        payload.subscription_id
    );

    // 更新訂閱的 last_fetched_at
    if let Err(e) = update_subscription_after_fetch(&state, payload.subscription_id, payload.success).await {
        tracing::error!("Failed to update subscription {}: {}", payload.subscription_id, e);
    }

    let mut items_received = 0;
    let mut items_parsed = 0;
    let mut items_failed = 0;
    let mut errors = Vec::new();

    match state.db.get() {
        Ok(mut conn) => {
            for raw_item in payload.items {
                items_received += 1;

                // 轉換 pub_date
                let pub_date = raw_item.pub_date.map(|dt| dt.naive_utc());

                // 儲存原始項目
                let saved_item = match TitleParserService::save_raw_item(
                    &mut conn,
                    &raw_item.title,
                    raw_item.description.as_deref(),
                    &raw_item.download_url,
                    pub_date,
                    payload.subscription_id,
                ) {
                    Ok(item) => item,
                    Err(e) => {
                        // 可能是重複項目（UNIQUE 違反），跳過
                        tracing::debug!("Skipped item (possibly duplicate): {}", e);
                        continue;
                    }
                };

                // 嘗試解析標題
                match TitleParserService::parse_title(&mut conn, &raw_item.title) {
                    Ok(Some(parsed)) => {
                        // 解析成功，建立相關記錄
                        match process_parsed_result(&mut conn, &saved_item, &parsed) {
                            Ok(_) => {
                                TitleParserService::update_raw_item_status(
                                    &mut conn,
                                    saved_item.item_id,
                                    ParseStatus::Parsed,
                                    Some(parsed.parser_id),
                                    None,
                                ).ok();
                                items_parsed += 1;
                                tracing::debug!(
                                    "Parsed: {} -> {} EP{}",
                                    raw_item.title,
                                    parsed.anime_title,
                                    parsed.episode_no
                                );
                            }
                            Err(e) => {
                                TitleParserService::update_raw_item_status(
                                    &mut conn,
                                    saved_item.item_id,
                                    ParseStatus::Failed,
                                    Some(parsed.parser_id),
                                    Some(&e),
                                ).ok();
                                items_failed += 1;
                                errors.push(e);
                            }
                        }
                    }
                    Ok(None) => {
                        // 無匹配解析器
                        TitleParserService::update_raw_item_status(
                            &mut conn,
                            saved_item.item_id,
                            ParseStatus::NoMatch,
                            None,
                            Some("No matching parser found"),
                        ).ok();
                        items_failed += 1;
                        tracing::warn!("No parser matched for: {}", raw_item.title);
                    }
                    Err(e) => {
                        TitleParserService::update_raw_item_status(
                            &mut conn,
                            saved_item.item_id,
                            ParseStatus::Failed,
                            None,
                            Some(&e),
                        ).ok();
                        items_failed += 1;
                        errors.push(e);
                    }
                }
            }

            let response = RawFetcherResultsResponse {
                success: items_failed == 0,
                items_received,
                items_parsed,
                items_failed,
                message: if errors.is_empty() {
                    format!("Processed {} items: {} parsed, {} failed", items_received, items_parsed, items_failed)
                } else {
                    format!("Processed {} items with errors: {:?}", items_received, errors)
                },
            };

            (StatusCode::OK, Json(json!(response)))
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!(RawFetcherResultsResponse {
                    success: false,
                    items_received: 0,
                    items_parsed: 0,
                    items_failed: 0,
                    message: format!("Database connection error: {}", e),
                })),
            )
        }
    }
}

/// 處理解析成功的結果，建立 anime, series, group, link 記錄
fn process_parsed_result(
    conn: &mut PgConnection,
    raw_item: &crate::models::RawAnimeItem,
    parsed: &ParsedResult,
) -> Result<(), String> {
    use sha2::{Sha256, Digest};

    // 1. 建立或取得 anime
    let anime = create_or_get_anime(conn, &parsed.anime_title)?;

    // 2. 建立或取得 season（使用預設值）
    let year = parsed.year.as_ref()
        .and_then(|y| y.parse::<i32>().ok())
        .unwrap_or(2025);
    let season_name = parsed.season.as_deref().unwrap_or("unknown");
    let season = create_or_get_season(conn, year, season_name)?;

    // 3. 建立或取得 series
    let series = create_or_get_series(
        conn,
        anime.anime_id,
        parsed.series_no,
        season.season_id,
        "",  // description
    )?;

    // 4. 建立或取得 subtitle_group
    let group_name = parsed.subtitle_group.as_deref().unwrap_or("未知字幕組");
    let group = create_or_get_subtitle_group(conn, group_name)?;

    // 5. 生成 source_hash
    let mut hasher = Sha256::new();
    hasher.update(raw_item.download_url.as_bytes());
    let source_hash = format!("{:x}", hasher.finalize());

    // 6. 建立 anime_link
    let now = Utc::now().naive_utc();
    let new_link = NewAnimeLink {
        series_id: series.series_id,
        group_id: group.group_id,
        episode_no: parsed.episode_no,
        title: Some(raw_item.title.clone()),
        url: raw_item.download_url.clone(),
        source_hash,
        filtered_flag: false,
        created_at: now,
        raw_item_id: Some(raw_item.item_id),
    };

    diesel::insert_into(anime_links::table)
        .values(&new_link)
        .execute(conn)
        .map_err(|e| format!("Failed to create anime link: {}", e))?;

    Ok(())
}
```

**Step 2: 新增必要的 imports**

在檔案開頭確保有：

```rust
use sha2::{Sha256, Digest};
```

並在 `core-service/Cargo.toml` 的 dependencies 加入（如果沒有）：

```toml
sha2 = "0.10"
```

**Step 3: 驗證編譯**

Run: `cd /workspace/core-service && cargo check`
Expected: 編譯成功

**Step 4: Commit**

```bash
cd /workspace/core-service
git add src/handlers/fetcher_results.rs Cargo.toml
git commit -m "feat: add receive_raw_fetcher_results handler for new architecture

- Handle RawFetcherResultsPayload from fetchers
- Save raw items to raw_anime_items table
- Parse titles using TitleParserService
- Create anime, series, group, link records on success
- Update raw item status based on parse result

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 6: 註冊新的路由

**Files:**
- Modify: `core-service/src/main.rs`
- Modify: `shared/src/api.rs`

**Step 1: 在 shared/src/api.rs 新增路由常數**

```rust
pub const RAW_FETCHER_RESULTS: &str = "/raw-fetcher-results";
pub const PARSERS: &str = "/parsers";
pub const PARSERS_BY_ID: &str = "/parsers/:parser_id";
pub const RAW_ITEMS: &str = "/raw-items";
pub const RAW_ITEMS_BY_ID: &str = "/raw-items/:item_id";
pub const RAW_ITEMS_REPARSE: &str = "/raw-items/:item_id/reparse";
pub const RAW_ITEMS_BATCH_REPARSE: &str = "/raw-items/reparse";
```

**Step 2: 在 core-service/src/main.rs 註冊路由**

找到路由定義區域，加入：

```rust
.route("/raw-fetcher-results", post(handlers::fetcher_results::receive_raw_fetcher_results))
```

**Step 3: 驗證編譯**

Run: `cd /workspace && cargo check --workspace`
Expected: 編譯成功

**Step 4: Commit**

```bash
cd /workspace
git add shared/src/api.rs core-service/src/main.rs
git commit -m "feat: register /raw-fetcher-results route

- Add route constants for new APIs
- Register receive_raw_fetcher_results handler

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 7: 修改 Fetcher 移除解析邏輯

**Files:**
- Modify: `fetchers/mikanani/src/rss_parser.rs`
- Modify: `fetchers/mikanani/src/handlers.rs`
- Modify: `fetchers/mikanani/src/fetch_task.rs`

**Step 1: 簡化 rss_parser.rs**

移除 parse_title 相關邏輯，只保留原始資料抓取：

```rust
use feed_rs::parser;
use sha2::{Sha256, Digest};
use shared::models::RawAnimeItem;
use crate::retry::retry_with_backoff;
use std::time::Duration;
use chrono::{DateTime, Utc};

pub struct RssParser;

impl RssParser {
    pub fn new() -> Self {
        Self
    }

    /// 抓取 RSS 並回傳原始項目（不解析）
    pub async fn fetch_raw_items(&self, rss_url: &str) -> Result<Vec<RawAnimeItem>, String> {
        // Download RSS feed with retry logic
        let url = rss_url.to_string();
        let content = retry_with_backoff(
            3,
            Duration::from_secs(2),
            || {
                let url = url.clone();
                async move {
                    let resp = reqwest::get(&url).await?;
                    let resp = resp.error_for_status()?;
                    resp.bytes().await
                }
            },
        )
        .await
        .map_err(|e| format!("Failed to fetch RSS feed: {}", e))?;

        // Parse RSS
        let feed = parser::parse(&content[..])
            .map_err(|e| format!("Failed to parse RSS feed: {}", e))?;

        let mut items = Vec::new();

        for entry in feed.entries {
            let title = entry.title.map(|t| t.content).unwrap_or_default();
            if title.is_empty() {
                continue;
            }

            // Get download URL from enclosure or link
            let download_url = entry.media.first()
                .and_then(|m| m.content.first())
                .and_then(|c| c.url.as_ref())
                .map(|u| u.to_string())
                .or_else(|| entry.links.first().map(|l| l.href.clone()))
                .unwrap_or_default();

            if download_url.is_empty() {
                continue;
            }

            let description = entry.summary.map(|s| s.content);

            let pub_date = entry.published
                .or(entry.updated)
                .map(|dt| DateTime::<Utc>::from(dt));

            items.push(RawAnimeItem {
                title,
                description,
                download_url,
                pub_date,
            });
        }

        Ok(items)
    }

    /// 生成 URL 的 hash（保留供其他用途）
    pub fn generate_hash(&self, url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

impl Default for RssParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_hash_deterministic() {
        let parser = RssParser::new();
        let url = "magnet:?xt=urn:btih:abc123";
        let hash1 = parser.generate_hash(url);
        let hash2 = parser.generate_hash(url);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }
}
```

**Step 2: 更新 fetch_task.rs 使用新格式**

修改 `execute` 方法回傳 `RawFetcherResultsPayload`：

```rust
use shared::models::{RawAnimeItem, RawFetcherResultsPayload};

// ... 在 execute 方法中 ...

pub async fn execute(&self) -> Result<RawFetcherResultsPayload, String> {
    let parser = RssParser::new();
    let items = parser.fetch_raw_items(&self.rss_url).await?;

    Ok(RawFetcherResultsPayload {
        subscription_id: self.subscription_id,
        items,
        fetcher_source: "mikanani".to_string(),
        success: true,
        error_message: None,
    })
}
```

**Step 3: 更新 handlers.rs 使用新格式**

修改 callback 發送的 payload：

```rust
// 發送到 Core 的 /raw-fetcher-results endpoint
let callback_url = format!("{}/raw-fetcher-results", core_base_url);
client.post(&callback_url)
    .json(&result)
    .send()
    .await
    .map_err(|e| format!("Failed to send results: {}", e))?;
```

**Step 4: 驗證編譯**

Run: `cd /workspace/fetchers/mikanani && cargo check`
Expected: 編譯成功

**Step 5: Commit**

```bash
cd /workspace/fetchers/mikanani
git add src/rss_parser.rs src/handlers.rs src/fetch_task.rs
git commit -m "refactor: remove parsing logic from fetcher, return raw data only

- Simplify RssParser to only fetch raw RSS items
- Remove parse_title and related regex logic
- Update FetchTask to return RawFetcherResultsPayload
- Update handlers to post to /raw-fetcher-results endpoint

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 8: 新增解析器管理 API

**Files:**
- Create: `core-service/src/handlers/parsers.rs`
- Modify: `core-service/src/handlers/mod.rs`
- Modify: `core-service/src/main.rs`

**Step 1: 建立 parsers.rs handler**

```rust
//! 解析器管理 API

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::Utc;

use crate::state::AppState;
use crate::models::{TitleParser, NewTitleParser, ParserSourceType};
use crate::schema::title_parsers;

// ============ DTOs ============

#[derive(Debug, Deserialize)]
pub struct CreateParserRequest {
    pub name: String,
    pub description: Option<String>,
    pub priority: i32,
    pub is_enabled: Option<bool>,
    pub condition_regex: String,
    pub parse_regex: String,
    pub anime_title_source: String,  // "regex" or "static"
    pub anime_title_value: String,
    pub episode_no_source: String,
    pub episode_no_value: String,
    pub series_no_source: Option<String>,
    pub series_no_value: Option<String>,
    pub subtitle_group_source: Option<String>,
    pub subtitle_group_value: Option<String>,
    pub resolution_source: Option<String>,
    pub resolution_value: Option<String>,
    pub season_source: Option<String>,
    pub season_value: Option<String>,
    pub year_source: Option<String>,
    pub year_value: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ParserResponse {
    pub parser_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub priority: i32,
    pub is_enabled: bool,
    pub condition_regex: String,
    pub parse_regex: String,
    pub anime_title_source: String,
    pub anime_title_value: String,
    pub episode_no_source: String,
    pub episode_no_value: String,
    pub series_no_source: Option<String>,
    pub series_no_value: Option<String>,
    pub subtitle_group_source: Option<String>,
    pub subtitle_group_value: Option<String>,
    pub resolution_source: Option<String>,
    pub resolution_value: Option<String>,
    pub season_source: Option<String>,
    pub season_value: Option<String>,
    pub year_source: Option<String>,
    pub year_value: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<TitleParser> for ParserResponse {
    fn from(p: TitleParser) -> Self {
        Self {
            parser_id: p.parser_id,
            name: p.name,
            description: p.description,
            priority: p.priority,
            is_enabled: p.is_enabled,
            condition_regex: p.condition_regex,
            parse_regex: p.parse_regex,
            anime_title_source: p.anime_title_source.to_string(),
            anime_title_value: p.anime_title_value,
            episode_no_source: p.episode_no_source.to_string(),
            episode_no_value: p.episode_no_value,
            series_no_source: p.series_no_source.map(|s| s.to_string()),
            series_no_value: p.series_no_value,
            subtitle_group_source: p.subtitle_group_source.map(|s| s.to_string()),
            subtitle_group_value: p.subtitle_group_value,
            resolution_source: p.resolution_source.map(|s| s.to_string()),
            resolution_value: p.resolution_value,
            season_source: p.season_source.map(|s| s.to_string()),
            season_value: p.season_value,
            year_source: p.year_source.map(|s| s.to_string()),
            year_value: p.year_value,
            created_at: p.created_at.to_string(),
            updated_at: p.updated_at.to_string(),
        }
    }
}

// ============ Handlers ============

/// GET /parsers - 列出所有解析器
pub async fn list_parsers(
    State(state): State<AppState>,
) -> Result<Json<Vec<ParserResponse>>, (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let parsers = title_parsers::table
        .order(title_parsers::priority.desc())
        .load::<TitleParser>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(parsers.into_iter().map(ParserResponse::from).collect()))
}

/// GET /parsers/:parser_id - 取得單一解析器
pub async fn get_parser(
    State(state): State<AppState>,
    Path(parser_id): Path<i32>,
) -> Result<Json<ParserResponse>, (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let parser = title_parsers::table
        .filter(title_parsers::parser_id.eq(parser_id))
        .first::<TitleParser>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Parser not found".to_string()))?;

    Ok(Json(ParserResponse::from(parser)))
}

/// POST /parsers - 新增解析器
pub async fn create_parser(
    State(state): State<AppState>,
    Json(req): Json<CreateParserRequest>,
) -> Result<(StatusCode, Json<ParserResponse>), (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let now = Utc::now().naive_utc();

    let new_parser = NewTitleParser {
        name: req.name,
        description: req.description,
        priority: req.priority,
        is_enabled: req.is_enabled.unwrap_or(true),
        condition_regex: req.condition_regex,
        parse_regex: req.parse_regex,
        anime_title_source: parse_source_type(&req.anime_title_source)?,
        anime_title_value: req.anime_title_value,
        episode_no_source: parse_source_type(&req.episode_no_source)?,
        episode_no_value: req.episode_no_value,
        series_no_source: req.series_no_source.as_ref().map(|s| parse_source_type(s)).transpose()?,
        series_no_value: req.series_no_value,
        subtitle_group_source: req.subtitle_group_source.as_ref().map(|s| parse_source_type(s)).transpose()?,
        subtitle_group_value: req.subtitle_group_value,
        resolution_source: req.resolution_source.as_ref().map(|s| parse_source_type(s)).transpose()?,
        resolution_value: req.resolution_value,
        season_source: req.season_source.as_ref().map(|s| parse_source_type(s)).transpose()?,
        season_value: req.season_value,
        year_source: req.year_source.as_ref().map(|s| parse_source_type(s)).transpose()?,
        year_value: req.year_value,
        created_at: now,
        updated_at: now,
    };

    let parser = diesel::insert_into(title_parsers::table)
        .values(&new_parser)
        .get_result::<TitleParser>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(ParserResponse::from(parser))))
}

/// DELETE /parsers/:parser_id - 刪除解析器
pub async fn delete_parser(
    State(state): State<AppState>,
    Path(parser_id): Path<i32>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let deleted = diesel::delete(title_parsers::table.filter(title_parsers::parser_id.eq(parser_id)))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted == 0 {
        return Err((StatusCode::NOT_FOUND, "Parser not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

fn parse_source_type(s: &str) -> Result<ParserSourceType, (StatusCode, String)> {
    match s {
        "regex" => Ok(ParserSourceType::Regex),
        "static" => Ok(ParserSourceType::Static),
        _ => Err((StatusCode::BAD_REQUEST, format!("Invalid source type: {}", s))),
    }
}
```

**Step 2: 更新 handlers/mod.rs**

```rust
pub mod parsers;
```

**Step 3: 在 main.rs 註冊路由**

```rust
.route("/parsers", get(handlers::parsers::list_parsers).post(handlers::parsers::create_parser))
.route("/parsers/:parser_id", get(handlers::parsers::get_parser).delete(handlers::parsers::delete_parser))
```

**Step 4: 驗證編譯**

Run: `cd /workspace/core-service && cargo check`
Expected: 編譯成功

**Step 5: Commit**

```bash
cd /workspace/core-service
git add src/handlers/parsers.rs src/handlers/mod.rs src/main.rs
git commit -m "feat: add parser management API endpoints

- GET /parsers - list all parsers
- GET /parsers/:id - get single parser
- POST /parsers - create new parser
- DELETE /parsers/:id - delete parser

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 9: 新增原始資料管理 API

**Files:**
- Create: `core-service/src/handlers/raw_items.rs`
- Modify: `core-service/src/handlers/mod.rs`
- Modify: `core-service/src/main.rs`

**Step 1: 建立 raw_items.rs handler**

```rust
//! 原始資料管理 API

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::state::AppState;
use crate::models::RawAnimeItem;
use crate::schema::raw_anime_items;
use crate::services::TitleParserService;
use crate::services::title_parser::ParseStatus;

// ============ DTOs ============

#[derive(Debug, Deserialize)]
pub struct ListRawItemsQuery {
    pub status: Option<String>,
    pub subscription_id: Option<i32>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RawItemResponse {
    pub item_id: i32,
    pub title: String,
    pub description: Option<String>,
    pub download_url: String,
    pub pub_date: Option<String>,
    pub subscription_id: i32,
    pub status: String,
    pub parser_id: Option<i32>,
    pub error_message: Option<String>,
    pub parsed_at: Option<String>,
    pub created_at: String,
}

impl From<RawAnimeItem> for RawItemResponse {
    fn from(item: RawAnimeItem) -> Self {
        Self {
            item_id: item.item_id,
            title: item.title,
            description: item.description,
            download_url: item.download_url,
            pub_date: item.pub_date.map(|d| d.to_string()),
            subscription_id: item.subscription_id,
            status: item.status,
            parser_id: item.parser_id,
            error_message: item.error_message,
            parsed_at: item.parsed_at.map(|d| d.to_string()),
            created_at: item.created_at.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BatchReparseRequest {
    pub status: Option<String>,
    pub parser_id: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ReparseResponse {
    pub success: bool,
    pub items_processed: usize,
    pub items_parsed: usize,
    pub message: String,
}

// ============ Handlers ============

/// GET /raw-items - 列出原始資料
pub async fn list_raw_items(
    State(state): State<AppState>,
    Query(query): Query<ListRawItemsQuery>,
) -> Result<Json<Vec<RawItemResponse>>, (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut q = raw_anime_items::table.into_boxed();

    if let Some(status) = &query.status {
        q = q.filter(raw_anime_items::status.eq(status));
    }

    if let Some(sub_id) = query.subscription_id {
        q = q.filter(raw_anime_items::subscription_id.eq(sub_id));
    }

    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let items = q
        .order(raw_anime_items::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load::<RawAnimeItem>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(items.into_iter().map(RawItemResponse::from).collect()))
}

/// GET /raw-items/:item_id - 取得單一項目
pub async fn get_raw_item(
    State(state): State<AppState>,
    Path(item_id): Path<i32>,
) -> Result<Json<RawItemResponse>, (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let item = raw_anime_items::table
        .filter(raw_anime_items::item_id.eq(item_id))
        .first::<RawAnimeItem>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Item not found".to_string()))?;

    Ok(Json(RawItemResponse::from(item)))
}

/// POST /raw-items/:item_id/reparse - 重新解析單一項目
pub async fn reparse_item(
    State(state): State<AppState>,
    Path(item_id): Path<i32>,
) -> Result<Json<ReparseResponse>, (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let item = raw_anime_items::table
        .filter(raw_anime_items::item_id.eq(item_id))
        .first::<RawAnimeItem>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Item not found".to_string()))?;

    match TitleParserService::parse_title(&mut conn, &item.title) {
        Ok(Some(parsed)) => {
            TitleParserService::update_raw_item_status(
                &mut conn,
                item_id,
                ParseStatus::Parsed,
                Some(parsed.parser_id),
                None,
            ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

            Ok(Json(ReparseResponse {
                success: true,
                items_processed: 1,
                items_parsed: 1,
                message: format!("Parsed: {} EP{}", parsed.anime_title, parsed.episode_no),
            }))
        }
        Ok(None) => {
            TitleParserService::update_raw_item_status(
                &mut conn,
                item_id,
                ParseStatus::NoMatch,
                None,
                Some("No matching parser"),
            ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

            Ok(Json(ReparseResponse {
                success: false,
                items_processed: 1,
                items_parsed: 0,
                message: "No matching parser found".to_string(),
            }))
        }
        Err(e) => {
            TitleParserService::update_raw_item_status(
                &mut conn,
                item_id,
                ParseStatus::Failed,
                None,
                Some(&e),
            ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

            Err((StatusCode::INTERNAL_SERVER_ERROR, e))
        }
    }
}

/// POST /raw-items/:item_id/skip - 標記為跳過
pub async fn skip_item(
    State(state): State<AppState>,
    Path(item_id): Path<i32>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    TitleParserService::update_raw_item_status(
        &mut conn,
        item_id,
        ParseStatus::Skipped,
        None,
        Some("Manually skipped"),
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(StatusCode::NO_CONTENT)
}
```

**Step 2: 更新 handlers/mod.rs**

```rust
pub mod raw_items;
```

**Step 3: 在 main.rs 註冊路由**

```rust
.route("/raw-items", get(handlers::raw_items::list_raw_items))
.route("/raw-items/:item_id", get(handlers::raw_items::get_raw_item))
.route("/raw-items/:item_id/reparse", post(handlers::raw_items::reparse_item))
.route("/raw-items/:item_id/skip", post(handlers::raw_items::skip_item))
```

**Step 4: 驗證編譯**

Run: `cd /workspace/core-service && cargo check`
Expected: 編譯成功

**Step 5: Commit**

```bash
cd /workspace/core-service
git add src/handlers/raw_items.rs src/handlers/mod.rs src/main.rs
git commit -m "feat: add raw items management API endpoints

- GET /raw-items - list raw items with status/subscription filter
- GET /raw-items/:id - get single raw item
- POST /raw-items/:id/reparse - reparse single item
- POST /raw-items/:id/skip - mark item as skipped

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 10: 整合測試

**Files:**
- Create: `core-service/tests/title_parser_test.rs`

**Step 1: 建立整合測試檔案**

```rust
//! 標題解析器整合測試

use regex::Regex;

/// 測試 LoliHouse 標準格式解析
#[test]
fn test_lolihouse_standard_format() {
    let condition_regex = Regex::new(r"^\[.+\].+\s-\s\d+").unwrap();
    let parse_regex = Regex::new(r"^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)\s*\[.*?(\d{3,4}p)").unwrap();

    let title = "[LoliHouse] 黄金神威 最终章 / Golden Kamuy - 53 [WebRip 1080p HEVC-10bit AAC][无字幕]";

    assert!(condition_regex.is_match(title));

    let captures = parse_regex.captures(title).expect("Should match");
    assert_eq!(captures.get(1).unwrap().as_str(), "LoliHouse");
    assert_eq!(captures.get(2).unwrap().as_str().trim(), "黄金神威 最终章 / Golden Kamuy");
    assert_eq!(captures.get(3).unwrap().as_str(), "53");
    assert_eq!(captures.get(4).unwrap().as_str(), "1080p");
}

/// 測試六四位元星號格式解析
#[test]
fn test_star_separator_format() {
    let condition_regex = Regex::new(r"^[^★]+★.+★\d+★").unwrap();
    let parse_regex = Regex::new(r"^([^★]+)★(.+?)★(\d+)★(\d+x\d+)").unwrap();

    let title = "六四位元字幕组★可以帮忙洗干净吗？Kirei ni Shite Moraemasu ka★04★1920x1080★AVC AAC MP4★繁体中文";

    assert!(condition_regex.is_match(title));

    let captures = parse_regex.captures(title).expect("Should match");
    assert_eq!(captures.get(1).unwrap().as_str(), "六四位元字幕组");
    assert_eq!(captures.get(2).unwrap().as_str(), "可以帮忙洗干净吗？Kirei ni Shite Moraemasu ka");
    assert_eq!(captures.get(3).unwrap().as_str(), "04");
    assert_eq!(captures.get(4).unwrap().as_str(), "1920x1080");
}

/// 測試預設解析器格式
#[test]
fn test_default_parser_format() {
    let condition_regex = Regex::new(r".+\s-\s\d+").unwrap();
    let parse_regex = Regex::new(r"^(.+?)\s+-\s*(\d+)").unwrap();

    let titles = vec![
        "[LoliHouse] 神八小妹不可怕 / Kaya-chan wa Kowakunai - 03 [WebRip 1080p]",
        "[豌豆字幕组&LoliHouse] 地狱乐 / Jigokuraku - 16 [WebRip 1080p]",
    ];

    for title in titles {
        assert!(condition_regex.is_match(title), "Should match: {}", title);

        let captures = parse_regex.captures(title).expect("Should parse");
        let episode: i32 = captures.get(2).unwrap().as_str().parse().unwrap();
        assert!(episode > 0);
    }
}

/// 測試不匹配的標題
#[test]
fn test_non_matching_title() {
    let condition_regex = Regex::new(r"^\[.+\].+\s-\s\d+").unwrap();

    let non_matching = vec![
        "Random text without brackets",
        "Just some anime title",
        "[Group only] no episode number",
    ];

    for title in non_matching {
        assert!(!condition_regex.is_match(title), "Should not match: {}", title);
    }
}
```

**Step 2: 執行測試**

Run: `cd /workspace/core-service && cargo test title_parser`
Expected: All tests pass

**Step 3: Commit**

```bash
cd /workspace/core-service
git add tests/title_parser_test.rs
git commit -m "test: add integration tests for title parser regex patterns

- Test LoliHouse standard format
- Test star separator format
- Test default parser format
- Test non-matching titles

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 11: 端對端驗證

**Step 1: 啟動 Core Service**

Run: `cd /workspace/core-service && cargo run`
Expected: Server running on configured port

**Step 2: 測試解析器 API**

Run: `curl http://localhost:3000/parsers | jq`
Expected: 回傳預設的 3 個解析器

**Step 3: 測試原始資料 API**

Run: `curl http://localhost:3000/raw-items | jq`
Expected: 回傳空陣列或現有資料

**Step 4: 模擬 Fetcher 回傳**

```bash
curl -X POST http://localhost:3000/raw-fetcher-results \
  -H "Content-Type: application/json" \
  -d '{
    "subscription_id": 1,
    "items": [
      {
        "title": "[LoliHouse] 黄金神威 最终章 / Golden Kamuy - 53 [WebRip 1080p HEVC-10bit AAC][无字幕]",
        "description": "Test description",
        "download_url": "https://example.com/test.torrent",
        "pub_date": "2026-01-30T12:00:00Z"
      }
    ],
    "fetcher_source": "mikanani",
    "success": true,
    "error_message": null
  }'
```

Expected: 回傳解析成功的結果

**Step 5: 驗證資料已儲存**

Run: `curl http://localhost:3000/raw-items | jq`
Expected: 回傳剛才新增的項目，status 為 "parsed"

---

## 完成檢查清單

- [ ] Task 1: Migration 建立並執行成功
- [ ] Task 2: ENUM 和 Model 定義完成
- [ ] Task 3: Shared 模組更新完成
- [ ] Task 4: TitleParserService 實作完成
- [ ] Task 5: 新的 fetcher results handler 實作完成
- [ ] Task 6: 路由註冊完成
- [ ] Task 7: Fetcher 移除解析邏輯
- [ ] Task 8: 解析器管理 API 完成
- [ ] Task 9: 原始資料管理 API 完成
- [ ] Task 10: 整合測試通過
- [ ] Task 11: 端對端驗證通過

---

## 版本歷程

| 版本 | 日期 | 變更內容 |
|------|------|---------|
| 1.0 | 2026-01-30 | 初版實作計畫 |
