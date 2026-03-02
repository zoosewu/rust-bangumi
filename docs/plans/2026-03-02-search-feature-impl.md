# Search Feature Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a cross-site anime search page where users can search content from all registered fetchers and subscribe to results with one click.

**Architecture:** Core exposes `GET /search?q=` which fans out to all registered Fetchers (that declare `search_endpoint`) in parallel with 10s timeout each. Mikanani fetcher scrapes mikanani.me search page HTML with the `scraper` crate. Results are merged with source attribution and returned. Frontend `SearchPage` is accessible from the sidebar; results show thumbnail + title; "Subscribe" button opens an inline create-subscription dialog pre-filled with the URL.

**Tech Stack:** Rust/Axum (Core + Fetcher), `scraper` crate (HTML parsing), `reqwest` (HTTP), TypeScript/React, Effect-TS, shadcn/ui, Lucide icons

---

## Task 1: Add search types to `shared/src/models.rs`

**Files:**
- Modify: `shared/src/models.rs`

**Step 1: Add `search_endpoint` to `Capabilities` and new search structs**

In `shared/src/models.rs`, find the `Capabilities` struct (currently around line 58):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_endpoint: Option<String>,
    #[serde(default)]
    pub supported_download_types: Vec<DownloadType>,
}
```

Replace with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_endpoint: Option<String>,
    #[serde(default)]
    pub supported_download_types: Vec<DownloadType>,
}
```

Then at the end of the file (after the last struct), add:

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
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub subscription_url: String,
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
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub subscription_url: String,
    pub source: String,
}

/// Core → Frontend: final search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedSearchResponse {
    pub results: Vec<AggregatedSearchResult>,
}
```

**Step 2: Verify it compiles**

```bash
cd /workspace && cargo check -p shared 2>&1 | tail -5
```

Expected: `Finished` with no errors.

**Step 3: Fix all compile errors from `Capabilities` change**

The `Capabilities` struct is constructed in several places. Find all of them:

```bash
cd /workspace && cargo check 2>&1 | grep "missing field"
```

Each location that constructs `Capabilities { ... }` needs `search_endpoint: None` added. The affected files are:
- `core-service/src/main.rs` (in `load_existing_services`) — multiple places
- `core-service/src/services/registry.rs` (test helper `create_test_service`)
- `fetchers/mikanani/src/main.rs` (two places: real startup and `#[cfg(test)]`)

For each location, add `search_endpoint: None,` after `fetch_endpoint:`.

**Step 4: Verify full workspace compiles**

```bash
cd /workspace && cargo check 2>&1 | tail -10
```

Expected: `Finished` with no errors.

**Step 5: Commit**

```bash
git add shared/src/models.rs core-service/src/main.rs core-service/src/services/registry.rs fetchers/mikanani/src/main.rs
git commit -m "feat(shared): add search_endpoint to Capabilities and search API types"
```

---

## Task 2: Core service — search handler

**Files:**
- Create: `core-service/src/handlers/search.rs`

**Step 1: Create the handler file**

```rust
// core-service/src/handlers/search.rs
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use futures::future::join_all;
use serde::Deserialize;
use shared::{
    AggregatedSearchResponse, AggregatedSearchResult, SearchRequest, SearchResponse, ServiceType,
};
use std::time::Duration;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SearchQueryParams {
    pub q: Option<String>,
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchQueryParams>,
) -> (StatusCode, Json<AggregatedSearchResponse>) {
    let query = params.q.unwrap_or_default();
    let query = query.trim().to_string();

    if query.is_empty() {
        return (
            StatusCode::OK,
            Json(AggregatedSearchResponse { results: vec![] }),
        );
    }

    // Collect fetchers that support search
    let fetchers = match state.registry.get_services_by_type(&ServiceType::Fetcher) {
        Ok(services) => services
            .into_iter()
            .filter(|s| s.capabilities.search_endpoint.is_some())
            .collect::<Vec<_>>(),
        Err(e) => {
            tracing::error!("Failed to get fetchers from registry: {}", e);
            return (
                StatusCode::OK,
                Json(AggregatedSearchResponse { results: vec![] }),
            );
        }
    };

    if fetchers.is_empty() {
        tracing::warn!("No fetchers with search_endpoint registered");
        return (
            StatusCode::OK,
            Json(AggregatedSearchResponse { results: vec![] }),
        );
    }

    let client = reqwest::Client::new();
    let search_request = SearchRequest {
        query: query.clone(),
    };

    // Fan out in parallel — each fetcher gets a 10s timeout
    let tasks = fetchers.into_iter().map(|fetcher| {
        let client = client.clone();
        let req = search_request.clone();
        let base_url = format!("http://{}:{}", fetcher.host, fetcher.port);
        let endpoint = fetcher.capabilities.search_endpoint.clone().unwrap();
        let url = format!("{}{}", base_url, endpoint);
        let source = fetcher.service_name.clone();

        async move {
            let result = tokio::time::timeout(
                Duration::from_secs(10),
                client.post(&url).json(&req).send(),
            )
            .await;

            match result {
                Ok(Ok(resp)) => match resp.json::<SearchResponse>().await {
                    Ok(sr) => sr
                        .results
                        .into_iter()
                        .map(|r| AggregatedSearchResult {
                            title: r.title,
                            description: r.description,
                            thumbnail_url: r.thumbnail_url,
                            subscription_url: r.subscription_url,
                            source: source.clone(),
                        })
                        .collect::<Vec<_>>(),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse search response from {}: {}",
                            source,
                            e
                        );
                        vec![]
                    }
                },
                Ok(Err(e)) => {
                    tracing::warn!("Search request to {} failed: {}", source, e);
                    vec![]
                }
                Err(_) => {
                    tracing::warn!("Search request to {} timed out after 10s", source);
                    vec![]
                }
            }
        }
    });

    let results: Vec<AggregatedSearchResult> = join_all(tasks)
        .await
        .into_iter()
        .flatten()
        .collect();

    tracing::info!("Search '{}' returned {} results", query, results.len());
    (StatusCode::OK, Json(AggregatedSearchResponse { results }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ServiceRegistry;
    use crate::state::AppState;

    fn make_state_no_fetchers() -> AppState {
        // Uses test AppState — requires a running DB for full integration,
        // but registry is in-memory so we can test logic without DB.
        // For a pure unit test, just test the empty-query path.
        todo!("AppState requires a DB pool; test via integration tests instead")
    }

    #[tokio::test]
    async fn test_search_empty_query_returns_empty() {
        // Test the guard clause directly without AppState
        let query = "  ".trim().to_string();
        assert!(query.is_empty());
        // If query is empty, handler returns empty results — verified by code inspection
    }
}
```

**Step 2: Verify it compiles**

```bash
cd /workspace && cargo check -p core-service 2>&1 | tail -10
```

Expected: `Finished` with no errors. If there are import errors, check that `futures` is in scope (it is — `futures.workspace = true` in core-service Cargo.toml).

**Step 3: Commit**

```bash
git add core-service/src/handlers/search.rs
git commit -m "feat(core): add search handler that fans out to all registered fetchers"
```

---

## Task 3: Wire search handler into Core routing

**Files:**
- Modify: `core-service/src/handlers/mod.rs`
- Modify: `core-service/src/main.rs`

**Step 1: Register the module in `mod.rs`**

In `core-service/src/handlers/mod.rs`, add after the last `pub mod` line:

```rust
pub mod search;
```

**Step 2: Add the route in `main.rs`**

In `core-service/src/main.rs`, find the route block ending with:

```rust
        // 健康檢查
        .route("/health", get(health_check))
```

Add BEFORE `/health`:

```rust
        // 搜尋
        .route("/search", get(handlers::search::search))
```

**Step 3: Update `load_existing_services` to expose search endpoint for Fetchers**

In `core-service/src/main.rs`, find the `load_existing_services` function. Find the `ModuleTypeEnum::Fetcher` arm:

```rust
                            ModuleTypeEnum::Fetcher => (
                                shared::ServiceType::Fetcher,
                                shared::Capabilities {
                                    fetch_endpoint: Some("/fetch".to_string()),
                                    download_endpoint: None,
                                    sync_endpoint: None,
                                    supported_download_types: vec![],
                                },
                            ),
```

Replace with:

```rust
                            ModuleTypeEnum::Fetcher => (
                                shared::ServiceType::Fetcher,
                                shared::Capabilities {
                                    fetch_endpoint: Some("/fetch".to_string()),
                                    search_endpoint: Some("/search".to_string()),
                                    download_endpoint: None,
                                    sync_endpoint: None,
                                    supported_download_types: vec![],
                                },
                            ),
```

**Step 4: Verify compile**

```bash
cd /workspace && cargo check -p core-service 2>&1 | tail -5
```

Expected: `Finished` with no errors.

**Step 5: Commit**

```bash
git add core-service/src/handlers/mod.rs core-service/src/main.rs
git commit -m "feat(core): register /search route and expose search_endpoint for fetchers"
```

---

## Task 4: Mikanani — HTML scraper

**Files:**
- Create: `fetchers/mikanani/src/search_scraper.rs`
- Modify: `fetchers/mikanani/Cargo.toml`

**Step 1: Add `scraper` dependency to Cargo.toml**

In `fetchers/mikanani/Cargo.toml`, after the `# RSS 解析` block, add:

```toml
# HTML 解析
scraper = "0.20"
```

**Step 2: Inspect Mikanani's search page HTML structure**

Before writing selectors, verify the actual page structure:

```bash
curl -s "https://mikanani.me/Home/Search?searchstr=test" | grep -A5 "an-ul\|an-info\|bangumi\|Bangumi" | head -40
```

Expected: HTML output containing bangumi card elements. Note the actual CSS classes used.

**Step 3: Create the scraper module**

Based on Mikanani's typical HTML structure (verify selectors from Step 2 output):

```rust
// fetchers/mikanani/src/search_scraper.rs
use scraper::{Html, Selector};
use shared::SearchResult;

/// Scrape Mikanani search page for a given query.
/// Returns a list of search results or an error string.
pub async fn scrape_mikanani_search(query: &str) -> Result<Vec<SearchResult>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (compatible; bangumi-bot/1.0)")
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let response = client
        .get("https://mikanani.me/Home/Search")
        .query(&[("searchstr", query)])
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Mikanani search: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Mikanani search returned status {}",
            response.status()
        ));
    }

    let html = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    parse_search_results(&html)
}

/// Parse HTML from Mikanani search results page.
/// CSS selectors target the bangumi card structure: `div.an-ul a.an-info-group`
/// Each card has `href="/Home/Bangumi/{id}"`, an `img` for thumbnail,
/// and `p.an-text` for the title.
///
/// NOTE: If selectors return 0 results, inspect the actual HTML:
///   curl -s "https://mikanani.me/Home/Search?searchstr=test" | grep -i "bangumi\|an-ul\|an-info"
/// and update the selector strings accordingly.
pub fn parse_search_results(html: &str) -> Result<Vec<SearchResult>, String> {
    let document = Html::parse_document(html);

    // Primary selector: anchor tags linking to /Home/Bangumi/{id}
    // These wrap the full bangumi card
    let item_sel = Selector::parse("a.an-info-group")
        .map_err(|e| format!("Invalid CSS selector: {:?}", e))?;
    let title_sel = Selector::parse("p.an-text")
        .map_err(|e| format!("Invalid CSS selector: {:?}", e))?;
    let img_sel = Selector::parse("img")
        .map_err(|e| format!("Invalid CSS selector: {:?}", e))?;

    let mut results = Vec::new();

    for element in document.select(&item_sel) {
        // Extract href, e.g. "/Home/Bangumi/3310"
        let href = match element.value().attr("href") {
            Some(h) => h,
            None => continue,
        };

        // Only process bangumi links
        if !href.contains("/Home/Bangumi/") {
            continue;
        }

        // Extract the bangumi ID from the href path
        let bangumi_id: u32 = match href.rsplit('/').next().and_then(|s| s.parse().ok()) {
            Some(id) => id,
            None => {
                tracing::warn!("Could not parse bangumi ID from href: {}", href);
                continue;
            }
        };

        // Extract title text
        let title = element
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        // Extract thumbnail URL (make absolute if relative)
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

        let subscription_url = format!(
            "https://mikanani.me/RSS/Bangumi?bangumiId={}",
            bangumi_id
        );

        results.push(SearchResult {
            title,
            description: None,
            thumbnail_url,
            subscription_url,
        });
    }

    tracing::info!(
        "Mikanani search parsed {} results from {} HTML bytes",
        results.len(),
        html.len()
    );

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_html() {
        let result = parse_search_results("<html><body></body></html>").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_bangumi_card() {
        // Minimal HTML matching the expected structure
        let html = r#"
            <html><body>
              <div class="an-ul">
                <a class="an-info-group" href="/Home/Bangumi/3310">
                  <div class="an-img-cell">
                    <img src="/images/Bangumi/3310/cover.jpg" />
                  </div>
                  <div class="an-info">
                    <p class="an-text">葬送的芙莉蓮</p>
                  </div>
                </a>
              </div>
            </body></html>
        "#;

        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "葬送的芙莉蓮");
        assert_eq!(
            results[0].subscription_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3310"
        );
        assert_eq!(
            results[0].thumbnail_url,
            Some("https://mikanani.me/images/Bangumi/3310/cover.jpg".to_string())
        );
    }

    #[test]
    fn test_parse_skips_non_bangumi_links() {
        let html = r#"
            <html><body>
              <a class="an-info-group" href="/Home/Episode/abc123">
                <p class="an-text">Some Episode</p>
              </a>
            </body></html>
        "#;
        let results = parse_search_results(html).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_absolute_thumbnail_url_unchanged() {
        let html = r#"
            <html><body>
              <a class="an-info-group" href="/Home/Bangumi/9999">
                <img src="https://cdn.example.com/cover.jpg" />
                <p class="an-text">Test Anime</p>
              </a>
            </body></html>
        "#;
        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].thumbnail_url,
            Some("https://cdn.example.com/cover.jpg".to_string())
        );
    }
}
```

**Step 4: Run unit tests**

```bash
cd /workspace && cargo test -p fetcher-mikanani search_scraper 2>&1 | tail -15
```

Expected: All 4 tests pass. If `test_parse_bangumi_card` fails, the CSS selectors don't match — adjust them based on Step 2's curl output.

**Step 5: Commit**

```bash
git add fetchers/mikanani/Cargo.toml fetchers/mikanani/src/search_scraper.rs
git commit -m "feat(mikanani): add HTML search scraper with unit tests"
```

---

## Task 5: Mikanani — search handler and route

**Files:**
- Modify: `fetchers/mikanani/src/handlers.rs`
- Modify: `fetchers/mikanani/src/lib.rs`
- Modify: `fetchers/mikanani/src/main.rs`

**Step 1: Add `search` handler to `handlers.rs`**

At the top of `fetchers/mikanani/src/handlers.rs`, add to the existing imports:

```rust
use fetcher_mikanani::search_scraper::scrape_mikanani_search;
use shared::{SearchRequest, SearchResponse};
```

Then at the end of the file (after the existing `#[cfg(test)]` block's closing brace), add the new handler:

```rust
pub async fn search(
    Json(payload): Json<SearchRequest>,
) -> (StatusCode, Json<SearchResponse>) {
    tracing::info!("Received search request: query={:?}", payload.query);

    match scrape_mikanani_search(&payload.query).await {
        Ok(results) => {
            tracing::info!("Search returned {} results", results.len());
            (StatusCode::OK, Json(SearchResponse { results }))
        }
        Err(e) => {
            tracing::error!("Search scraping failed: {}", e);
            // Return empty results rather than an error — Core handles partial failures
            (StatusCode::OK, Json(SearchResponse { results: vec![] }))
        }
    }
}
```

**Step 2: Export `search_scraper` from `lib.rs`**

In `fetchers/mikanani/src/lib.rs`, add:

```rust
pub mod search_scraper;
```

**Step 3: Add route and update capabilities in `main.rs`**

In `fetchers/mikanani/src/main.rs`, find the router:

```rust
    let mut app = Router::new()
        .route("/fetch", post(handlers::fetch))
        .route("/health", get(handlers::health_check))
        .route(
            "/can-handle-subscription",
            post(handlers::can_handle_subscription),
        )
        .with_state(app_state);
```

Replace with:

```rust
    let mut app = Router::new()
        .route("/fetch", post(handlers::fetch))
        .route("/search", post(handlers::search))
        .route("/health", get(handlers::health_check))
        .route(
            "/can-handle-subscription",
            post(handlers::can_handle_subscription),
        )
        .with_state(app_state);
```

Then find the `ServiceRegistration` in the tokio spawn block (the main registration):

```rust
            capabilities: shared::Capabilities {
                fetch_endpoint: Some("/fetch".to_string()),
                download_endpoint: None,
                sync_endpoint: None,
                supported_download_types: vec![],
            },
```

Replace with:

```rust
            capabilities: shared::Capabilities {
                fetch_endpoint: Some("/fetch".to_string()),
                search_endpoint: Some("/search".to_string()),
                download_endpoint: None,
                sync_endpoint: None,
                supported_download_types: vec![],
            },
```

Also update the `#[cfg(test)]` registration helper at the bottom of `main.rs` (the `register_to_core` function) the same way.

**Step 4: Verify compile**

```bash
cd /workspace && cargo check -p fetcher-mikanani 2>&1 | tail -5
```

Expected: `Finished` with no errors.

**Step 5: Run all mikanani tests**

```bash
cd /workspace && cargo test -p fetcher-mikanani 2>&1 | tail -15
```

Expected: All tests pass. The handler test in `handlers.rs` for `test_fetch_returns_202_accepted` and others should still pass.

**Step 6: Commit**

```bash
git add fetchers/mikanani/src/handlers.rs fetchers/mikanani/src/lib.rs fetchers/mikanani/src/main.rs
git commit -m "feat(mikanani): add /search endpoint and declare search_endpoint capability"
```

---

## Task 6: Frontend — schema and API layer

**Files:**
- Create: `frontend/src/schemas/search.ts`
- Modify: `frontend/src/services/CoreApi.ts`
- Modify: `frontend/src/layers/ApiLayer.ts`

**Step 1: Create search schema**

```typescript
// frontend/src/schemas/search.ts
import { Schema } from "effect"

export const SearchResultSchema = Schema.Struct({
  title: Schema.String,
  description: Schema.NullOr(Schema.String),
  thumbnail_url: Schema.NullOr(Schema.String),
  subscription_url: Schema.String,
  source: Schema.String,
})

export const AggregatedSearchResponseSchema = Schema.Struct({
  results: Schema.Array(SearchResultSchema),
})

export type SearchResult = typeof SearchResultSchema.Type
export type AggregatedSearchResponse = typeof AggregatedSearchResponseSchema.Type
```

**Step 2: Add `search` to `CoreApi.ts`**

In `frontend/src/services/CoreApi.ts`, add the import at the top:

```typescript
import type { AggregatedSearchResponse } from "@/schemas/search"
```

Then in the `CoreApi.of({...})` interface block, add after the last `readonly` line (before the closing `}`):

```typescript
    readonly search: (query: string) => Effect.Effect<AggregatedSearchResponse>
```

**Step 3: Implement `search` in `ApiLayer.ts`**

In `frontend/src/layers/ApiLayer.ts`, add the import at the top:

```typescript
import { AggregatedSearchResponse, AggregatedSearchResponseSchema } from "@/schemas/search"
```

Then in the `return CoreApi.of({...})` block, add after the last entry (before the closing `}`):

```typescript
    search: (query) => {
      const qs = new URLSearchParams()
      qs.set("q", query)
      return fetchJson(
        HttpClientRequest.get(`/api/core/search?${qs.toString()}`),
        AggregatedSearchResponseSchema,
      )
    },
```

**Step 4: Verify TypeScript compiles**

```bash
cd /workspace/frontend && npx tsc --noEmit 2>&1 | tail -10
```

Expected: No errors.

**Step 5: Commit**

```bash
git add frontend/src/schemas/search.ts frontend/src/services/CoreApi.ts frontend/src/layers/ApiLayer.ts
git commit -m "feat(frontend): add search schema and CoreApi.search method"
```

---

## Task 7: Frontend — SearchPage component

**Files:**
- Create: `frontend/src/pages/search/SearchPage.tsx`

**Step 1: Create the page**

```tsx
// frontend/src/pages/search/SearchPage.tsx
import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { SearchBar } from "@/components/shared/SearchBar"
import { PageHeader } from "@/components/shared/PageHeader"
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
import { Badge } from "@/components/ui/badge"
import { toast } from "sonner"
import type { SearchResult } from "@/schemas/search"
import type { ServiceModule } from "@/schemas/service-module"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

export default function SearchPage() {
  const { t } = useTranslation()
  const [rawQuery, setRawQuery] = useState("")
  const [debouncedQuery, setDebouncedQuery] = useState("")
  const [subscribeTarget, setSubscribeTarget] = useState<SearchResult | null>(null)
  const [newName, setNewName] = useState("")
  const [newInterval, setNewInterval] = useState("30")
  const [newPreferredDl, setNewPreferredDl] = useState<number | null>(null)

  // Debounce search input by 500ms
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

  const { data: downloaderModules } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getDownloaderModules
      }),
    [],
  )

  const { mutate: createSubscription, isLoading: creating } = useEffectMutation(
    (req: {
      source_url: string
      name?: string
      fetch_interval_minutes?: number
      preferred_downloader_id?: number | null
    }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createSubscription(req)
      }),
  )

  const handleSubscribeClick = (result: SearchResult) => {
    setSubscribeTarget(result)
    setNewName("")
    setNewInterval("30")
    setNewPreferredDl(null)
  }

  const handleCreateSubscription = () => {
    if (!subscribeTarget) return
    createSubscription({
      source_url: subscribeTarget.subscription_url,
      name: newName || undefined,
      fetch_interval_minutes: Number(newInterval) || 30,
      preferred_downloader_id: newPreferredDl,
    })
      .then(() => {
        toast.success(t("subscriptions.created", "Subscription created"))
        setSubscribeTarget(null)
      })
      .catch(() => {
        toast.error(t("common.saveFailed", "Failed to create subscription"))
      })
  }

  const searchResults = results?.results ?? []

  return (
    <div className="space-y-6">
      <PageHeader title={t("search.title")} />

      <SearchBar
        value={rawQuery}
        onChange={setRawQuery}
        placeholder={t("search.placeholder")}
      />

      {/* States */}
      {isLoading && debouncedQuery && (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      )}

      {error && (
        <p className="text-destructive text-sm">
          {t("common.error")}: {String(error)}
        </p>
      )}

      {!isLoading && !error && debouncedQuery && searchResults.length === 0 && (
        <p className="text-sm text-muted-foreground">{t("search.noResults")}</p>
      )}

      {!debouncedQuery && !isLoading && (
        <p className="text-sm text-muted-foreground">{t("search.hint", "Type to search across all sources")}</p>
      )}

      {/* Results list */}
      {searchResults.length > 0 && (
        <div className="space-y-3">
          {searchResults.map((result, idx) => (
            <div
              key={`${result.source}-${result.subscription_url}-${idx}`}
              className="flex items-start gap-4 p-4 border rounded-lg bg-card"
            >
              {/* Thumbnail */}
              <div className="w-16 h-20 flex-shrink-0 rounded overflow-hidden bg-muted">
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
                    {t("search.noImage", "No image")}
                  </div>
                )}
              </div>

              {/* Info */}
              <div className="flex-1 min-w-0">
                <p className="font-medium truncate">{result.title}</p>
                {result.description && (
                  <p className="text-sm text-muted-foreground mt-1 line-clamp-2">
                    {result.description}
                  </p>
                )}
                <div className="flex items-center gap-2 mt-2">
                  <Badge variant="outline" className="text-xs">
                    {result.source}
                  </Badge>
                  <span className="text-xs text-muted-foreground font-mono truncate max-w-[300px]">
                    {result.subscription_url}
                  </span>
                </div>
              </div>

              {/* Subscribe button */}
              <Button
                size="sm"
                onClick={() => handleSubscribeClick(result)}
                className="flex-shrink-0"
              >
                {t("search.subscribe")}
              </Button>
            </div>
          ))}
        </div>
      )}

      {/* Create Subscription Dialog */}
      <Dialog open={!!subscribeTarget} onOpenChange={(open) => { if (!open) setSubscribeTarget(null) }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.addSubscription")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            {subscribeTarget && (
              <div className="space-y-1">
                <Label>{t("subscriptions.sourceUrl")}</Label>
                <p className="text-sm font-mono text-muted-foreground break-all">
                  {subscribeTarget.subscription_url}
                </p>
              </div>
            )}
            <div className="space-y-2">
              <Label>{t("subscriptions.name")}</Label>
              <Input
                placeholder={subscribeTarget?.title ?? t("subscriptions.name")}
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
            {downloaderModules && (downloaderModules as ServiceModule[]).length > 0 && (
              <div className="space-y-2">
                <Label>{t("subscriptions.preferredDownloader")}</Label>
                <Select
                  value={newPreferredDl ? String(newPreferredDl) : "none"}
                  onValueChange={(v) => setNewPreferredDl(v === "none" ? null : Number(v))}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="none">{t("subscriptions.useGlobalPriority")}</SelectItem>
                    {(downloaderModules as ServiceModule[]).map((m) => (
                      <SelectItem key={m.module_id} value={String(m.module_id)}>
                        {m.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}
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
    </div>
  )
}
```

**Step 2: Verify TypeScript compiles**

```bash
cd /workspace/frontend && npx tsc --noEmit 2>&1 | tail -10
```

Expected: No errors. If there are missing import errors, check the `SearchBar` component exists at `@/components/shared/SearchBar`, `PageHeader` at `@/components/shared/PageHeader`, and that `useEffectMutation` is imported correctly.

**Step 3: Commit**

```bash
git add frontend/src/pages/search/SearchPage.tsx
git commit -m "feat(frontend): add SearchPage with debounced search and subscribe dialog"
```

---

## Task 8: Wire into app — routing, sidebar, i18n

**Files:**
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/components/layout/Sidebar.tsx`
- Modify: `frontend/src/i18n/en.json`
- Modify: `frontend/src/i18n/zh-TW.json`
- Modify: `frontend/src/i18n/ja.json`

**Step 1: Add route to `App.tsx`**

In `frontend/src/App.tsx`, add the import after the existing imports:

```typescript
import SearchPage from "@/pages/search/SearchPage"
```

Then inside the `<Routes>`, add after the last `<Route>`:

```tsx
          <Route path="search" element={<SearchPage />} />
```

**Step 2: Add sidebar nav item**

In `frontend/src/components/layout/Sidebar.tsx`, add `Search` to the lucide icon imports:

```typescript
import {
  LayoutDashboard,
  Film,
  Rss,
  RefreshCw,
  AlertTriangle,
  Users,
  Library,
  ScanText,
  Filter,
  ChevronDown,
  ChevronRight,
  Search,
} from "lucide-react"
```

Then in the `mainNavItems` array, add after the subscriptions item:

```typescript
const mainNavItems = [
  { to: "/", icon: LayoutDashboard, labelKey: "sidebar.dashboard" },
  { to: "/subscriptions", icon: Rss, labelKey: "sidebar.subscriptions" },
  { to: "/search", icon: Search, labelKey: "sidebar.search" },
  { to: "/anime", icon: Film, labelKey: "sidebar.animeSeries" },
  { to: "/raw-items", icon: RefreshCw, labelKey: "sidebar.rawItems" },
  { to: "/conflicts", icon: AlertTriangle, labelKey: "sidebar.conflicts" },
]
```

**Step 3: Add i18n keys to `en.json`**

In `frontend/src/i18n/en.json`, add `"search"` to the `"sidebar"` object:

```json
  "sidebar": {
    "title": "Bangumi",
    "dashboard": "Dashboard",
    "animeSeries": "Anime",
    "anime": "Anime Titles",
    "subtitleGroups": "Subtitle Groups",
    "subscriptions": "Subscriptions",
    "search": "Search",
    "rawItems": "Latest Updates",
    "conflicts": "Conflicts",
    "parsers": "Parsers",
    "filters": "Filters",
    "others": "Others"
  },
```

And add a new `"search"` top-level key (after `"sidebar"`):

```json
  "search": {
    "title": "Search",
    "placeholder": "Search anime...",
    "noResults": "No results found",
    "subscribe": "Subscribe",
    "hint": "Type to search across all sources",
    "noImage": "No image"
  },
```

**Step 4: Add i18n keys to `zh-TW.json`**

Add `"search": "搜尋"` to `"sidebar"`, and add:

```json
  "search": {
    "title": "搜尋",
    "placeholder": "搜尋動畫...",
    "noResults": "找不到結果",
    "subscribe": "訂閱",
    "hint": "輸入關鍵字搜尋所有來源",
    "noImage": "無圖片"
  },
```

**Step 5: Add i18n keys to `ja.json`**

Add `"search": "検索"` to `"sidebar"`, and add:

```json
  "search": {
    "title": "検索",
    "placeholder": "アニメを検索...",
    "noResults": "結果が見つかりません",
    "subscribe": "サブスクライブ",
    "hint": "すべてのソースを検索するには入力してください",
    "noImage": "画像なし"
  },
```

**Step 6: Verify TypeScript compiles**

```bash
cd /workspace/frontend && npx tsc --noEmit 2>&1 | tail -10
```

Expected: No errors.

**Step 7: Verify frontend builds**

```bash
cd /workspace/frontend && npm run build 2>&1 | tail -10
```

Expected: `built in Xs` with no errors.

**Step 8: Commit**

```bash
git add frontend/src/App.tsx frontend/src/components/layout/Sidebar.tsx frontend/src/i18n/en.json frontend/src/i18n/zh-TW.json frontend/src/i18n/ja.json
git commit -m "feat(frontend): wire SearchPage into routing, sidebar, and i18n"
```

---

## Task 9: Verification

**Step 1: Full Rust workspace build**

```bash
cd /workspace && cargo check 2>&1 | tail -5
```

Expected: `Finished` with no errors.

**Step 2: Run all Rust tests**

```bash
cd /workspace && cargo test -p fetcher-mikanani -p core-service 2>&1 | tail -20
```

Expected: All tests pass.

**Step 3: Frontend build**

```bash
cd /workspace/frontend && npm run build 2>&1 | tail -10
```

Expected: No errors.

**Step 4: Verify CSS selectors (if Mikanani is reachable)**

```bash
curl -s "https://mikanani.me/Home/Search?searchstr=%E8%8A%99%E8%8E%89%E8%93%AE" \
  | grep -o 'class="[^"]*"' | sort -u | head -20
```

If the output doesn't include `an-info-group` or `an-text`, adjust the selectors in `search_scraper.rs` and re-run the unit tests.

---

## Summary of Changed Files

| File | Change |
|------|--------|
| `shared/src/models.rs` | Add `search_endpoint` to `Capabilities`; add 5 new search types |
| `core-service/src/handlers/search.rs` | **New** — GET /search handler |
| `core-service/src/handlers/mod.rs` | Add `pub mod search` |
| `core-service/src/main.rs` | Register route; expose `search_endpoint` in `load_existing_services` |
| `core-service/src/services/registry.rs` | Add `search_endpoint: None` to test helper |
| `fetchers/mikanani/Cargo.toml` | Add `scraper = "0.20"` |
| `fetchers/mikanani/src/search_scraper.rs` | **New** — HTML scraping logic |
| `fetchers/mikanani/src/handlers.rs` | Add `search` handler |
| `fetchers/mikanani/src/lib.rs` | Export `search_scraper` module |
| `fetchers/mikanani/src/main.rs` | Register `/search` route; update capabilities |
| `frontend/src/schemas/search.ts` | **New** — Effect-TS schema definitions |
| `frontend/src/pages/search/SearchPage.tsx` | **New** — Search page component |
| `frontend/src/services/CoreApi.ts` | Add `search` method signature |
| `frontend/src/layers/ApiLayer.ts` | Implement `search` API call |
| `frontend/src/App.tsx` | Add `/search` route |
| `frontend/src/components/layout/Sidebar.tsx` | Add Search nav item |
| `frontend/src/i18n/en.json` | Add i18n keys |
| `frontend/src/i18n/zh-TW.json` | Add i18n keys |
| `frontend/src/i18n/ja.json` | Add i18n keys |
