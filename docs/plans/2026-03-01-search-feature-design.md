# Search Feature Design

**Date**: 2026-03-01
**Status**: Approved

## Overview

Add a search bar to all 7 major table pages in the frontend. Search filters any field value (case-insensitive). The search bar is displayed as a full-width banner between the page title row and the table.

Additionally, all pages limit display to a maximum of 50 rows (applied after search filtering for client-side pages).

---

## Affected Pages

| Page | Route | Table Type | Search Method |
|------|-------|-----------|---------------|
| 動畫系列 (AnimeSeriesPage) | `/anime` | DataTable + Grid cards | Client-side |
| 動畫作品 (AnimeWorksPage) | `/anime-works` | DataTable | Client-side |
| 衝突 (ConflictsPage) | `/conflicts` | Native HTML table | Client-side |
| 字幕組 (SubtitleGroupsPage) | `/subtitle-groups` | DataTable | Client-side |
| 解析器 (ParsersPage) | `/parsers` | DataTable | Client-side |
| 篩選器 (FiltersPage) | `/filters` | DataTable | Client-side |
| 最新更新 (RawItemsPage) | `/raw-items` | DataTable | Backend API (ILIKE) |

---

## Architecture

### Client-side search flow (6 pages)

```
API response (all data)
  → useTableSearch(data, query)   ← generic hook, memoized
    → filter: stringify all values, case-insensitive includes
    → slice(0, 50)                ← display cap
  → DataTable / native table / Grid cards
```

### Server-side search flow (RawItemsPage)

```
SearchBar input (debounced 300ms)
  → getRawItems({ search, status, limit: 50, offset: 0 })
  → Backend: title ILIKE %search%
  → DataTable with existing pagination
```

---

## New Shared Components

### `src/components/shared/SearchBar.tsx`

```tsx
interface SearchBarProps {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  className?: string
}
```

- Full-width `Input` with `Search` icon (Lucide) on the left
- Clear `X` button shown only when `value` is non-empty
- No border radius adjustment needed — uses existing Input styling

### `src/hooks/useTableSearch.ts`

```ts
function useTableSearch<T>(data: T[], query: string): T[]
```

- Wrapped in `useMemo([data, query])`
- If `query` is empty/whitespace → return `data.slice(0, 50)`
- Otherwise: for each item, recursively stringify all values (handles nested objects/arrays)
- Filter items where any stringified value contains `query.toLowerCase()`
- Return `filtered.slice(0, 50)`

---

## UI Layout

```
┌─────────────────────────────────────────┐
│ [Page Title]            [Action Button] │
│                                         │
│ [🔍 Search...                      ] [×]│  ← SearchBar (full width)
│                                         │
│ ┌─────────┬──────────┬─────────────────┐│
│ │ Col A   │ Col B    │ Col C           ││
│ ├─────────┼──────────┼─────────────────┤│
│ │ ...     │ ...      │ ...             ││
│ └─────────┴──────────┴─────────────────┘│
│                                         │
│ Showing N results                       │  ← result count hint (optional)
└─────────────────────────────────────────┘
```

---

## Backend Changes

### `core-service/src/handlers/raw_items.rs`

Add `search` field to `ListRawItemsQuery`:

```rust
pub struct ListRawItemsQuery {
    pub status: Option<String>,
    pub subscription_id: Option<i32>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub search: Option<String>,  // NEW
}
```

Add Diesel filter in `list_raw_items` handler:

```rust
if let Some(ref search) = query.search {
    q = q.filter(raw_anime_items::title.ilike(format!("%{}%", search)));
}
```

---

## Frontend API Changes

### `src/services/CoreApi.ts`

```ts
readonly getRawItems: (params: {
  status?: string
  subscription_id?: number
  limit?: number
  offset?: number
  search?: string  // NEW
}) => Effect.Effect<readonly RawAnimeItem[]>
```

### `src/layers/ApiLayer.ts`

```ts
getRawItems: (params) => {
  const qs = new URLSearchParams()
  if (params.status) qs.set("status", params.status)
  if (params.subscription_id != null) qs.set("subscription_id", String(params.subscription_id))
  if (params.limit != null) qs.set("limit", String(params.limit))
  if (params.offset != null) qs.set("offset", String(params.offset))
  if (params.search) qs.set("search", params.search)  // NEW
  ...
}
```

---

## i18n Additions

Add to `en.json`, `zh-TW.json`, `ja.json` under `"common"`:

| Key | en | zh-TW | ja |
|-----|----|-------|-----|
| `search` | `"Search"` | `"搜尋"` | `"検索"` |
| `searchPlaceholder` | `"Search..."` | `"搜尋..."` | `"検索..."` |

---

## Page-by-Page Changes

### AnimeSeriesPage (`/anime`)
- Add `useState("")` for search query
- Add `SearchBar` below title row
- Apply `useTableSearch(seriesList, query)` to get filtered+capped list
- Grid mode: render filtered cards
- List mode: pass filtered to `DataTable`

### AnimeWorksPage (`/anime-works`)
- Add `SearchBar` below title row
- Apply `useTableSearch(animes, query)`

### ConflictsPage (`/conflicts`)
- Add `SearchBar` below title row
- Apply `useTableSearch(conflicts, query)` before rendering the native `<table>`

### SubtitleGroupsPage (`/subtitle-groups`)
- Add `SearchBar` below title row
- Apply `useTableSearch(groups, query)`

### ParsersPage (`/parsers`)
- Add `SearchBar` below title row
- Apply `useTableSearch(parsers, query)`

### FiltersPage (`/filters`)
- Add `SearchBar` below title row
- Apply `useTableSearch(sortedRules, query)` (applied after existing sort)

### RawItemsPage (`/raw-items`)
- Add `useState("")` for `rawSearch` and `useState("")` for `debouncedSearch`
- useEffect debounce (300ms): `debouncedSearch` updates from `rawSearch`
- Pass `debouncedSearch` as `search` param to `getRawItems`
- On `rawSearch` change: reset `offset` to 0
- Add `SearchBar` adjacent to existing status filter (in the header row)
- Keep existing pagination UI

---

## Out of Scope

- Dashboard page: no table, no search needed
- Subscriptions page: not listed in requirements
- Server-side search for client-side pages (unnecessary given typical dataset sizes)
