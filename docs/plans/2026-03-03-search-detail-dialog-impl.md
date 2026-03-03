# Search Detail Dialog Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 搜尋結果卡片點擊後開啟 DetailDialog，向 fetcher 進階查詢並以表格列出可訂閱的 RSS，支援點擊 RSS URL 開新視窗與一鍵預填訂閱。

**Architecture:** `detail_key` 是 fetcher 自訂的不透明字串（`"bangumi:3822"` 或 `"source:searchstr"`），由 fetcher 在 `/search` 時發出，前端收到後原封不動傳回給 Core `/detail` endpoint，Core 再轉發給對應 fetcher 的 `/detail` endpoint 處理。

**Tech Stack:** Rust (axum, scraper, reqwest), TypeScript/React (Effect-ts, shadcn/ui)

---

## Task 1: Update Shared Models

**Files:**
- Modify: `shared/src/models.rs:424-461`

### Step 1: Replace search models

Replace the entire `// ============ Search ============` section (lines 424–461):

```rust
// ============ Search ============

/// Core → Fetcher: search request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
}

/// Fetcher → Core: a single search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub thumbnail_url: Option<String>,
    /// Opaque key for the fetcher's /detail endpoint.
    /// e.g. "bangumi:3822" or "source:[KITA]...金牌"
    pub detail_key: String,
}

/// Fetcher → Core: search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

/// Core → Frontend: merged result with source attribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedSearchResult {
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub detail_key: String,
    pub source: String,
}

/// Core → Frontend: final search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedSearchResponse {
    pub results: Vec<AggregatedSearchResult>,
}

/// Frontend → Core → Fetcher: detail request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailRequest {
    pub detail_key: String,
}

/// One row in the detail table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailItem {
    pub subgroup_name: String,
    pub rss_url: String,
}

/// Fetcher → Core → Frontend: detail response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailResponse {
    pub items: Vec<DetailItem>,
}
```

### Step 2: Add `detail_endpoint` to Capabilities

Find `pub struct Capabilities` and add the field:

```rust
pub struct Capabilities {
    pub fetch_endpoint: Option<String>,
    pub search_endpoint: Option<String>,
    pub detail_endpoint: Option<String>,   // ← add this line
    pub download_endpoint: Option<String>,
    pub sync_endpoint: Option<String>,
    pub supported_download_types: Vec<String>,
}
```

### Step 3: Verify it compiles

```bash
cargo check -p shared
```
Expected: no errors (all consumers of old `SearchResult` will fail — fix in later tasks).

### Step 4: Commit

```bash
git add shared/src/models.rs
git commit -m "feat(shared): add detail_key to SearchResult, add DetailRequest/Response/Item"
```

---

## Task 2: Update Mikanani Search Scraper

**Files:**
- Modify: `fetchers/mikanani/src/search_scraper.rs`

The scraper must now:
1. Bangumi results (`/Home/Bangumi/`): `detail_key = "bangumi:{id}"`
2. Magnet/episode results (`/Home/Episode/`): parse title, truncate at last `_`, `detail_key = "source:{searchstr}"`

### Step 1: Write failing test for magnet source parsing

Add to the `#[cfg(test)]` block in `search_scraper.rs`:

```rust
#[test]
fn test_parse_magnet_source_card() {
    let html = r#"
        <html><body>
          <div class="an-ul">
            <a class="an-info-group" href="/Home/Episode/abc123hash">
              <div class="an-img-cell">
                <img src="/images/Bangumi/3822/cover.jpg" />
              </div>
              <div class="an-info">
                <p class="an-text">[KITA]（双语人工翻译）金牌得主19_Ciallo</p>
              </div>
            </a>
          </div>
        </body></html>
    "#;

    let results = parse_search_results(html).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "[KITA]（双语人工翻译）金牌得主19_Ciallo");
    assert_eq!(
        results[0].detail_key,
        "source:[KITA]（双语人工翻译）金牌得主19"
    );
}

#[test]
fn test_parse_magnet_source_no_underscore_uses_full_title() {
    let html = r#"
        <html><body>
          <a class="an-info-group" href="/Home/Episode/xyz">
            <img src="/img/cover.jpg" />
            <p class="an-text">SomeTitle NoUnderscore</p>
          </a>
        </body></html>
    "#;

    let results = parse_search_results(html).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].detail_key, "source:SomeTitle NoUnderscore");
}
```

### Step 2: Run failing tests

```bash
cargo test -p fetcher-mikanani -- search_scraper 2>&1 | head -40
```
Expected: compile error (old `SearchResult` shape) + test failures.

### Step 3: Rewrite `parse_search_results`

Replace the entire function (and update `SearchResult` usage):

```rust
pub fn parse_search_results(html: &str) -> Result<Vec<SearchResult>, String> {
    let document = Html::parse_document(html);

    let item_sel = Selector::parse("a.an-info-group")
        .map_err(|e| format!("Invalid CSS selector: {:?}", e))?;
    let title_sel = Selector::parse("p.an-text")
        .map_err(|e| format!("Invalid CSS selector: {:?}", e))?;
    let img_sel = Selector::parse("img")
        .map_err(|e| format!("Invalid CSS selector: {:?}", e))?;

    let mut results = Vec::new();

    for element in document.select(&item_sel) {
        let href = match element.value().attr("href") {
            Some(h) => h,
            None => continue,
        };

        let title = element
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        let thumbnail_url = element
            .select(&img_sel)
            .next()
            .and_then(|el| el.value().attr("src"))
            .map(|src| {
                if src.starts_with("http") {
                    src.to_string()
                } else {
                    format!("https://mikanani.me{}", src)
                }
            });

        let detail_key = if href.contains("/Home/Bangumi/") {
            let bangumi_id: u32 = match href.rsplit('/').next().and_then(|s| s.parse().ok()) {
                Some(id) => id,
                None => {
                    tracing::warn!("Could not parse bangumi ID from href: {}", href);
                    continue;
                }
            };
            format!("bangumi:{}", bangumi_id)
        } else if href.contains("/Home/Episode/") {
            // Truncate title at the last '_' to get the searchstr
            let searchstr = match title.rfind('_') {
                Some(idx) => &title[..idx],
                None => &title,
            };
            format!("source:{}", searchstr)
        } else {
            continue; // Skip unknown link types
        };

        results.push(SearchResult {
            title,
            thumbnail_url,
            detail_key,
        });
    }

    tracing::info!(
        "Mikanani search parsed {} results from {} HTML bytes",
        results.len(),
        html.len()
    );

    Ok(results)
}
```

### Step 4: Update existing tests

In the existing `test_parse_bangumi_card` test, replace `subscription_url` assertion:
```rust
// Old:
assert_eq!(results[0].subscription_url, "https://mikanani.me/RSS/Bangumi?bangumiId=3310");
// New:
assert_eq!(results[0].detail_key, "bangumi:3310");
```

Remove `description: None` from any test assertions (field no longer exists).

Update `MockSearchScraper` — it already uses `Vec<SearchResult>`, no structural change needed, but test fixtures must use new struct fields.

### Step 5: Run tests

```bash
cargo test -p fetcher-mikanani -- search_scraper 2>&1
```
Expected: all tests pass.

### Step 6: Commit

```bash
git add fetchers/mikanani/src/search_scraper.rs
git commit -m "feat(mikanani): add magnet source parsing with detail_key to search scraper"
```

---

## Task 3: Create Mikanani Detail Scraper

**Files:**
- Create: `fetchers/mikanani/src/detail_scraper.rs`
- Modify: `fetchers/mikanani/src/lib.rs`

The detail scraper handles two `detail_key` prefixes:
- `bangumi:{id}` → scrape `https://mikanani.me/Home/Bangumi/{id}` for subgroup list
- `source:{searchstr}` → re-search with searchstr, group results by subgroup

### Step 1: Inspect bangumi page HTML (manual step)

Before writing code, open a browser and inspect `https://mikanani.me/Home/Bangumi/3822` to find:
- The CSS selector for each subgroup row/button
- Where the `subgroupid` is stored (e.g., in a `data-*` attribute or in an `href`)
- The subgroup name text element

Typical mikanani bangumi page structure to look for:
```html
<!-- Look for elements like these: -->
<div class="tag-subgroup-name" data-subgroup-id="202">花山映画</div>
<!-- or anchor tags like: -->
<a href="/RSS/Bangumi?bangumiId=3822&subgroupid=202">花山映画</a>
```

**Note:** The selectors in Step 3 below use `a[href*="subgroupid"]` as a generic approach that looks for RSS links containing `subgroupid` — verify this works against the actual page.

### Step 2: Write failing tests

Create `fetchers/mikanani/src/detail_scraper.rs` with tests first:

```rust
use async_trait::async_trait;
use scraper::{Html, Selector};
use shared::{DetailItem, DetailResponse};

#[async_trait]
pub trait DetailScraper: Send + Sync {
    async fn scrape(&self, detail_key: &str) -> Result<DetailResponse, String>;
}

pub struct RealDetailScraper {
    client: reqwest::Client,
}

impl RealDetailScraper {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .user_agent("Mozilla/5.0 (compatible; bangumi-bot/1.0)")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

impl Default for RealDetailScraper {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DetailScraper for RealDetailScraper {
    async fn scrape(&self, detail_key: &str) -> Result<DetailResponse, String> {
        if let Some(bangumi_id) = detail_key.strip_prefix("bangumi:") {
            scrape_bangumi(&self.client, bangumi_id).await
        } else if let Some(searchstr) = detail_key.strip_prefix("source:") {
            scrape_source(&self.client, searchstr).await
        } else {
            Err(format!("Unknown detail_key prefix: {}", detail_key))
        }
    }
}

async fn scrape_bangumi(client: &reqwest::Client, bangumi_id: &str) -> Result<DetailResponse, String> {
    let url = format!("https://mikanani.me/Home/Bangumi/{}", bangumi_id);
    let html = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Read body failed: {}", e))?;

    parse_bangumi_detail(&html, bangumi_id)
}

async fn scrape_source(client: &reqwest::Client, searchstr: &str) -> Result<DetailResponse, String> {
    let html = client
        .get("https://mikanani.me/Home/Search")
        .query(&[("searchstr", searchstr)])
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Read body failed: {}", e))?;

    parse_source_detail(&html, searchstr)
}

/// Parse the bangumi detail page for subgroup RSS subscriptions.
/// Looks for anchor tags linking to RSS feeds with subgroupid parameter.
/// Also returns the root RSS (no subgroupid) as "全部".
pub fn parse_bangumi_detail(html: &str, bangumi_id: &str) -> Result<DetailResponse, String> {
    let document = Html::parse_document(html);

    // Look for anchor tags whose href contains both bangumiId and subgroupid
    // NOTE: Verify this selector against the real mikanani.me bangumi page HTML
    let link_sel = Selector::parse("a[href*='subgroupid']")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;

    let mut items: Vec<DetailItem> = Vec::new();
    let mut seen_subgroups = std::collections::HashSet::new();

    for element in document.select(&link_sel) {
        let href = match element.value().attr("href") {
            Some(h) => h,
            None => continue,
        };

        // Extract subgroupid from href
        let subgroup_id = href
            .split("subgroupid=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .unwrap_or("");

        if subgroup_id.is_empty() || seen_subgroups.contains(subgroup_id) {
            continue;
        }
        seen_subgroups.insert(subgroup_id.to_string());

        let subgroup_name = element.text().collect::<String>().trim().to_string();
        if subgroup_name.is_empty() {
            continue;
        }

        let rss_url = format!(
            "https://mikanani.me/RSS/Bangumi?bangumiId={}&subgroupid={}",
            bangumi_id, subgroup_id
        );

        items.push(DetailItem { subgroup_name, rss_url });
    }

    // Always add the root RSS (all subgroups) as the last item
    items.push(DetailItem {
        subgroup_name: "全部".to_string(),
        rss_url: format!("https://mikanani.me/RSS/Bangumi?bangumiId={}", bangumi_id),
    });

    Ok(DetailResponse { items })
}

/// Parse source search results and group by subgroup.
/// Each episode result title contains a subgroup in brackets: "[SubgroupName] ..."
/// Uses the title-truncated-at-last-underscore as the RSS searchstr.
pub fn parse_source_detail(html: &str, _original_searchstr: &str) -> Result<DetailResponse, String> {
    let document = Html::parse_document(html);

    let item_sel = Selector::parse("a.an-info-group")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;
    let title_sel = Selector::parse("p.an-text")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;

    let mut items: Vec<DetailItem> = Vec::new();
    let mut seen_rss: std::collections::HashSet<String> = std::collections::HashSet::new();

    for element in document.select(&item_sel) {
        let href = element.value().attr("href").unwrap_or("");
        if !href.contains("/Home/Episode/") {
            continue;
        }

        let title = element
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        // Extract subgroup name from "[SubgroupName] ..." format
        let subgroup_name = if title.starts_with('[') {
            title
                .find(']')
                .map(|i| title[1..i].trim().to_string())
                .unwrap_or_else(|| title.clone())
        } else {
            title.clone()
        };

        // Compute RSS searchstr: truncate at last '_'
        let searchstr = match title.rfind('_') {
            Some(idx) => &title[..idx],
            None => &title,
        };

        let rss_url = format!(
            "https://mikanani.me/RSS/Search?searchstr={}",
            urlencoding::encode(searchstr)
        );

        if seen_rss.contains(&rss_url) {
            continue;
        }
        seen_rss.insert(rss_url.clone());

        items.push(DetailItem { subgroup_name, rss_url });
    }

    Ok(DetailResponse { items })
}

pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockDetailScraper {
        result: Mutex<Result<DetailResponse, String>>,
    }

    impl MockDetailScraper {
        pub fn with_items(items: Vec<DetailItem>) -> Self {
            Self {
                result: Mutex::new(Ok(DetailResponse { items })),
            }
        }

        pub fn with_error(message: impl Into<String>) -> Self {
            Self {
                result: Mutex::new(Err(message.into())),
            }
        }
    }

    #[async_trait]
    impl DetailScraper for MockDetailScraper {
        async fn scrape(&self, _detail_key: &str) -> Result<DetailResponse, String> {
            self.result.lock().unwrap().clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bangumi_detail_with_subgroups() {
        // NOTE: Replace this mock HTML with real mikanani bangumi page structure
        // after inspecting the actual page HTML
        let html = r#"
            <html><body>
              <a href="/RSS/Bangumi?bangumiId=3822&subgroupid=202">花山映画</a>
              <a href="/RSS/Bangumi?bangumiId=3822&subgroupid=80">喵萌奶茶屋</a>
              <a href="/other-link">不相關連結</a>
            </body></html>
        "#;

        let result = parse_bangumi_detail(html, "3822").unwrap();
        // 2 subgroups + 1 root
        assert_eq!(result.items.len(), 3);
        assert_eq!(result.items[0].subgroup_name, "花山映画");
        assert_eq!(
            result.items[0].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3822&subgroupid=202"
        );
        assert_eq!(result.items[2].subgroup_name, "全部");
        assert_eq!(
            result.items[2].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3822"
        );
    }

    #[test]
    fn test_parse_bangumi_detail_no_subgroups_returns_root_only() {
        let html = "<html><body><p>no subgroups</p></body></html>";
        let result = parse_bangumi_detail(html, "9999").unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].subgroup_name, "全部");
    }

    #[test]
    fn test_parse_source_detail_groups_by_subgroup() {
        let html = r#"
            <html><body>
              <a class="an-info-group" href="/Home/Episode/abc">
                <p class="an-text">[KITA]金牌得主19_Ciallo</p>
              </a>
              <a class="an-info-group" href="/Home/Episode/def">
                <p class="an-text">[KITA]金牌得主18_Ciallo</p>
              </a>
              <a class="an-info-group" href="/Home/Episode/ghi">
                <p class="an-text">[SubB]金牌得主19_release</p>
              </a>
            </body></html>
        "#;

        let result = parse_source_detail(html, "[KITA]金牌").unwrap();
        // KITA appears twice (same searchstr after truncation), SubB once
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].subgroup_name, "KITA");
        assert_eq!(result.items[1].subgroup_name, "SubB");
    }

    #[test]
    fn test_parse_source_detail_title_without_brackets() {
        let html = r#"
            <html><body>
              <a class="an-info-group" href="/Home/Episode/abc">
                <p class="an-text">金牌得主19_noBrackets</p>
              </a>
            </body></html>
        "#;

        let result = parse_source_detail(html, "金牌得主19").unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].subgroup_name, "金牌得主19_noBrackets");
    }
}
```

### Step 3: Add `urlencoding` crate dependency

In `fetchers/mikanani/Cargo.toml`, add:
```toml
urlencoding = "2"
```

### Step 4: Run tests

```bash
cargo test -p fetcher-mikanani -- detail_scraper 2>&1
```
Expected: all pass.

### Step 5: Expose from lib.rs

```rust
// In fetchers/mikanani/src/lib.rs, add:
pub mod detail_scraper;
pub use detail_scraper::{DetailScraper, RealDetailScraper};
```

### Step 6: Commit

```bash
git add fetchers/mikanani/src/detail_scraper.rs fetchers/mikanani/src/lib.rs fetchers/mikanani/Cargo.toml
git commit -m "feat(mikanani): add detail scraper for bangumi and source detail keys"
```

---

## Task 4: Add Detail Handler to Mikanani Fetcher

**Files:**
- Modify: `fetchers/mikanani/src/handlers.rs`
- Modify: `fetchers/mikanani/src/main.rs`

### Step 1: Write failing test

Add to `handlers.rs` tests:

```rust
#[tokio::test]
async fn test_detail_returns_items_from_scraper() {
    use fetcher_mikanani::detail_scraper::mock::MockDetailScraper;

    let state = AppState {
        parser: Arc::new(RssParser::new()),
        http_client: Arc::new(RealHttpClient::new()),
        search_scraper: Arc::new(
            fetcher_mikanani::search_scraper::mock::MockSearchScraper::with_results(vec![])
        ),
        detail_scraper: Arc::new(MockDetailScraper::with_items(vec![
            shared::DetailItem {
                subgroup_name: "TestGroup".to_string(),
                rss_url: "https://mikanani.me/RSS/Bangumi?bangumiId=1".to_string(),
            }
        ])),
    };

    let payload = Json(shared::DetailRequest {
        detail_key: "bangumi:1".to_string(),
    });

    let (status, body) = detail(State(state), payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.items.len(), 1);
    assert_eq!(body.items[0].subgroup_name, "TestGroup");
}
```

### Step 2: Add detail_scraper to AppState

In `handlers.rs`, update imports and `AppState`:

```rust
use fetcher_mikanani::{
    FetchTask, RealHttpClient, RealSearchScraper, RealDetailScraper,
    RssParser, SearchScraper, DetailScraper,
};
use shared::{
    FetchTriggerRequest, FetchTriggerResponse, SearchRequest, SearchResponse,
    DetailRequest, DetailResponse,
};

#[derive(Clone)]
pub struct AppState {
    pub parser: Arc<RssParser>,
    pub http_client: Arc<RealHttpClient>,
    pub search_scraper: Arc<dyn SearchScraper>,
    pub detail_scraper: Arc<dyn DetailScraper>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            parser: Arc::new(RssParser::new()),
            http_client: Arc::new(RealHttpClient::new()),
            search_scraper: Arc::new(RealSearchScraper::new()),
            detail_scraper: Arc::new(RealDetailScraper::new()),
        }
    }
}
```

### Step 3: Add detail handler function

```rust
pub async fn detail(
    State(state): State<AppState>,
    Json(payload): Json<DetailRequest>,
) -> (StatusCode, Json<DetailResponse>) {
    tracing::info!("Received detail request: detail_key={:?}", payload.detail_key);

    match state.detail_scraper.scrape(&payload.detail_key).await {
        Ok(response) => {
            tracing::info!("Detail returned {} items", response.items.len());
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            tracing::error!("Detail scraping failed: {}", e);
            (StatusCode::OK, Json(DetailResponse { items: vec![] }))
        }
    }
}
```

### Step 4: Add route in main.rs

```rust
// In the router, add after /search:
.route("/detail", post(handlers::detail))
```

Also update the capabilities registration to include `detail_endpoint`:

```rust
capabilities: shared::Capabilities {
    fetch_endpoint: Some("/fetch".to_string()),
    search_endpoint: Some("/search".to_string()),
    detail_endpoint: Some("/detail".to_string()),   // ← add
    download_endpoint: None,
    sync_endpoint: None,
    supported_download_types: vec![],
},
```

### Step 5: Run tests

```bash
cargo test -p fetcher-mikanani 2>&1
```
Expected: all pass.

### Step 6: Commit

```bash
git add fetchers/mikanani/src/handlers.rs fetchers/mikanani/src/main.rs
git commit -m "feat(mikanani): add /detail endpoint"
```

---

## Task 5: Update Core Search Handler

**Files:**
- Modify: `core-service/src/handlers/search.rs`

### Step 1: Update AggregatedSearchResult mapping

The `SearchResult` no longer has `description` or `subscription_url`, only `title`, `thumbnail_url`, `detail_key`.

Replace the mapping at lines 83–89:

```rust
.map(|r| AggregatedSearchResult {
    title: r.title,
    thumbnail_url: r.thumbnail_url,
    detail_key: r.detail_key,
    source: source.clone(),
})
```

### Step 2: Update imports

```rust
use shared::{
    AggregatedSearchResponse, AggregatedSearchResult, SearchRequest, SearchResponse, ServiceType,
};
```
(Remove `description` and `subscription_url` references — these fields no longer exist.)

### Step 3: Compile check

```bash
cargo check -p core-service 2>&1
```
Expected: search.rs compiles cleanly.

### Step 4: Commit

```bash
git add core-service/src/handlers/search.rs
git commit -m "feat(core): update search handler for new detail_key-based SearchResult"
```

---

## Task 6: Add Core Detail Handler

**Files:**
- Create: `core-service/src/handlers/detail.rs`
- Modify: `core-service/src/handlers/mod.rs`
- Modify: `core-service/src/main.rs`

### Step 1: Create detail handler

```rust
// core-service/src/handlers/detail.rs
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use shared::{DetailRequest, DetailResponse, ServiceType};
use std::time::Duration;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CoreDetailRequest {
    pub detail_key: String,
    pub source: String,
}

pub async fn detail(
    State(state): State<AppState>,
    Json(payload): Json<CoreDetailRequest>,
) -> (StatusCode, Json<DetailResponse>) {
    // Find the fetcher matching the given source name
    let fetcher = match state.registry.get_services_by_type(&ServiceType::Fetcher) {
        Ok(services) => services
            .into_iter()
            .find(|s| s.service_name == payload.source && s.capabilities.detail_endpoint.is_some()),
        Err(e) => {
            tracing::error!("Failed to get fetchers: {}", e);
            return (StatusCode::OK, Json(DetailResponse { items: vec![] }));
        }
    };

    let fetcher = match fetcher {
        Some(f) => f,
        None => {
            tracing::warn!("No fetcher with detail_endpoint found for source={}", payload.source);
            return (StatusCode::OK, Json(DetailResponse { items: vec![] }));
        }
    };

    let endpoint = fetcher.capabilities.detail_endpoint.as_deref().unwrap_or("/detail");
    let url = format!("http://{}:{}{}", fetcher.host, fetcher.port, endpoint);
    let req_body = DetailRequest { detail_key: payload.detail_key.clone() };

    let client = reqwest::Client::new();
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        client.post(&url).json(&req_body).send(),
    )
    .await;

    match result {
        Ok(Ok(resp)) => match resp.json::<DetailResponse>().await {
            Ok(dr) => {
                tracing::info!(
                    "Detail for key={} returned {} items",
                    payload.detail_key,
                    dr.items.len()
                );
                (StatusCode::OK, Json(dr))
            }
            Err(e) => {
                tracing::warn!("Failed to parse detail response: {}", e);
                (StatusCode::OK, Json(DetailResponse { items: vec![] }))
            }
        },
        Ok(Err(e)) => {
            tracing::warn!("Detail request to {} failed: {}", url, e);
            (StatusCode::OK, Json(DetailResponse { items: vec![] }))
        }
        Err(_) => {
            tracing::warn!("Detail request to {} timed out", url);
            (StatusCode::OK, Json(DetailResponse { items: vec![] }))
        }
    }
}
```

### Step 2: Register module

In `core-service/src/handlers/mod.rs`, add:
```rust
pub mod detail;
```

### Step 3: Add route in main.rs

Find the search route line and add after it:
```rust
.route("/search", get(handlers::search::search))
.route("/detail", post(handlers::detail::detail))  // ← add
```

### Step 4: Compile

```bash
cargo check -p core-service 2>&1
```
Expected: compiles cleanly.

### Step 5: Commit

```bash
git add core-service/src/handlers/detail.rs core-service/src/handlers/mod.rs core-service/src/main.rs
git commit -m "feat(core): add /detail proxy endpoint"
```

---

## Task 7: Update Frontend Schemas

**Files:**
- Modify: `frontend/src/schemas/search.ts`

### Step 1: Rewrite search.ts

```typescript
import { Schema } from "effect"

export const SearchResultSchema = Schema.Struct({
  title: Schema.String,
  thumbnail_url: Schema.NullOr(Schema.String),
  detail_key: Schema.String,
  source: Schema.String,
})

export const AggregatedSearchResponseSchema = Schema.Struct({
  results: Schema.Array(SearchResultSchema),
})

export type SearchResult = typeof SearchResultSchema.Type
export type AggregatedSearchResponse = typeof AggregatedSearchResponseSchema.Type

export const DetailItemSchema = Schema.Struct({
  subgroup_name: Schema.String,
  rss_url: Schema.String,
})

export const DetailResponseSchema = Schema.Struct({
  items: Schema.Array(DetailItemSchema),
})

export type DetailItem = typeof DetailItemSchema.Type
export type DetailResponse = typeof DetailResponseSchema.Type
```

### Step 2: Commit

```bash
git add frontend/src/schemas/search.ts
git commit -m "feat(frontend): update search schema with detail_key, add DetailItem/DetailResponse"
```

---

## Task 8: Update CoreApi Service Definition and ApiLayer

**Files:**
- Modify: `frontend/src/services/CoreApi.ts`
- Modify: `frontend/src/layers/ApiLayer.ts`

### Step 1: Add getDetail to CoreApi.ts

In `CoreApi.ts`, add the import:
```typescript
import type { AggregatedSearchResponse, DetailResponse } from "@/schemas/search"
```

Add to the service interface (after `readonly search`):
```typescript
readonly getDetail: (detail_key: string, source: string) => Effect.Effect<DetailResponse>
```

### Step 2: Add getDetail to ApiLayer.ts

In `ApiLayer.ts`, add the import:
```typescript
import {
  AggregatedSearchResponseSchema,
  DetailResponseSchema,
} from "@/schemas/search"
```

Add the implementation (after the `search` entry, before the closing `})`):
```typescript
getDetail: (detail_key, source) =>
  fetchJson(
    HttpClientRequest.post("/api/core/detail").pipe(
      HttpClientRequest.bodyUnsafeJson({ detail_key, source }),
    ),
    DetailResponseSchema,
  ),
```

### Step 3: Check TypeScript

```bash
cd frontend && npx tsc --noEmit 2>&1 | head -40
```
Expected: errors only from SearchPage.tsx (which we'll fix next), not from ApiLayer.

### Step 4: Commit

```bash
git add frontend/src/services/CoreApi.ts frontend/src/layers/ApiLayer.ts
git commit -m "feat(frontend): add getDetail to CoreApi and ApiLayer"
```

---

## Task 9: Update i18n

**Files:**
- Modify: `frontend/src/i18n/zh-TW.json`
- Modify: `frontend/src/i18n/en.json`

### Step 1: Add translation keys to zh-TW.json

Find the `"search"` section and replace:
```json
"search": {
  "title": "搜尋",
  "placeholder": "搜尋動畫...",
  "noResults": "找不到結果",
  "hint": "輸入關鍵字搜尋所有來源",
  "noImage": "無圖片",
  "detail": {
    "subgroup": "字幕組",
    "rssUrl": "RSS 網址",
    "subscribe": "訂閱",
    "loading": "載入中...",
    "noItems": "找不到可訂閱的 RSS"
  }
}
```

### Step 2: Add translation keys to en.json

```json
"search": {
  "title": "Search",
  "placeholder": "Search anime...",
  "noResults": "No results found",
  "hint": "Type to search across all sources",
  "noImage": "No image",
  "detail": {
    "subgroup": "Subgroup",
    "rssUrl": "RSS URL",
    "subscribe": "Subscribe",
    "loading": "Loading...",
    "noItems": "No RSS feeds found"
  }
}
```

### Step 3: Commit

```bash
git add frontend/src/i18n/zh-TW.json frontend/src/i18n/en.json
git commit -m "feat(frontend): add i18n keys for detail dialog"
```

---

## Task 10: Rewrite SearchPage and Add DetailDialog

**Files:**
- Modify: `frontend/src/pages/search/SearchPage.tsx`
- Create: `frontend/src/pages/search/DetailDialog.tsx`

### Step 1: Create DetailDialog component

Create `frontend/src/pages/search/DetailDialog.tsx`:

```typescript
import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { toast } from "sonner"
import type { SearchResult, DetailItem } from "@/schemas/search"

interface DetailDialogProps {
  result: SearchResult | null
  onClose: () => void
}

export function DetailDialog({ result, onClose }: DetailDialogProps) {
  const { t } = useTranslation()
  const [subscribeTarget, setSubscribeTarget] = useState<DetailItem & { animeTitle: string } | null>(null)
  const [newName, setNewName] = useState("")
  const [newInterval, setNewInterval] = useState("30")

  const { data: detail, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        if (!result) return { items: [] }
        return yield* api.getDetail(result.detail_key, result.source)
      }),
    [result?.detail_key, result?.source],
  )

  const { mutate: createSubscription, isLoading: creating } = useEffectMutation(
    (req: { source_url: string; name?: string; fetch_interval_minutes?: number }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createSubscription(req)
      }),
  )

  const handleSubscribeClick = (item: DetailItem) => {
    setSubscribeTarget({ ...item, animeTitle: result?.title ?? "" })
    setNewName(`${result?.title ?? ""} - ${item.subgroup_name}`)
    setNewInterval("30")
  }

  const handleCreateSubscription = () => {
    if (!subscribeTarget) return
    createSubscription({
      source_url: subscribeTarget.rss_url,
      name: newName || undefined,
      fetch_interval_minutes: Number(newInterval) || 30,
    })
      .then(() => {
        toast.success(t("subscriptions.created", "Subscription created"))
        setSubscribeTarget(null)
      })
      .catch(() => {
        toast.error(t("common.saveFailed", "Failed to create subscription"))
      })
  }

  const items = detail?.items ?? []

  return (
    <>
      {/* Main detail dialog */}
      <Dialog open={!!result} onOpenChange={(open) => { if (!open) onClose() }}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <div className="flex items-center gap-4">
              {result?.thumbnail_url && (
                <div className="w-16 h-20 flex-shrink-0 rounded overflow-hidden bg-muted">
                  <img
                    src={result.thumbnail_url}
                    alt={result.title}
                    className="w-full h-full object-cover"
                    onError={(e) => {
                      ;(e.target as HTMLImageElement).style.display = "none"
                    }}
                  />
                </div>
              )}
              <DialogTitle className="text-lg">{result?.title}</DialogTitle>
            </div>
          </DialogHeader>

          {isLoading && (
            <p className="text-sm text-muted-foreground py-4">
              {t("search.detail.loading")}
            </p>
          )}

          {!isLoading && items.length === 0 && (
            <p className="text-sm text-muted-foreground py-4">
              {t("search.detail.noItems")}
            </p>
          )}

          {!isLoading && items.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("search.detail.subgroup")}</TableHead>
                  <TableHead>{t("search.detail.rssUrl")}</TableHead>
                  <TableHead className="w-20" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {items.map((item, idx) => (
                  <TableRow key={idx}>
                    <TableCell className="font-medium whitespace-nowrap">
                      {item.subgroup_name}
                    </TableCell>
                    <TableCell>
                      <button
                        type="button"
                        className="text-xs font-mono text-blue-500 hover:underline text-left break-all"
                        onClick={() =>
                          window.open(item.rss_url, "", "noopener,width=900,height=700")
                        }
                      >
                        {item.rss_url}
                      </button>
                    </TableCell>
                    <TableCell>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => handleSubscribeClick(item)}
                      >
                        {t("search.detail.subscribe")}
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </DialogContent>
      </Dialog>

      {/* Nested subscribe dialog */}
      <Dialog
        open={!!subscribeTarget}
        onOpenChange={(open) => { if (!open) setSubscribeTarget(null) }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.addSubscription")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            {subscribeTarget && (
              <div className="space-y-1">
                <Label>{t("subscriptions.sourceUrl")}</Label>
                <p className="text-sm font-mono text-muted-foreground break-all">
                  {subscribeTarget.rss_url}
                </p>
              </div>
            )}
            <div className="space-y-2">
              <Label>{t("subscriptions.name")}</Label>
              <Input
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label>{t("subscriptions.fetchInterval")}</Label>
              <Input
                type="number"
                min="1"
                value={newInterval}
                onChange={(e) => setNewInterval(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setSubscribeTarget(null)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={handleCreateSubscription} disabled={creating}>
              {creating ? t("common.creating") : t("common.create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
```

### Step 2: Rewrite SearchPage.tsx

Replace the full file content:

```typescript
import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { SearchBar } from "@/components/shared/SearchBar"
import { PageHeader } from "@/components/shared/PageHeader"
import { Badge } from "@/components/ui/badge"
import type { SearchResult } from "@/schemas/search"
import { DetailDialog } from "./DetailDialog"

export default function SearchPage() {
  const { t } = useTranslation()
  const [rawQuery, setRawQuery] = useState("")
  const [debouncedQuery, setDebouncedQuery] = useState("")
  const [selectedResult, setSelectedResult] = useState<SearchResult | null>(null)

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(rawQuery.trim())
    }, 500)
    return () => clearTimeout(timer)
  }, [rawQuery])

  const { data: results, isLoading, error } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        if (!debouncedQuery) return { results: [] }
        return yield* api.search(debouncedQuery)
      }),
    [debouncedQuery],
  )

  const searchResults = results?.results ?? []

  return (
    <div className="space-y-6">
      <PageHeader title={t("search.title")} />

      <SearchBar
        value={rawQuery}
        onChange={setRawQuery}
        placeholder={t("search.placeholder")}
      />

      {isLoading && debouncedQuery && (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      )}

      {!!error && (
        <p className="text-destructive text-sm">
          {t("common.error")}: {String(error)}
        </p>
      )}

      {!isLoading && !error && debouncedQuery && searchResults.length === 0 && (
        <p className="text-sm text-muted-foreground">{t("search.noResults")}</p>
      )}

      {!debouncedQuery && !isLoading && (
        <p className="text-sm text-muted-foreground">
          {t("search.hint", "Type to search across all sources")}
        </p>
      )}

      {searchResults.length > 0 && (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4">
          {searchResults.map((result, idx) => (
            <button
              key={`${result.source}-${result.detail_key}-${idx}`}
              type="button"
              className="flex flex-col items-center gap-2 p-3 border rounded-lg bg-card hover:bg-accent cursor-pointer text-left transition-colors"
              onClick={() => setSelectedResult(result)}
            >
              <div className="w-full aspect-[3/4] rounded overflow-hidden bg-muted flex-shrink-0">
                {result.thumbnail_url ? (
                  <img
                    src={result.thumbnail_url}
                    alt={result.title}
                    className="w-full h-full object-cover"
                    onError={(e) => {
                      ;(e.target as HTMLImageElement).style.display = "none"
                    }}
                  />
                ) : (
                  <div className="w-full h-full flex items-center justify-center text-muted-foreground text-xs">
                    {t("search.noImage")}
                  </div>
                )}
              </div>
              <p className="text-sm font-medium line-clamp-2 w-full">{result.title}</p>
              <Badge variant="outline" className="text-xs self-start">
                {result.source}
              </Badge>
            </button>
          ))}
        </div>
      )}

      <DetailDialog
        result={selectedResult}
        onClose={() => setSelectedResult(null)}
      />
    </div>
  )
}
```

### Step 3: Check TypeScript

```bash
cd frontend && npx tsc --noEmit 2>&1
```
Expected: no errors.

### Step 4: Commit

```bash
git add frontend/src/pages/search/SearchPage.tsx frontend/src/pages/search/DetailDialog.tsx
git commit -m "feat(frontend): rewrite SearchPage with compact cards, add DetailDialog"
```

---

## Task 11: End-to-End Verification

### Step 1: Build all Rust crates

```bash
cargo build --workspace 2>&1
```
Expected: compiles with 0 errors.

### Step 2: Run all Rust tests

```bash
cargo test --workspace 2>&1
```
Expected: all tests pass.

### Step 3: Build frontend

```bash
cd frontend && npm run build 2>&1 | tail -20
```
Expected: build succeeds.

### Step 4: Final commit if any fixups needed

```bash
git add -p
git commit -m "fix: address build issues from search detail feature"
```

---

## Summary of Changes

| File | Change |
|---|---|
| `shared/src/models.rs` | Remove `description`/`subscription_url` from SearchResult, add `detail_key`; add `DetailRequest/Response/Item`; add `detail_endpoint` to Capabilities |
| `fetchers/mikanani/src/search_scraper.rs` | Parse magnet source items; produce `detail_key` for all results |
| `fetchers/mikanani/src/detail_scraper.rs` | **New** — `DetailScraper` trait, bangumi + source detail parsing |
| `fetchers/mikanani/src/lib.rs` | Export `DetailScraper`, `RealDetailScraper` |
| `fetchers/mikanani/src/handlers.rs` | Add `detail_scraper` to `AppState`; add `detail` handler |
| `fetchers/mikanani/src/main.rs` | Add `/detail` route; add `detail_endpoint` to capabilities |
| `core-service/src/handlers/search.rs` | Update result mapping for new SearchResult shape |
| `core-service/src/handlers/detail.rs` | **New** — proxy to fetcher's `/detail` |
| `core-service/src/handlers/mod.rs` | Add `pub mod detail` |
| `core-service/src/main.rs` | Add `/detail` route |
| `frontend/src/schemas/search.ts` | Update SearchResult; add DetailItem/DetailResponse |
| `frontend/src/services/CoreApi.ts` | Add `getDetail` |
| `frontend/src/layers/ApiLayer.ts` | Implement `getDetail` |
| `frontend/src/pages/search/SearchPage.tsx` | Simplify to compact grid cards |
| `frontend/src/pages/search/DetailDialog.tsx` | **New** — detail table + nested subscribe dialog |
| `frontend/src/i18n/zh-TW.json` | Add `search.detail.*` keys |
| `frontend/src/i18n/en.json` | Add `search.detail.*` keys |
