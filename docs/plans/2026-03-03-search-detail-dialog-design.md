# Search Detail Dialog Design

**Date:** 2026-03-03
**Branch:** feature/search-feature
**Status:** Approved

---

## Overview

改進前端搜尋功能：搜尋結果卡片只顯示縮圖與標題，點擊後開啟 Dialog，向 fetcher 進階查詢並以表格列出可訂閱的 RSS 清單。使用者可直接點擊 RSS URL 開啟新視窗預覽，或點擊訂閱按鈕預填資料進行訂閱。

---

## Core Principles

- Fetcher 對外只提供 `/search` 與 `/detail` 兩個端點
- Core 和 Frontend 對結果類型（季番 vs magnet source）完全透明
- `detail_key` 為 fetcher 自訂的不透明字串，由 fetcher 發出、收回、解讀

---

## Data Models

### Fetcher SearchResult
```rust
SearchResult {
    title:         String,
    thumbnail_url: Option<String>,
    detail_key:    String,   // e.g. "bangumi:3822" or "source:[KITA]...金牌"
}
```

### Core AggregatedSearchResult
```rust
AggregatedSearchResult {
    title:         String,
    thumbnail_url: Option<String>,
    detail_key:    String,   // passed through, Core does not interpret
    source:        String,   // fetcher name, e.g. "mikanani"
}
```

### DetailRequest / DetailResponse
```rust
DetailRequest {
    detail_key: String,
}

DetailResponse {
    items: Vec<DetailItem>,
}

DetailItem {
    subgroup_name: String,
    rss_url:       String,
}
```

---

## API Endpoints

### Fetcher (mikanani)

| Endpoint | Description |
|---|---|
| `POST /search` | Existing; add `detail_key` to response |
| `POST /detail` | **New**; receives `detail_key`, returns `DetailResponse` |

Fetcher `/detail` internal logic:
```
"bangumi:3822"         → scrape Home/Bangumi/3822 → return subgroup RSS list
"source:[KITA]...金牌" → search and group by subgroup → return RSS URLs
```

### Core Service

| Endpoint | Description |
|---|---|
| `GET /api/core/search?q=...` | Existing; pass through `detail_key`, add `source` |
| `POST /api/core/detail` | **New**; `{ detail_key, source }` → proxy to matching fetcher |

---

## Mikanani Scraper Changes

### `/search` — Parse Two Result Types

**Bangumi results:**
- `detail_key = "bangumi:{bangumi_id}"`

**Magnet source results:**
- Split title by `_`, take everything before the last `_` as `searchstr`
- `detail_key = "source:{searchstr}"`
- Example:
  ```
  "[KITA]...金牌得主19..._Ciallo"
              ↓ split at last _
  searchstr = "[KITA]...金牌得主19..."
  detail_key = "source:[KITA]...金牌得主19..."
  ```

### `/detail` — Two Internal Paths

**bangumi:{id}:**
```
GET mikanani.me/Home/Bangumi/{id}
→ parse subgroup list + subgroupid
→ return [
    { subgroup_name: "花山映画", rss_url: ".../RSS/Bangumi?bangumiId=3822&subgroupid=202" },
    { subgroup_name: "Root",    rss_url: ".../RSS/Bangumi?bangumiId=3822" },
  ]
```

**source:{searchstr}:**
```
GET mikanani.me/Home/Search?searchstr={searchstr}
→ parse results, group by subgroup
→ each group: compute RSS/Search?searchstr URL
→ return [
    { subgroup_name: "KITA", rss_url: ".../RSS/Search?searchstr=[KITA]...金牌" },
  ]
```

---

## Frontend Changes

### Schema (`schemas/search.ts`)
```typescript
type SearchResult = {
    title:         string
    thumbnail_url: string | null
    detail_key:    string
    source:        string
}

type DetailItem = {
    subgroup_name: string
    rss_url:       string
}

type DetailResponse = {
    items: DetailItem[]
}
```

### SearchPage
- Cards show thumbnail + title only (remove subscribe button)
- Click card → open `DetailDialog`

### New `DetailDialog` Component

```
┌────────────────────────────────────────────────────────────┐
│  [封面圖]  金牌得主                                          │
├────────────────────────────────────────────────────────────┤
│  字幕組       RSS 網址                             操作      │
│ ────────────────────────────────────────────────────────── │
│  花山映画  [.../RSS/Bangumi?...subgroupid=202↗]    [訂閱]   │
│  Root      [.../RSS/Bangumi?bangumiId=3822↗]       [訂閱]   │
│  KITA      [.../RSS/Search?searchstr=...↗]         [訂閱]   │
└────────────────────────────────────────────────────────────┘
```

- RSS URL is a clickable link: `window.open(url, '', 'noopener')` → opens **new window** (not tab)
- 「訂閱」button → opens existing subscription creation dialog, pre-filled with:
  - `source_url` = row's rss_url
  - `name` = `"{anime title} - {subgroup_name}"` (e.g. "金牌得主 - 花山映画")

### ApiLayer Addition
```typescript
getDetail: (detail_key: string, source: string) =>
    postJson("/api/core/detail", { detail_key, source }, DetailResponse)
```

---

## Files Affected

| Layer | File | Change |
|---|---|---|
| `mikanani` | `search_scraper.rs` | Parse magnet source section; add `detail_key` |
| `mikanani` | `handlers.rs` | Add `detail` handler |
| `mikanani` | `main.rs` | Add `/detail` route |
| `shared` | `models.rs` | Update `SearchResult` (add `detail_key`); add `DetailRequest/Response/Item` |
| `core` | `handlers/search.rs` | Pass through `detail_key`; add `source` field |
| `core` | `handlers/detail.rs` | **New** proxy handler |
| `core` | `main.rs` | Add `/api/core/detail` route |
| `frontend` | `schemas/search.ts` | Update schema |
| `frontend` | `pages/search/SearchPage.tsx` | Simplify card; add click handler |
| `frontend` | `pages/search/DetailDialog.tsx` | **New** component |
| `frontend` | `layers/ApiLayer.ts` | Add `getDetail` |
| `frontend` | `i18n/*.json` | Add translation keys |
