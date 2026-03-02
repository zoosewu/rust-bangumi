# Search Feature Design

**Date:** 2026-03-02
**Status:** Approved

## Overview

Allow users to search for anime content across all supported source sites directly from the frontend, without having to visit each site manually.

**Flow:**
1. User types a query in the search page
2. Frontend calls Core `GET /search?q=<query>`
3. Core broadcasts `POST /search` to all registered Fetchers that support search (in parallel, 10s timeout each)
4. Each Fetcher scrapes/queries its source site and returns results
5. Core merges results (with source attribution) and returns to frontend
6. Frontend shows results (thumbnail, title, description, source) with a "Subscribe" button
7. Clicking "Subscribe" opens the existing `SubscriptionDialog` pre-filled with the subscription URL

---

## Architecture

### Communication Flow

```
Frontend
  → GET /api/core/search?q=芙莉蓮
      → Core collects all Fetchers with search_endpoint
      → tokio::join_all (parallel, 10s timeout each):
          → POST mikanani:8001/search {"query":"芙莉蓮"}
          → POST future-fetcher/search ...
      → Merge results, add source field
  ← AggregatedSearchResponse { results: [...] }
```

### Failure Handling

- Individual Fetcher timeout or error → result set is empty for that source, other sources unaffected
- All Fetchers fail → return empty results (not an error)
- Query is empty → return empty results immediately (no API call)

---

## Data Model Changes

### `shared/src/models.rs` — New Types

```rust
// Capability declaration (added to Capabilities struct)
pub struct Capabilities {
    pub fetch_endpoint: Option<String>,
    pub search_endpoint: Option<String>,  // NEW
    // ... existing fields
}

// Core → Fetcher
pub struct SearchRequest {
    pub query: String,
}

// Fetcher → Core (single result)
pub struct SearchResult {
    pub title: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub subscription_url: String, // e.g. https://mikanani.me/RSS/Bangumi?bangumiId=3310
}

// Fetcher → Core (response)
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

// Core → Frontend (with source attribution)
pub struct AggregatedSearchResult {
    pub title: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub subscription_url: String,
    pub source: String, // fetcher service_name, e.g. "mikanani"
}

pub struct AggregatedSearchResponse {
    pub results: Vec<AggregatedSearchResult>,
}
```

---

## Backend Changes

### 1. `core-service/src/handlers/search.rs` (new)

**Endpoint:** `GET /search?q=<query>`

```
1. Parse query from URL param
2. Load all registered services with type=Fetcher and search_endpoint present
3. For each such Fetcher:
   - Build URL: {base_url}{search_endpoint}
   - POST SearchRequest { query }
   - 10s timeout
4. tokio::join_all — collect Ok results, log and ignore Err
5. For each result, attach source = service.service_name
6. Return AggregatedSearchResponse
```

**Route registration in `core-service/src/main.rs`:**
```rust
.route("/search", get(handlers::search::search))
```

### 2. `fetchers/mikanani/src/` Changes

**New endpoint:** `POST /search`

**New handler** `search` in `fetchers/mikanani/src/handlers.rs`:
```
1. Parse SearchRequest body
2. HTTP GET https://mikanani.me/Home/Search?searchstr={url_encoded_query}
3. Parse HTML with scraper crate:
   - Find bangumi list items
   - Extract: bangumi_id, title, thumbnail_url, description
4. Construct subscription_url: https://mikanani.me/RSS/Bangumi?bangumiId={id}
5. Return SearchResponse { results }
```

**New file** `fetchers/mikanani/src/search_scraper.rs`:
- HTML scraping logic using `scraper` crate
- CSS selectors targeting Mikanani's bangumi search page structure

**Registration change** in `fetchers/mikanani/src/main.rs`:
```rust
capabilities: Capabilities {
    fetch_endpoint: Some("/fetch".to_string()),
    search_endpoint: Some("/search".to_string()),  // ADD
    ...
}
```

**Route registration in** `fetchers/mikanani/src/main.rs`:
```rust
.route("/search", post(handlers::search))
```

**New dependency** in `fetchers/mikanani/Cargo.toml`:
```toml
scraper = "0.20"
```

---

## Frontend Changes

### New Files

**`frontend/src/schemas/search.ts`**
```typescript
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

**`frontend/src/pages/search/SearchPage.tsx`**

Layout:
```
┌──────────────────────────────────────┐
│ 搜尋                                 │
│ ┌────────────────────────────────┐   │
│ │ 🔍 搜尋動畫...                 │   │
│ └────────────────────────────────┘   │
│                                      │
│ [loading spinner / empty state]      │
│ ┌────┬─────────────────────────────┐ │
│ │    │ 葬送的芙莉蓮               │ │
│ │縮圖│ 說明文字...                │ │
│ │    │ 來源: mikanani  [訂閱]     │ │
│ └────┴─────────────────────────────┘ │
└──────────────────────────────────────┘
```

Behavior:
- Debounced input (500ms) before firing API call
- Loading state while waiting for Core response
- Empty state when query is blank or no results
- "訂閱" button opens `SubscriptionDialog` with `source_url` pre-filled

### Modified Files

**`frontend/src/services/CoreApi.ts`**
- Add: `readonly search: (query: string) => Effect.Effect<AggregatedSearchResponse>`

**`frontend/src/layers/ApiLayer.ts`**
- Implement: `GET /search?q={query}` → decode with `AggregatedSearchResponseSchema`

**`frontend/src/components/layout/Sidebar.tsx`**
- Add search entry to `mainNavItems`:
  ```typescript
  { to: "/search", icon: Search, labelKey: "sidebar.search" }
  ```

**`frontend/src/App.tsx`**
- Add route: `<Route path="/search" element={<SearchPage />} />`

**`frontend/src/i18n/en.json` / `zh-TW.json` / `ja.json`**
- Add keys:
  ```json
  "sidebar": { "search": "Search" }
  "search": {
    "title": "Search",
    "placeholder": "Search anime...",
    "noResults": "No results found",
    "subscribe": "Subscribe",
    "source": "Source"
  }
  ```

---

## Files Summary

| File | Change |
|------|--------|
| `shared/src/models.rs` | Add `search_endpoint` to Capabilities; add SearchRequest, SearchResult, SearchResponse, AggregatedSearchResult, AggregatedSearchResponse |
| `core-service/src/handlers/search.rs` | **New** — GET /search handler |
| `core-service/src/handlers/mod.rs` | Add `pub mod search` |
| `core-service/src/main.rs` | Register `/search` route |
| `fetchers/mikanani/src/search_scraper.rs` | **New** — HTML scraping logic |
| `fetchers/mikanani/src/handlers.rs` | Add `search` handler |
| `fetchers/mikanani/src/main.rs` | Register `/search` route + update Capabilities |
| `fetchers/mikanani/Cargo.toml` | Add `scraper` dependency |
| `frontend/src/schemas/search.ts` | **New** — Schema definitions |
| `frontend/src/pages/search/SearchPage.tsx` | **New** — Search page component |
| `frontend/src/services/CoreApi.ts` | Add `search` method |
| `frontend/src/layers/ApiLayer.ts` | Implement search API call |
| `frontend/src/components/layout/Sidebar.tsx` | Add search nav item |
| `frontend/src/App.tsx` | Add /search route |
| `frontend/src/i18n/en.json` | Add i18n keys |
| `frontend/src/i18n/zh-TW.json` | Add i18n keys |
| `frontend/src/i18n/ja.json` | Add i18n keys |
