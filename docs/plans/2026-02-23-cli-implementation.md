# CLI 完整重寫實作計劃

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 完全重寫 `bangumi` CLI，正確對應所有後端 API 端點，提供 human-readable 表格輸出與 `--json` 腳本模式。

**Architecture:** 資源導向指令結構（`bangumi subscription list/add/...`）搭配短別名（`sub`, `raw`, `dl`, `sg`, `st`），每個資源對應一個 `commands/*.rs` 模組。`ApiClient` 統一處理 HTTP 請求與錯誤，`output.rs` 統一格式化輸出。

**Tech Stack:** Rust + clap v4 (derive) + reqwest + tabled 0.15 + colored 2.1 + anyhow

---

## 背景知識

### 專案結構
- CLI 位於 `/workspace/cli/`
- Workspace root: `/workspace/Cargo.toml`
- Core Service API base URL: `http://localhost:8000`
- Downloader Service URL（qb-config 用）: `http://localhost:8002`

### 關鍵 API 端點（完整清單）

```
Core Service (port 8000):
GET    /dashboard/stats
GET    /services
GET    /subscriptions
POST   /subscriptions          body: {source_url, name?, fetch_interval_minutes?}
PATCH  /subscriptions/:id      body: {name?, fetch_interval_minutes?, is_active?}
DELETE /subscriptions/:id?purge=true
GET    /anime
POST   /anime                  body: {title}
DELETE /anime/:id
GET    /anime/:id/series
GET    /series
GET    /anime/series/:id
PUT    /anime/series/:id       body: {season_id?, description?, aired_date?, end_date?}
GET    /links/:series_id
GET    /raw-items?status=&subscription_id=&limit=&offset=
GET    /raw-items/:id
POST   /raw-items/:id/reparse
POST   /raw-items/:id/skip
GET    /downloads?status=&limit=&offset=
GET    /conflicts
POST   /conflicts/:id/resolve  body: {fetcher_id}
GET    /link-conflicts
POST   /link-conflicts/:id/resolve  body: {chosen_link_id}
GET    /filters?target_type=&target_id=
POST   /filters                body: {target_type, target_id?, rule_order, is_positive, regex_pattern}
DELETE /filters/:id
POST   /filters/preview        body: {target_type?, target_id?, rule_order, is_positive, regex_pattern}
GET    /parsers?created_from_type=&created_from_id=
GET    /parsers/:id
POST   /parsers
PUT    /parsers/:id
DELETE /parsers/:id
POST   /parsers/preview
GET    /subtitle-groups
POST   /subtitle-groups        body: {group_name}
DELETE /subtitle-groups/:id

Downloader Service (port 8002):
POST   /config/credentials     body: {username, password}
```

### clap 別名語法
```rust
#[derive(Subcommand)]
enum Commands {
    #[command(name = "subscription", aliases = &["sub"], about = "...")]
    Subscription { #[command(subcommand)] action: SubscriptionAction },
}
```

### 環境變數
- `BANGUMI_API_URL` → `--api-url`（預設 `http://localhost:8000`）
- `BANGUMI_DOWNLOADER_URL`（qb-config 用，預設 `http://localhost:8002`）

---

## Task 1: 更新 Cargo.toml

**Files:**
- Modify: `cli/Cargo.toml`

**Step 1: 更新依賴**

將 `cli/Cargo.toml` 內容改為：

```toml
[package]
name = "bangumi-cli"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[[bin]]
name = "bangumi"
path = "src/main.rs"

[dependencies]
shared = { path = "../shared" }

tokio.workspace = true
reqwest.workspace = true

serde.workspace = true
serde_json.workspace = true

chrono.workspace = true
uuid.workspace = true

clap.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true

dotenv.workspace = true

anyhow.workspace = true
thiserror.workspace = true

tabled = "0.15"
colored = "2.1"

[dev-dependencies]
```

**Step 2: 確認可編譯（空的 main.rs 暫時不動）**

```bash
cd /workspace && cargo check -p bangumi-cli 2>&1 | head -30
```

如果出現 `tabled` 或 `colored` 找不到的 error，確認版本正確。

**Step 3: Commit**

```bash
git add cli/Cargo.toml
git commit -m "chore(cli): replace prettytable-rs with tabled and colored"
```

---

## Task 2: 建立 output.rs

**Files:**
- Create: `cli/src/output.rs`

**Step 1: 建立輸出工具模組**

```rust
// cli/src/output.rs
use colored::Colorize;
use serde::Serialize;

/// 列印任何 Serialize 類型為 JSON（--json 模式）
pub fn print_json<T: Serialize>(data: &T) {
    match serde_json::to_string_pretty(data) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("JSON 序列化失敗: {}", e),
    }
}

/// 列印 key-value 詳情（show 指令用）
pub fn print_kv(title: &str, pairs: &[(&str, String)]) {
    println!("{}", title.bold());
    for (k, v) in pairs {
        println!("  {}: {}", k.cyan(), v);
    }
}

/// 格式化狀態顯示（帶顏色）
pub fn format_status(status: &str) -> String {
    match status {
        "active" | "completed" | "synced" | "healthy" | "true" | "parsed" => {
            status.green().to_string()
        }
        "downloading" | "pending" | "in_progress" => status.yellow().to_string(),
        "failed" | "error" | "unhealthy" | "false" | "no_match" => {
            status.red().to_string()
        }
        "skipped" | "paused" | "inactive" => status.dimmed().to_string(),
        _ => status.to_string(),
    }
}

/// 格式化布林值
pub fn format_bool(v: bool) -> String {
    if v {
        "✓".green().to_string()
    } else {
        "✗".red().to_string()
    }
}

/// 格式化 Option<String>，None 顯示 "-"
pub fn opt_str(v: &Option<String>) -> String {
    v.as_deref().unwrap_or("-").to_string()
}

/// 格式化 Option<i64>
pub fn opt_i64(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "-".to_string())
}

/// 顯示成功訊息
pub fn print_success(msg: &str) {
    println!("{} {}", "✓".green(), msg);
}

/// 顯示錯誤訊息
pub fn print_error(msg: &str) {
    eprintln!("{} {}", "✗".red(), msg);
}
```

**Step 2: 確認編譯**

```bash
cd /workspace && cargo check -p bangumi-cli 2>&1 | head -30
```

（此時 output.rs 還不被 main.rs 引用，可能有 unused warning，忽略即可）

---

## Task 3: 重寫 client.rs

**Files:**
- Modify: `cli/src/client.rs`（完整替換）

**Step 1: 替換 client.rs**

```rust
// cli/src/client.rs
use serde::de::DeserializeOwned;
use serde::Serialize;

/// HTTP API 客戶端
#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    pub base_url: String,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    /// 處理非成功 HTTP 回應，嘗試解析錯誤訊息
    async fn handle_error(response: reqwest::Response) -> anyhow::Error {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        // 嘗試從 JSON 取出 message 欄位
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
                return anyhow::anyhow!("HTTP {}: {}", status, msg);
            }
            if let Some(msg) = val.get("error").and_then(|m| m.as_str()) {
                return anyhow::anyhow!("HTTP {}: {}", status, msg);
            }
        }
        anyhow::anyhow!("HTTP {}: {}", status, text)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        response
            .json::<T>()
            .await
            .map_err(|e| anyhow::anyhow!("解析回應失敗: {}", e))
    }

    pub async fn post<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow::Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        response
            .json::<R>()
            .await
            .map_err(|e| anyhow::anyhow!("解析回應失敗: {}", e))
    }

    /// POST 無 body，僅取得回應
    pub async fn post_no_body<R: DeserializeOwned>(&self, path: &str) -> anyhow::Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        response
            .json::<R>()
            .await
            .map_err(|e| anyhow::anyhow!("解析回應失敗: {}", e))
    }

    /// POST 無 body，不解析回應
    pub async fn post_no_body_ignore_response(&self, path: &str) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        Ok(())
    }

    pub async fn patch<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow::Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .patch(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        response
            .json::<R>()
            .await
            .map_err(|e| anyhow::anyhow!("解析回應失敗: {}", e))
    }

    pub async fn put<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow::Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .put(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        response
            .json::<R>()
            .await
            .map_err(|e| anyhow::anyhow!("解析回應失敗: {}", e))
    }

    pub async fn delete(&self, path: &str) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        Ok(())
    }
}
```

**Step 2: 確認編譯**

```bash
cd /workspace && cargo check -p bangumi-cli 2>&1 | head -30
```

---

## Task 4: 重寫 models.rs

**Files:**
- Modify: `cli/src/models.rs`（完整替換）

**Step 1: 替換 models.rs**

```rust
// cli/src/models.rs
// 所有 API response/request DTO 均定義於此
// 欄位名稱需與後端 JSON 完全一致（snake_case）

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ==============================
// Dashboard
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct DashboardStats {
    pub total_anime: i64,
    pub total_series: i64,
    pub active_subscriptions: i64,
    pub total_downloads: i64,
    pub downloading: i64,
    pub completed: i64,
    pub failed: i64,
    pub pending_raw_items: i64,
    pub pending_conflicts: i64,
    pub services: Vec<ServiceHealth>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServiceHealth {
    pub name: String,
    pub module_type: String,
    pub is_healthy: bool,
}

// ==============================
// Subscriptions
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct SubscriptionsResponse {
    pub subscriptions: Vec<SubscriptionResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SubscriptionResponse {
    pub subscription_id: i64,
    pub fetcher_id: Option<i64>,
    pub source_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub last_fetched_at: Option<DateTime<Utc>>,
    pub next_fetch_at: Option<DateTime<Utc>>,
    pub fetch_interval_minutes: i32,
    pub is_active: bool,
    pub source_type: Option<String>,
    pub assignment_status: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreateSubscriptionRequest {
    pub source_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_interval_minutes: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct UpdateSubscriptionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_interval_minutes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
}

// ==============================
// Anime
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct AnimesResponse {
    pub animes: Vec<AnimeResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AnimeResponse {
    pub anime_id: i64,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreateAnimeRequest {
    pub title: String,
}

// ==============================
// Series
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct SeriesListResponse {
    pub series: Vec<AnimeSeriesRichResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AnimeSeriesRichResponse {
    pub series_id: i64,
    pub anime_id: i64,
    pub anime_title: String,
    pub series_no: i32,
    pub season: Option<SeasonInfo>,
    pub episode_downloaded: i64,
    pub episode_found: i64,
    pub subscriptions: Vec<SeriesSubscriptionRef>,
    pub description: Option<String>,
    pub aired_date: Option<String>,
    pub end_date: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SeasonInfo {
    pub year: i32,
    pub season: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SeriesSubscriptionRef {
    pub subscription_id: i64,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateSeriesRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aired_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
}

// ==============================
// Anime Links
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct LinksResponse {
    pub links: Vec<AnimeLinkRichResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AnimeLinkRichResponse {
    pub link_id: i64,
    pub series_id: i64,
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub conflict_flag: bool,
    pub download: Option<DownloadInfo>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DownloadInfo {
    pub download_id: i64,
    pub status: String,
    pub progress: Option<f64>,
    pub torrent_hash: Option<String>,
}

// ==============================
// Raw Items
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct RawItemsResponse {
    pub items: Vec<RawItemResponse>,
    pub total: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawItemResponse {
    pub item_id: i64,
    pub subscription_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub download_url: String,
    pub pub_date: Option<DateTime<Utc>>,
    pub status: String,
    pub parser_id: Option<i64>,
    pub parsed_title: Option<String>,
    pub parsed_episode_no: Option<i32>,
    pub filtered_flag: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ==============================
// Downloads
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct DownloadsResponse {
    pub downloads: Vec<DownloadResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DownloadResponse {
    pub download_id: i64,
    pub link_id: Option<i64>,
    pub status: String,
    pub progress: Option<f64>,
    pub torrent_hash: Option<String>,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ==============================
// Conflicts
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct ConflictsResponse {
    pub conflicts: Vec<ConflictResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConflictResponse {
    pub conflict_id: i64,
    pub rss_url: Option<String>,
    pub source_url: Option<String>,
    pub candidate_fetchers: Vec<CandidateFetcher>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CandidateFetcher {
    pub fetcher_id: i64,
    pub fetcher_name: String,
}

#[derive(Debug, Serialize)]
pub struct ResolveConflictRequest {
    pub fetcher_id: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LinkConflictsResponse {
    pub conflicts: Vec<LinkConflictResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LinkConflictResponse {
    pub conflict_id: i64,
    pub series_id: i64,
    pub episode_no: i32,
    pub conflicting_links: Vec<ConflictingLink>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConflictingLink {
    pub link_id: i64,
    pub group_name: Option<String>,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct ResolveLinkConflictRequest {
    pub chosen_link_id: i64,
}

// ==============================
// Filters
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct FiltersResponse {
    pub rules: Vec<FilterRuleResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FilterRuleResponse {
    pub rule_id: i64,
    pub target_type: String,
    pub target_id: Option<i64>,
    pub rule_order: i32,
    pub is_positive: bool,
    pub regex_pattern: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreateFilterRuleRequest {
    pub target_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<i64>,
    pub rule_order: i32,
    pub is_positive: bool,
    pub regex_pattern: String,
}

// ==============================
// Parsers
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct ParsersResponse {
    pub parsers: Vec<ParserResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ParserResponse {
    pub parser_id: i64,
    pub name: String,
    pub priority: i32,
    pub condition_regex: Option<String>,
    pub enabled: bool,
    pub created_from_type: Option<String>,
    pub created_from_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreateParserRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_from_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_from_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct UpdateParserRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

// ==============================
// Subtitle Groups
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct SubtitleGroupsResponse {
    pub groups: Vec<SubtitleGroupResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SubtitleGroupResponse {
    pub group_id: i64,
    pub group_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreateSubtitleGroupRequest {
    pub group_name: String,
}

// ==============================
// Generic
// ==============================

/// 通用成功回應
#[derive(Debug, Deserialize, Serialize)]
pub struct MessageResponse {
    pub message: String,
}
```

**Step 2: 確認編譯**

```bash
cd /workspace && cargo check -p bangumi-cli 2>&1 | head -30
```

**Step 3: Commit**

```bash
git add cli/src/output.rs cli/src/client.rs cli/src/models.rs
git commit -m "refactor(cli): rewrite output, client, models for full API coverage"
```

---

## Task 5: 建立 commands/ 目錄結構

**Files:**
- Create: `cli/src/commands/mod.rs`
- Delete old: `cli/src/commands.rs`（用 mod.rs 取代）

**Step 1: 建立 commands/mod.rs（空的 dispatch 架構）**

先建立目錄和佔位符，讓整個 crate 可以編譯：

```rust
// cli/src/commands/mod.rs
pub mod status;
pub mod subscription;
pub mod anime;
pub mod series;
pub mod raw_item;
pub mod conflict;
pub mod download;
pub mod filter;
pub mod parser;
pub mod subtitle_group;
pub mod qb_config;
```

**Step 2: 為每個模組建立空白佔位符**

在 `cli/src/commands/` 下建立以下各個檔案，每個都只有一個空的 pub 函式佔位：

`status.rs`:
```rust
use crate::client::ApiClient;
use anyhow::Result;
pub async fn run(client: &ApiClient, json: bool) -> Result<()> { todo!() }
```

`subscription.rs`, `anime.rs`, `series.rs`, `raw_item.rs`, `conflict.rs`, `download.rs`, `filter.rs`, `parser.rs`, `subtitle_group.rs`, `qb_config.rs` — 同樣格式。

**Step 3: 刪除舊的 commands.rs 並確認沒有 tests 模組引用**

檢查 `cli/src/main.rs` 和 `cli/src/tests.rs`（如存在），移除舊的 `mod commands;` 與 `mod models;`。

確認 `cli/src/` 目錄下：
- 刪除 `commands.rs`
- 保留 `models.rs`（已在 Task 4 替換）

**Step 4: 確認編譯**

```bash
cd /workspace && cargo check -p bangumi-cli 2>&1 | head -30
```

---

## Task 6: 重寫 main.rs（完整指令結構）

**Files:**
- Modify: `cli/src/main.rs`（完整替換）

**Step 1: 替換 main.rs**

```rust
// cli/src/main.rs
use clap::{Parser, Subcommand};
use std::process;

mod client;
mod commands;
mod models;
mod output;

use client::ApiClient;

#[derive(Parser)]
#[command(
    name = "bangumi",
    about = "動畫 RSS 聚合、下載與媒體庫管理系統",
    version
)]
struct Cli {
    /// Core Service URL（或設定環境變數 BANGUMI_API_URL）
    #[arg(
        global = true,
        long,
        env = "BANGUMI_API_URL",
        default_value = "http://localhost:8000"
    )]
    api_url: String,

    /// 以 JSON 格式輸出（適合腳本）
    #[arg(global = true, long)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 查看系統狀態與統計資訊
    #[command(name = "status", alias = "st")]
    Status,

    /// RSS 訂閱管理
    #[command(name = "subscription", aliases = &["sub"])]
    Subscription {
        #[command(subcommand)]
        action: commands::subscription::SubscriptionAction,
    },

    /// 動畫條目管理
    #[command(name = "anime")]
    Anime {
        #[command(subcommand)]
        action: commands::anime::AnimeAction,
    },

    /// 動畫系列查詢與管理
    #[command(name = "series")]
    Series {
        #[command(subcommand)]
        action: commands::series::SeriesAction,
    },

    /// Raw RSS 項目瀏覽與操作
    #[command(name = "raw-item", aliases = &["raw"])]
    RawItem {
        #[command(subcommand)]
        action: commands::raw_item::RawItemAction,
    },

    /// 衝突列表與解決
    #[command(name = "conflict")]
    Conflict {
        #[command(subcommand)]
        action: commands::conflict::ConflictAction,
    },

    /// 下載記錄查詢
    #[command(name = "download", aliases = &["dl"])]
    Download {
        #[command(subcommand)]
        action: commands::download::DownloadAction,
    },

    /// 過濾規則管理
    #[command(name = "filter")]
    Filter {
        #[command(subcommand)]
        action: commands::filter::FilterAction,
    },

    /// 標題解析器管理
    #[command(name = "parser")]
    Parser {
        #[command(subcommand)]
        action: commands::parser::ParserAction,
    },

    /// 字幕組管理
    #[command(name = "subtitle-group", aliases = &["sg"])]
    SubtitleGroup {
        #[command(subcommand)]
        action: commands::subtitle_group::SubtitleGroupAction,
    },

    /// qBittorrent 連線設定
    #[command(name = "qb-config")]
    QbConfig {
        #[command(subcommand)]
        action: commands::qb_config::QbConfigAction,
    },
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let cli = Cli::parse();
    let client = ApiClient::new(cli.api_url.clone());
    let json = cli.json;

    let result = match cli.command {
        Commands::Status => commands::status::run(&client, json).await,
        Commands::Subscription { action } => {
            commands::subscription::run(&client, action, json).await
        }
        Commands::Anime { action } => commands::anime::run(&client, action, json).await,
        Commands::Series { action } => commands::series::run(&client, action, json).await,
        Commands::RawItem { action } => commands::raw_item::run(&client, action, json).await,
        Commands::Conflict { action } => commands::conflict::run(&client, action, json).await,
        Commands::Download { action } => commands::download::run(&client, action, json).await,
        Commands::Filter { action } => commands::filter::run(&client, action, json).await,
        Commands::Parser { action } => commands::parser::run(&client, action, json).await,
        Commands::SubtitleGroup { action } => {
            commands::subtitle_group::run(&client, action, json).await
        }
        Commands::QbConfig { action } => commands::qb_config::run(&client, action, json).await,
    };

    if let Err(e) = result {
        output::print_error(&e.to_string());
        process::exit(2);
    }
}
```

**Step 2: 確認 `#[cfg(test)] mod tests;` 不存在（舊的測試模組要移除）**

如果 `cli/src/tests.rs` 存在，先刪除，或在 main.rs 裡移除 `mod tests;`。

**Step 3: 確認編譯（`todo!()` 會有警告但應能編譯）**

```bash
cd /workspace && cargo check -p bangumi-cli 2>&1 | head -50
```

---

## Task 7: 實作 commands/status.rs

**Files:**
- Modify: `cli/src/commands/status.rs`

**Step 1: 實作 status 指令**

```rust
// cli/src/commands/status.rs
use crate::client::ApiClient;
use crate::models::DashboardStats;
use crate::output;
use anyhow::Result;
use colored::Colorize;

pub async fn run(client: &ApiClient, json: bool) -> Result<()> {
    let stats: DashboardStats = client.get("/dashboard/stats").await?;

    if json {
        output::print_json(&stats);
        return Ok(());
    }

    // 系統統計
    println!("{}", "=== 系統統計 ===".bold());
    println!("  動畫總數:     {}", stats.total_anime);
    println!("  系列總數:     {}", stats.total_series);
    println!("  活躍訂閱:     {}", stats.active_subscriptions);
    println!();

    println!("{}", "=== 下載狀態 ===".bold());
    println!("  下載中:       {}", stats.downloading.to_string().yellow());
    println!("  已完成:       {}", stats.completed.to_string().green());
    println!("  失敗:         {}", stats.failed.to_string().red());
    println!("  總計:         {}", stats.total_downloads);
    println!();

    println!("{}", "=== 待處理 ===".bold());
    if stats.pending_raw_items > 0 {
        println!("  待解析 Raw Items: {}", stats.pending_raw_items.to_string().yellow());
    } else {
        println!("  待解析 Raw Items: {}", "0".green());
    }
    if stats.pending_conflicts > 0 {
        println!("  待解決衝突:       {}", stats.pending_conflicts.to_string().red());
    } else {
        println!("  待解決衝突:       {}", "0".green());
    }
    println!();

    // 服務健康
    println!("{}", "=== 服務狀態 ===".bold());
    for svc in &stats.services {
        let status = if svc.is_healthy {
            "✓ 健康".green().to_string()
        } else {
            "✗ 不健康".red().to_string()
        };
        println!("  [{:10}] {}: {}", svc.module_type, svc.name, status);
    }

    Ok(())
}
```

**Step 2: 確認編譯**

```bash
cd /workspace && cargo check -p bangumi-cli 2>&1 | head -30
```

---

## Task 8: 實作 commands/subscription.rs

**Files:**
- Modify: `cli/src/commands/subscription.rs`

```rust
// cli/src/commands/subscription.rs
use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum SubscriptionAction {
    /// 列出所有訂閱
    #[command(about = "列出所有 RSS 訂閱")]
    List,

    /// 新增訂閱
    #[command(about = "新增 RSS 訂閱")]
    Add {
        /// RSS URL
        url: String,
        /// 訂閱名稱（選填）
        #[arg(long, short = 'n')]
        name: Option<String>,
        /// 抓取間隔（分鐘，預設 60）
        #[arg(long, short = 'i')]
        interval: Option<i32>,
    },

    /// 顯示訂閱詳情
    #[command(about = "顯示訂閱詳情")]
    Show {
        /// 訂閱 ID
        id: i64,
    },

    /// 更新訂閱設定
    #[command(about = "更新訂閱設定")]
    Update {
        /// 訂閱 ID
        id: i64,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        interval: Option<i32>,
        /// 啟用訂閱
        #[arg(long, conflicts_with = "inactive")]
        active: bool,
        /// 停用訂閱
        #[arg(long, conflicts_with = "active")]
        inactive: bool,
    },

    /// 刪除訂閱
    #[command(about = "刪除訂閱（--purge 完整清除含下載記錄）")]
    Delete {
        /// 訂閱 ID
        id: i64,
        /// 硬刪除（含清理下載記錄與媒體）
        #[arg(long)]
        purge: bool,
    },
}

#[derive(Tabled)]
struct SubRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "名稱")]
    name: String,
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "間隔(分)")]
    interval: i32,
    #[tabled(rename = "狀態")]
    status: String,
    #[tabled(rename = "上次抓取")]
    last_fetched: String,
}

pub async fn run(client: &ApiClient, action: SubscriptionAction, json: bool) -> Result<()> {
    match action {
        SubscriptionAction::List => {
            let resp: SubscriptionsResponse = client.get("/subscriptions").await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            if resp.subscriptions.is_empty() {
                println!("尚無訂閱");
                return Ok(());
            }
            let rows: Vec<SubRow> = resp.subscriptions.iter().map(|s| SubRow {
                id: s.subscription_id,
                name: output::opt_str(&s.name),
                url: if s.source_url.len() > 60 {
                    format!("{}...", &s.source_url[..60])
                } else {
                    s.source_url.clone()
                },
                interval: s.fetch_interval_minutes,
                status: output::format_status(if s.is_active { "active" } else { "inactive" }),
                last_fetched: s
                    .last_fetched_at
                    .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "-".to_string()),
            }).collect();
            println!("{}", Table::new(rows));
        }

        SubscriptionAction::Add { url, name, interval } => {
            let req = CreateSubscriptionRequest {
                source_url: url,
                name,
                fetch_interval_minutes: interval,
            };
            let resp: SubscriptionResponse = client.post("/subscriptions", &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("訂閱已建立 (ID: {})", resp.subscription_id));
            println!("  URL: {}", resp.source_url);
            println!("  間隔: {} 分鐘", resp.fetch_interval_minutes);
        }

        SubscriptionAction::Show { id } => {
            let resp: SubscriptionsResponse = client.get("/subscriptions").await?;
            let sub = resp
                .subscriptions
                .iter()
                .find(|s| s.subscription_id == id)
                .ok_or_else(|| anyhow::anyhow!("找不到訂閱 ID: {}", id))?;
            if json {
                return Ok(output::print_json(sub));
            }
            output::print_kv(
                &format!("訂閱 #{}", id),
                &[
                    ("ID", sub.subscription_id.to_string()),
                    ("名稱", output::opt_str(&sub.name)),
                    ("URL", sub.source_url.clone()),
                    ("間隔", format!("{} 分鐘", sub.fetch_interval_minutes)),
                    ("狀態", output::format_status(if sub.is_active { "active" } else { "inactive" })),
                    (
                        "上次抓取",
                        sub.last_fetched_at
                            .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "下次抓取",
                        sub.next_fetch_at
                            .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                ],
            );
        }

        SubscriptionAction::Update { id, name, interval, active, inactive } => {
            let is_active = if active {
                Some(true)
            } else if inactive {
                Some(false)
            } else {
                None
            };
            let req = UpdateSubscriptionRequest {
                name,
                fetch_interval_minutes: interval,
                is_active,
            };
            let resp: SubscriptionResponse =
                client.patch(&format!("/subscriptions/{}", id), &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("訂閱 #{} 已更新", id));
        }

        SubscriptionAction::Delete { id, purge } => {
            let path = if purge {
                format!("/subscriptions/{}?purge=true", id)
            } else {
                format!("/subscriptions/{}", id)
            };
            client.delete(&path).await?;
            if json {
                return Ok(output::print_json(&serde_json::json!({"deleted": id, "purge": purge})));
            }
            output::print_success(&format!(
                "訂閱 #{} 已刪除{}",
                id,
                if purge { "（含完整清除）" } else { "" }
            ));
        }
    }
    Ok(())
}
```

**確認編譯：**
```bash
cd /workspace && cargo check -p bangumi-cli 2>&1 | head -30
```

---

## Task 9: 實作 commands/anime.rs

**Files:**
- Modify: `cli/src/commands/anime.rs`

```rust
// cli/src/commands/anime.rs
use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum AnimeAction {
    /// 列出所有動畫
    #[command(about = "列出所有動畫條目")]
    List,

    /// 新增動畫
    #[command(about = "新增動畫條目")]
    Add {
        /// 動畫標題
        title: String,
    },

    /// 刪除動畫
    #[command(about = "刪除動畫條目")]
    Delete {
        /// 動畫 ID
        id: i64,
    },

    /// 列出某動畫的所有系列
    #[command(about = "列出動畫下的所有系列")]
    Series {
        /// 動畫 ID
        anime_id: i64,
    },
}

#[derive(Tabled)]
struct AnimeRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "標題")]
    title: String,
    #[tabled(rename = "建立時間")]
    created_at: String,
}

pub async fn run(client: &ApiClient, action: AnimeAction, json: bool) -> Result<()> {
    match action {
        AnimeAction::List => {
            let resp: AnimesResponse = client.get("/anime").await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            if resp.animes.is_empty() {
                println!("尚無動畫");
                return Ok(());
            }
            let rows: Vec<AnimeRow> = resp
                .animes
                .iter()
                .map(|a| AnimeRow {
                    id: a.anime_id,
                    title: a.title.clone(),
                    created_at: a.created_at.format("%Y-%m-%d").to_string(),
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        AnimeAction::Add { title } => {
            let req = CreateAnimeRequest { title: title.clone() };
            let resp: AnimeResponse = client.post("/anime", &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("動畫已建立: {} (ID: {})", resp.title, resp.anime_id));
        }

        AnimeAction::Delete { id } => {
            client.delete(&format!("/anime/{}", id)).await?;
            if json {
                return Ok(output::print_json(&serde_json::json!({"deleted": id})));
            }
            output::print_success(&format!("動畫 #{} 已刪除", id));
        }

        AnimeAction::Series { anime_id } => {
            // GET /anime/:anime_id/series 回傳格式需確認，使用 serde_json::Value 做寬鬆解析
            let resp: serde_json::Value =
                client.get(&format!("/anime/{}/series", anime_id)).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }
    Ok(())
}
```

---

## Task 10: 實作 commands/series.rs

**Files:**
- Modify: `cli/src/commands/series.rs`

```rust
// cli/src/commands/series.rs
use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum SeriesAction {
    /// 列出所有動畫系列
    #[command(about = "列出所有動畫系列（含集數統計）")]
    List {
        /// 篩選特定動畫 ID
        #[arg(long)]
        anime: Option<i64>,
    },

    /// 顯示系列詳情
    #[command(about = "顯示系列詳情")]
    Show {
        /// 系列 ID
        id: i64,
    },

    /// 更新系列元資料
    #[command(about = "更新系列元資料")]
    Update {
        /// 系列 ID
        id: i64,
        #[arg(long)]
        description: Option<String>,
        #[arg(long, help = "開播日期，格式: YYYY-MM-DD")]
        aired_date: Option<String>,
        #[arg(long, help = "完結日期，格式: YYYY-MM-DD")]
        end_date: Option<String>,
        #[arg(long)]
        season_id: Option<i64>,
    },

    /// 列出系列的所有集數連結
    #[command(about = "列出系列集數與下載狀態")]
    Links {
        /// 系列 ID
        id: i64,
    },
}

#[derive(Tabled)]
struct SeriesRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "動畫")]
    anime: String,
    #[tabled(rename = "季")]
    series_no: i32,
    #[tabled(rename = "播出季")]
    season: String,
    #[tabled(rename = "已下載")]
    downloaded: i64,
    #[tabled(rename = "已找到")]
    found: i64,
}

#[derive(Tabled)]
struct LinkRow {
    #[tabled(rename = "Link ID")]
    link_id: i64,
    #[tabled(rename = "集")]
    episode: i32,
    #[tabled(rename = "字幕組")]
    group: String,
    #[tabled(rename = "過濾")]
    filtered: String,
    #[tabled(rename = "衝突")]
    conflict: String,
    #[tabled(rename = "下載狀態")]
    dl_status: String,
}

pub async fn run(client: &ApiClient, action: SeriesAction, json: bool) -> Result<()> {
    match action {
        SeriesAction::List { anime } => {
            let resp: SeriesListResponse = client.get("/series").await?;
            let mut series = resp.series;
            if let Some(anime_id) = anime {
                series.retain(|s| s.anime_id == anime_id);
            }
            if json {
                return Ok(output::print_json(&series));
            }
            if series.is_empty() {
                println!("尚無系列");
                return Ok(());
            }
            let rows: Vec<SeriesRow> = series
                .iter()
                .map(|s| SeriesRow {
                    id: s.series_id,
                    anime: s.anime_title.clone(),
                    series_no: s.series_no,
                    season: s
                        .season
                        .as_ref()
                        .map(|se| format!("{} {}", se.year, se.season))
                        .unwrap_or_else(|| "-".to_string()),
                    downloaded: s.episode_downloaded,
                    found: s.episode_found,
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        SeriesAction::Show { id } => {
            let resp: AnimeSeriesRichResponse =
                client.get(&format!("/anime/series/{}", id)).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            let season_str = resp
                .season
                .as_ref()
                .map(|s| format!("{} {}", s.year, s.season))
                .unwrap_or_else(|| "-".to_string());
            let subs: Vec<String> = resp
                .subscriptions
                .iter()
                .map(|s| {
                    s.name
                        .clone()
                        .unwrap_or_else(|| format!("#{}", s.subscription_id))
                })
                .collect();
            output::print_kv(
                &format!("系列 #{}", id),
                &[
                    ("ID", resp.series_id.to_string()),
                    ("動畫", resp.anime_title.clone()),
                    ("季號", format!("S{}", resp.series_no)),
                    ("播出季", season_str),
                    ("已下載", resp.episode_downloaded.to_string()),
                    ("已找到", resp.episode_found.to_string()),
                    ("說明", output::opt_str(&resp.description)),
                    ("開播", output::opt_str(&resp.aired_date)),
                    ("完結", output::opt_str(&resp.end_date)),
                    ("訂閱", subs.join(", ")),
                ],
            );
        }

        SeriesAction::Update { id, description, aired_date, end_date, season_id } => {
            let req = UpdateSeriesRequest {
                season_id,
                description,
                aired_date,
                end_date,
            };
            let resp: serde_json::Value =
                client.put(&format!("/anime/series/{}", id), &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("系列 #{} 已更新", id));
        }

        SeriesAction::Links { id } => {
            let resp: LinksResponse = client.get(&format!("/links/{}", id)).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            if resp.links.is_empty() {
                println!("尚無集數連結");
                return Ok(());
            }
            let rows: Vec<LinkRow> = resp
                .links
                .iter()
                .map(|l| LinkRow {
                    link_id: l.link_id,
                    episode: l.episode_no,
                    group: l
                        .group_name
                        .clone()
                        .unwrap_or_else(|| format!("#{}", l.group_id.unwrap_or(0))),
                    filtered: output::format_bool(l.filtered_flag),
                    conflict: if l.conflict_flag {
                        output::format_bool(true)
                    } else {
                        "-".to_string()
                    },
                    dl_status: l
                        .download
                        .as_ref()
                        .map(|d| output::format_status(&d.status))
                        .unwrap_or_else(|| "-".to_string()),
                })
                .collect();
            println!("{}", Table::new(rows));
        }
    }
    Ok(())
}
```

---

## Task 11: 實作 commands/raw_item.rs

**Files:**
- Modify: `cli/src/commands/raw_item.rs`

```rust
// cli/src/commands/raw_item.rs
use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum RawItemAction {
    /// 列出 Raw Items
    #[command(about = "列出 RSS 抓取記錄（可依狀態篩選）")]
    List {
        /// 狀態篩選: pending|parsed|no_match|failed|skipped
        #[arg(long, short = 's')]
        status: Option<String>,
        /// 訂閱 ID 篩選
        #[arg(long)]
        sub: Option<i64>,
        /// 返回筆數（預設 50）
        #[arg(long, default_value = "50")]
        limit: i64,
        /// 偏移量（預設 0）
        #[arg(long, default_value = "0")]
        offset: i64,
    },

    /// 顯示 Raw Item 詳情
    #[command(about = "顯示單一 Raw Item 詳情")]
    Show {
        /// Item ID
        id: i64,
    },

    /// 重新解析
    #[command(about = "重新解析指定 Raw Item")]
    Reparse {
        /// Item ID
        id: i64,
    },

    /// 標記跳過
    #[command(about = "標記 Raw Item 為跳過")]
    Skip {
        /// Item ID
        id: i64,
    },
}

#[derive(Tabled)]
struct RawItemRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "標題")]
    title: String,
    #[tabled(rename = "狀態")]
    status: String,
    #[tabled(rename = "解析標題")]
    parsed_title: String,
    #[tabled(rename = "集數")]
    episode: String,
    #[tabled(rename = "過濾")]
    filtered: String,
    #[tabled(rename = "訂閱 ID")]
    sub_id: i64,
}

pub async fn run(client: &ApiClient, action: RawItemAction, json: bool) -> Result<()> {
    match action {
        RawItemAction::List { status, sub, limit, offset } => {
            let mut params = format!("?limit={}&offset={}", limit, offset);
            if let Some(s) = &status {
                params.push_str(&format!("&status={}", s));
            }
            if let Some(sid) = sub {
                params.push_str(&format!("&subscription_id={}", sid));
            }
            let resp: RawItemsResponse =
                client.get(&format!("/raw-items{}", params)).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            println!("共 {} 筆，顯示 {}-{}", resp.total, offset, offset + limit.min(resp.items.len() as i64));
            if resp.items.is_empty() {
                println!("（無記錄）");
                return Ok(());
            }
            let rows: Vec<RawItemRow> = resp
                .items
                .iter()
                .map(|item| RawItemRow {
                    id: item.item_id,
                    title: if item.title.len() > 40 {
                        format!("{}...", &item.title[..40])
                    } else {
                        item.title.clone()
                    },
                    status: output::format_status(&item.status),
                    parsed_title: item
                        .parsed_title
                        .as_deref()
                        .map(|t| if t.len() > 30 { format!("{}...", &t[..30]) } else { t.to_string() })
                        .unwrap_or_else(|| "-".to_string()),
                    episode: item
                        .parsed_episode_no
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    filtered: item
                        .filtered_flag
                        .map(|f| output::format_bool(f))
                        .unwrap_or_else(|| "-".to_string()),
                    sub_id: item.subscription_id,
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        RawItemAction::Show { id } => {
            let item: RawItemResponse = client.get(&format!("/raw-items/{}", id)).await?;
            if json {
                return Ok(output::print_json(&item));
            }
            output::print_kv(
                &format!("Raw Item #{}", id),
                &[
                    ("ID", item.item_id.to_string()),
                    ("標題", item.title.clone()),
                    ("下載 URL", item.download_url.clone()),
                    ("狀態", output::format_status(&item.status)),
                    ("解析標題", output::opt_str(&item.parsed_title)),
                    ("集數", item.parsed_episode_no.map(|e| e.to_string()).unwrap_or_else(|| "-".to_string())),
                    ("過濾", item.filtered_flag.map(|f| f.to_string()).unwrap_or_else(|| "-".to_string())),
                    ("訂閱 ID", item.subscription_id.to_string()),
                    ("Parser ID", item.parser_id.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string())),
                    ("建立時間", item.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
                ],
            );
        }

        RawItemAction::Reparse { id } => {
            let resp: serde_json::Value =
                client.post_no_body(&format!("/raw-items/{}/reparse", id)).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("Raw Item #{} 已重新解析", id));
        }

        RawItemAction::Skip { id } => {
            let resp: serde_json::Value =
                client.post_no_body(&format!("/raw-items/{}/skip", id)).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("Raw Item #{} 已標記跳過", id));
        }
    }
    Ok(())
}
```

---

## Task 12: 實作 commands/conflict.rs

**Files:**
- Modify: `cli/src/commands/conflict.rs`

```rust
// cli/src/commands/conflict.rs
use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum ConflictAction {
    /// 列出所有衝突
    #[command(about = "列出所有訂閱衝突與 Link 衝突")]
    List,

    /// 解決訂閱衝突
    #[command(about = "解決訂閱衝突，指定處理的 Fetcher")]
    Resolve {
        /// 衝突 ID
        id: i64,
        /// 指定 Fetcher ID
        #[arg(long)]
        fetcher: i64,
    },

    /// 解決 Link 衝突
    #[command(about = "解決 Link 衝突，選擇保留的 Link")]
    ResolveLink {
        /// 衝突 ID
        id: i64,
        /// 選擇保留的 Link ID
        #[arg(long)]
        link: i64,
    },
}

#[derive(Tabled)]
struct ConflictRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "類型")]
    kind: String,
    #[tabled(rename = "說明")]
    description: String,
    #[tabled(rename = "候選 Fetcher / Link")]
    candidates: String,
    #[tabled(rename = "建立時間")]
    created_at: String,
}

pub async fn run(client: &ApiClient, action: ConflictAction, json: bool) -> Result<()> {
    match action {
        ConflictAction::List => {
            let conflicts: ConflictsResponse = client.get("/conflicts").await?;
            let link_conflicts: LinkConflictsResponse = client.get("/link-conflicts").await?;

            if json {
                let combined = serde_json::json!({
                    "subscription_conflicts": conflicts.conflicts,
                    "link_conflicts": link_conflicts.conflicts,
                });
                return Ok(output::print_json(&combined));
            }

            let mut rows: Vec<ConflictRow> = Vec::new();

            for c in &conflicts.conflicts {
                let url = c.rss_url.as_deref()
                    .or(c.source_url.as_deref())
                    .unwrap_or("-");
                let fetchers: Vec<String> = c
                    .candidate_fetchers
                    .iter()
                    .map(|f| format!("{}({})", f.fetcher_name, f.fetcher_id))
                    .collect();
                rows.push(ConflictRow {
                    id: c.conflict_id,
                    kind: "訂閱衝突".to_string(),
                    description: if url.len() > 50 { format!("{}...", &url[..50]) } else { url.to_string() },
                    candidates: fetchers.join(", "),
                    created_at: c.created_at.format("%Y-%m-%d %H:%M").to_string(),
                });
            }

            for lc in &link_conflicts.conflicts {
                let links: Vec<String> = lc
                    .conflicting_links
                    .iter()
                    .map(|l| format!("Link#{}", l.link_id))
                    .collect();
                rows.push(ConflictRow {
                    id: lc.conflict_id,
                    kind: "Link 衝突".to_string(),
                    description: format!("系列#{} 第{}集", lc.series_id, lc.episode_no),
                    candidates: links.join(", "),
                    created_at: lc.created_at.format("%Y-%m-%d %H:%M").to_string(),
                });
            }

            if rows.is_empty() {
                output::print_success("目前無衝突");
                return Ok(());
            }
            println!("{}", Table::new(rows));
        }

        ConflictAction::Resolve { id, fetcher } => {
            let req = ResolveConflictRequest { fetcher_id: fetcher };
            let resp: serde_json::Value =
                client.post(&format!("/conflicts/{}/resolve", id), &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("衝突 #{} 已解決（Fetcher: {}）", id, fetcher));
        }

        ConflictAction::ResolveLink { id, link } => {
            let req = ResolveLinkConflictRequest { chosen_link_id: link };
            let resp: serde_json::Value =
                client.post(&format!("/link-conflicts/{}/resolve", id), &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("Link 衝突 #{} 已解決（保留 Link: {}）", id, link));
        }
    }
    Ok(())
}
```

---

## Task 13: 實作 commands/download.rs

**Files:**
- Modify: `cli/src/commands/download.rs`

```rust
// cli/src/commands/download.rs
use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum DownloadAction {
    /// 列出下載記錄
    #[command(about = "列出下載記錄（可依狀態篩選）")]
    List {
        /// 狀態篩選: downloading|completed|failed|paused
        #[arg(long, short = 's')]
        status: Option<String>,
        /// 返回筆數（預設 50）
        #[arg(long, default_value = "50")]
        limit: i64,
        /// 偏移量（預設 0）
        #[arg(long, default_value = "0")]
        offset: i64,
    },
}

#[derive(Tabled)]
struct DownloadRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Link ID")]
    link_id: String,
    #[tabled(rename = "狀態")]
    status: String,
    #[tabled(rename = "進度")]
    progress: String,
    #[tabled(rename = "路徑")]
    path: String,
    #[tabled(rename = "建立時間")]
    created_at: String,
}

pub async fn run(client: &ApiClient, action: DownloadAction, json: bool) -> Result<()> {
    match action {
        DownloadAction::List { status, limit, offset } => {
            let mut params = format!("?limit={}&offset={}", limit, offset);
            if let Some(s) = &status {
                params.push_str(&format!("&status={}", s));
            }
            let resp: DownloadsResponse =
                client.get(&format!("/downloads{}", params)).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            if resp.downloads.is_empty() {
                println!("尚無下載記錄");
                return Ok(());
            }
            let rows: Vec<DownloadRow> = resp
                .downloads
                .iter()
                .map(|d| DownloadRow {
                    id: d.download_id,
                    link_id: d
                        .link_id
                        .map(|l| l.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    status: output::format_status(&d.status),
                    progress: d
                        .progress
                        .map(|p| format!("{:.1}%", p * 100.0))
                        .unwrap_or_else(|| "-".to_string()),
                    path: d
                        .file_path
                        .as_deref()
                        .map(|p| {
                            if p.len() > 40 {
                                format!("...{}", &p[p.len() - 40..])
                            } else {
                                p.to_string()
                            }
                        })
                        .unwrap_or_else(|| "-".to_string()),
                    created_at: d.created_at.format("%Y-%m-%d %H:%M").to_string(),
                })
                .collect();
            println!("{}", Table::new(rows));
        }
    }
    Ok(())
}
```

---

## Task 14: 實作 commands/filter.rs

**Files:**
- Modify: `cli/src/commands/filter.rs`

```rust
// cli/src/commands/filter.rs
use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum FilterAction {
    /// 列出過濾規則
    #[command(about = "列出過濾規則（可依類型/目標篩選）")]
    List {
        /// 目標類型: global|anime|series|group|fetcher
        #[arg(long, short = 't')]
        r#type: Option<String>,
        /// 目標 ID（非 global 時使用）
        #[arg(long)]
        target: Option<i64>,
    },

    /// 新增過濾規則
    #[command(about = "新增過濾規則")]
    Add {
        /// 目標類型: global|anime|series|group|fetcher
        #[arg(long, short = 't')]
        r#type: String,
        /// 目標 ID
        #[arg(long)]
        target: Option<i64>,
        /// 正規式
        #[arg(long, short = 'r')]
        regex: String,
        /// 排序（預設 1）
        #[arg(long, default_value = "1")]
        order: i32,
        /// 設為負向規則（過濾掉）
        #[arg(long)]
        negative: bool,
    },

    /// 刪除過濾規則
    #[command(about = "刪除過濾規則")]
    Delete {
        /// 規則 ID
        id: i64,
    },

    /// 預覽過濾效果
    #[command(about = "預覽規則對現有資料的篩選效果")]
    Preview {
        /// 目標類型: global|anime|series|group|fetcher
        #[arg(long, short = 't')]
        r#type: Option<String>,
        /// 目標 ID
        #[arg(long)]
        target: Option<i64>,
        /// 正規式
        #[arg(long, short = 'r')]
        regex: String,
        /// 設為負向規則
        #[arg(long)]
        negative: bool,
        /// 排序（預設 1）
        #[arg(long, default_value = "1")]
        order: i32,
    },
}

#[derive(Tabled)]
struct FilterRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "目標類型")]
    target_type: String,
    #[tabled(rename = "目標 ID")]
    target_id: String,
    #[tabled(rename = "排序")]
    order: i32,
    #[tabled(rename = "方向")]
    direction: String,
    #[tabled(rename = "正規式")]
    regex: String,
}

pub async fn run(client: &ApiClient, action: FilterAction, json: bool) -> Result<()> {
    match action {
        FilterAction::List { r#type, target } => {
            let mut params = String::new();
            let mut sep = "?";
            if let Some(t) = &r#type {
                params.push_str(&format!("{}target_type={}", sep, t));
                sep = "&";
            }
            if let Some(id) = target {
                params.push_str(&format!("{}target_id={}", sep, id));
            }
            let resp: FiltersResponse =
                client.get(&format!("/filters{}", params)).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            if resp.rules.is_empty() {
                println!("尚無過濾規則");
                return Ok(());
            }
            let rows: Vec<FilterRow> = resp
                .rules
                .iter()
                .map(|r| FilterRow {
                    id: r.rule_id,
                    target_type: r.target_type.clone(),
                    target_id: r.target_id.map(|i| i.to_string()).unwrap_or_else(|| "-".to_string()),
                    order: r.rule_order,
                    direction: if r.is_positive {
                        output::format_status("active")
                    } else {
                        output::format_status("failed")
                    },
                    regex: r.regex_pattern.clone(),
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        FilterAction::Add { r#type, target, regex, order, negative } => {
            let req = CreateFilterRuleRequest {
                target_type: r#type,
                target_id: target,
                rule_order: order,
                is_positive: !negative,
                regex_pattern: regex,
            };
            let resp: FilterRuleResponse = client.post("/filters", &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("過濾規則已建立 (ID: {})", resp.rule_id));
        }

        FilterAction::Delete { id } => {
            client.delete(&format!("/filters/{}", id)).await?;
            if json {
                return Ok(output::print_json(&serde_json::json!({"deleted": id})));
            }
            output::print_success(&format!("過濾規則 #{} 已刪除", id));
        }

        FilterAction::Preview { r#type, target, regex, negative, order } => {
            let req = CreateFilterRuleRequest {
                target_type: r#type.unwrap_or_else(|| "global".to_string()),
                target_id: target,
                rule_order: order,
                is_positive: !negative,
                regex_pattern: regex,
            };
            let resp: serde_json::Value = client.post("/filters/preview", &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }
    Ok(())
}
```

---

## Task 15: 實作 commands/parser.rs

**Files:**
- Modify: `cli/src/commands/parser.rs`

```rust
// cli/src/commands/parser.rs
use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum ParserAction {
    /// 列出解析器
    #[command(about = "列出所有解析器")]
    List {
        #[arg(long)]
        r#type: Option<String>,
        #[arg(long)]
        target: Option<i64>,
    },

    /// 顯示解析器詳情
    #[command(about = "顯示解析器詳情")]
    Show {
        id: i64,
    },

    /// 新增解析器
    #[command(about = "新增標題解析器")]
    Add {
        /// 解析器名稱
        #[arg(long, short = 'n')]
        name: String,
        /// 優先度（數字越小越優先）
        #[arg(long, default_value = "10")]
        priority: i32,
        /// 條件正規式（符合才套用此解析器）
        #[arg(long)]
        condition: Option<String>,
        /// 解析正規式
        #[arg(long)]
        parse_regex: Option<String>,
        /// 停用
        #[arg(long)]
        disabled: bool,
        /// 建立來源類型（global|anime|series|group）
        #[arg(long, default_value = "global")]
        from_type: String,
        /// 建立來源 ID
        #[arg(long)]
        from_id: Option<i64>,
    },

    /// 更新解析器
    #[command(about = "更新解析器設定")]
    Update {
        id: i64,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        priority: Option<i32>,
        #[arg(long)]
        condition: Option<String>,
        #[arg(long)]
        parse_regex: Option<String>,
        #[arg(long, conflicts_with = "disable")]
        enable: bool,
        #[arg(long, conflicts_with = "enable")]
        disable: bool,
    },

    /// 刪除解析器
    #[command(about = "刪除解析器")]
    Delete {
        id: i64,
    },

    /// 預覽解析效果
    #[command(about = "預覽解析器對現有 Raw Items 的效果")]
    Preview {
        /// 使用現有解析器 ID
        #[arg(long)]
        id: Option<i64>,
        /// 條件正規式
        #[arg(long)]
        condition: Option<String>,
        /// 解析正規式
        #[arg(long)]
        parse_regex: Option<String>,
    },
}

#[derive(Tabled)]
struct ParserRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "名稱")]
    name: String,
    #[tabled(rename = "優先度")]
    priority: i32,
    #[tabled(rename = "條件正規式")]
    condition: String,
    #[tabled(rename = "啟用")]
    enabled: String,
    #[tabled(rename = "來源")]
    from: String,
}

pub async fn run(client: &ApiClient, action: ParserAction, json: bool) -> Result<()> {
    match action {
        ParserAction::List { r#type, target } => {
            let mut params = String::new();
            let mut sep = "?";
            if let Some(t) = &r#type {
                params.push_str(&format!("{}created_from_type={}", sep, t));
                sep = "&";
            }
            if let Some(id) = target {
                params.push_str(&format!("{}created_from_id={}", sep, id));
            }
            let resp: ParsersResponse =
                client.get(&format!("/parsers{}", params)).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            if resp.parsers.is_empty() {
                println!("尚無解析器");
                return Ok(());
            }
            let rows: Vec<ParserRow> = resp
                .parsers
                .iter()
                .map(|p| ParserRow {
                    id: p.parser_id,
                    name: p.name.clone(),
                    priority: p.priority,
                    condition: p.condition_regex.clone().unwrap_or_else(|| "-".to_string()),
                    enabled: output::format_bool(p.enabled),
                    from: match &p.created_from_type {
                        Some(t) => {
                            if let Some(fid) = p.created_from_id {
                                format!("{}#{}", t, fid)
                            } else {
                                t.clone()
                            }
                        }
                        None => "global".to_string(),
                    },
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        ParserAction::Show { id } => {
            let parser: ParserResponse = client.get(&format!("/parsers/{}", id)).await?;
            if json {
                return Ok(output::print_json(&parser));
            }
            output::print_kv(
                &format!("解析器 #{}", id),
                &[
                    ("ID", parser.parser_id.to_string()),
                    ("名稱", parser.name.clone()),
                    ("優先度", parser.priority.to_string()),
                    ("條件正規式", output::opt_str(&parser.condition_regex)),
                    ("啟用", parser.enabled.to_string()),
                    ("來源類型", output::opt_str(&parser.created_from_type)),
                    ("來源 ID", output::opt_i64(parser.created_from_id)),
                    ("建立時間", parser.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
                ],
            );
        }

        ParserAction::Add { name, priority, condition, parse_regex, disabled, from_type, from_id } => {
            let req = CreateParserRequest {
                name,
                priority: Some(priority),
                condition_regex: condition,
                parse_regex,
                enabled: Some(!disabled),
                created_from_type: Some(from_type),
                created_from_id: from_id,
            };
            let resp: ParserResponse = client.post("/parsers", &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("解析器已建立: {} (ID: {})", resp.name, resp.parser_id));
        }

        ParserAction::Update { id, name, priority, condition, parse_regex, enable, disable } => {
            let enabled = if enable { Some(true) } else if disable { Some(false) } else { None };
            let req = UpdateParserRequest {
                name,
                priority,
                condition_regex: condition,
                parse_regex,
                enabled,
            };
            let resp: ParserResponse = client.put(&format!("/parsers/{}", id), &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("解析器 #{} 已更新", id));
        }

        ParserAction::Delete { id } => {
            client.delete(&format!("/parsers/{}", id)).await?;
            if json {
                return Ok(output::print_json(&serde_json::json!({"deleted": id})));
            }
            output::print_success(&format!("解析器 #{} 已刪除", id));
        }

        ParserAction::Preview { id, condition, parse_regex } => {
            let body = serde_json::json!({
                "parser_id": id,
                "condition_regex": condition,
                "parse_regex": parse_regex,
            });
            let resp: serde_json::Value = client.post("/parsers/preview", &body).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }
    Ok(())
}
```

---

## Task 16: 實作 commands/subtitle_group.rs 和 commands/qb_config.rs

**Files:**
- Modify: `cli/src/commands/subtitle_group.rs`
- Modify: `cli/src/commands/qb_config.rs`

**subtitle_group.rs:**

```rust
// cli/src/commands/subtitle_group.rs
use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum SubtitleGroupAction {
    /// 列出所有字幕組
    #[command(about = "列出所有字幕組")]
    List,

    /// 新增字幕組
    #[command(about = "新增字幕組")]
    Add {
        /// 字幕組名稱
        name: String,
    },

    /// 刪除字幕組
    #[command(about = "刪除字幕組")]
    Delete {
        /// 字幕組 ID
        id: i64,
    },
}

#[derive(Tabled)]
struct GroupRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "名稱")]
    name: String,
    #[tabled(rename = "建立時間")]
    created_at: String,
}

pub async fn run(client: &ApiClient, action: SubtitleGroupAction, json: bool) -> Result<()> {
    match action {
        SubtitleGroupAction::List => {
            let resp: SubtitleGroupsResponse = client.get("/subtitle-groups").await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            if resp.groups.is_empty() {
                println!("尚無字幕組");
                return Ok(());
            }
            let rows: Vec<GroupRow> = resp
                .groups
                .iter()
                .map(|g| GroupRow {
                    id: g.group_id,
                    name: g.group_name.clone(),
                    created_at: g.created_at.format("%Y-%m-%d").to_string(),
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        SubtitleGroupAction::Add { name } => {
            let req = CreateSubtitleGroupRequest { group_name: name.clone() };
            let resp: SubtitleGroupResponse = client.post("/subtitle-groups", &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success(&format!("字幕組已建立: {} (ID: {})", resp.group_name, resp.group_id));
        }

        SubtitleGroupAction::Delete { id } => {
            client.delete(&format!("/subtitle-groups/{}", id)).await?;
            if json {
                return Ok(output::print_json(&serde_json::json!({"deleted": id})));
            }
            output::print_success(&format!("字幕組 #{} 已刪除", id));
        }
    }
    Ok(())
}
```

**qb_config.rs:**

```rust
// cli/src/commands/qb_config.rs
use crate::client::ApiClient;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use serde::Serialize;

#[derive(Subcommand)]
pub enum QbConfigAction {
    /// 設定 qBittorrent 帳密
    #[command(about = "設定 qBittorrent WebUI 帳號與密碼")]
    SetCredentials {
        /// qBittorrent WebUI 帳號
        #[arg(long, short = 'u')]
        user: String,
        /// qBittorrent WebUI 密碼
        #[arg(long, short = 'p')]
        password: String,
        /// Downloader Service URL（或設定環境變數 BANGUMI_DOWNLOADER_URL）
        #[arg(
            long,
            env = "BANGUMI_DOWNLOADER_URL",
            default_value = "http://localhost:8002"
        )]
        downloader_url: String,
    },
}

#[derive(Serialize)]
struct CredentialsRequest<'a> {
    username: &'a str,
    password: &'a str,
}

pub async fn run(client: &ApiClient, action: QbConfigAction, json: bool) -> Result<()> {
    match action {
        QbConfigAction::SetCredentials { user, password, downloader_url } => {
            // 使用 downloader_url 而非 api_url，建立獨立的 client
            let dl_client = ApiClient::new(downloader_url.clone());
            let req = CredentialsRequest {
                username: &user,
                password: &password,
            };
            let resp: serde_json::Value = dl_client.post("/config/credentials", &req).await?;
            if json {
                return Ok(output::print_json(&resp));
            }
            output::print_success("qBittorrent 帳密已設定");
            println!("  帳號: {}", user);
            println!("  Downloader URL: {}", downloader_url);
        }
    }
    Ok(())
}
```

---

## Task 17: 最終編譯修正與驗證

**Step 1: 完整編譯**

```bash
cd /workspace && cargo build -p bangumi-cli 2>&1
```

**Step 2: 修正所有編譯錯誤**

常見問題與解法：
- `todo!()` 殘留 → 確認所有命令模組都已完整實作
- `models.rs` 欄位名稱不符 → 根據錯誤訊息調整欄位名稱
- `tabled::Tabled` derive 問題 → 確認每個 `Tabled` struct 的欄位都是 `String` 或 `impl Display`
- `output::format_status` 回傳的是 `String`，直接放入 tabled struct 欄位即可
- `serde_json::Value` 作為 post body → 需要 `client.post::<serde_json::Value, serde_json::Value>`

**Step 3: 測試 Help 輸出**

```bash
./target/debug/bangumi --help
./target/debug/bangumi subscription --help
./target/debug/bangumi subscription add --help
./target/debug/bangumi raw-item --help
./target/debug/bangumi raw --help           # 別名確認
./target/debug/bangumi sub --help           # 別名確認
./target/debug/bangumi st --help            # 別名確認
```

預期：所有指令都有完整的說明文字和參數列表。

**Step 4: 確認 clap 別名運作**

```bash
./target/debug/bangumi sub list --help
./target/debug/bangumi raw list --help
./target/debug/bangumi dl list --help
./target/debug/bangumi sg list --help
```

**Step 5: Commit**

```bash
git add cli/src/
git commit -m "feat(cli): complete rewrite of bangumi CLI with full API coverage

- Resource-oriented command structure with short aliases (sub, raw, dl, sg, st)
- All commands: status, subscription, anime, series, raw-item, conflict,
  download, filter, parser, subtitle-group, qb-config
- --json flag for machine-readable output
- BANGUMI_API_URL and BANGUMI_DOWNLOADER_URL env var support
- tabled for table output, colored for status colorization
- Proper PATCH/PUT HTTP methods in ApiClient"
```

---

## 注意事項（給實作者）

1. **models.rs 的欄位名稱**：後端 API 回傳的 JSON 欄位名稱可能與設計文件略有出入。如果 `serde_json` 反序列化失敗，請用 `--json` 先查看原始 API 回應，再調整 struct 欄位名稱。

2. **`tabled` 的 `Tabled` derive**：所有放入表格的欄位需要 `impl Display`。`String` 型別直接可用，數字可以先 `.to_string()` 轉換，或使用 `#[tabled(display_with = "...")]` attribute。

3. **API 回應格式不確定的地方**：`links/:series_id`、`/anime/:id/series`、`/parsers/preview` 等，設計文件標注了使用 `serde_json::Value` 做寬鬆解析。正式使用後可再強型別化。

4. **cli/src/tests.rs 若存在**：直接刪除，因為舊測試依賴不存在的 API。

5. **`post_no_body` 的 reparse/skip 回應**：後端可能回傳不同格式，若 JSON 解析失敗改用 `post_no_body_ignore_response`。

6. **確認 `cli/src/commands.rs` 已刪除**：重寫後應只有 `cli/src/commands/` 目錄，不應有 `commands.rs` 檔案。
