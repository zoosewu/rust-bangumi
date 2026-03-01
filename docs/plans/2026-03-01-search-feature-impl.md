# Search Feature Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a full-width search bar above the table on 7 pages; client-side pages filter all loaded data (cap 50 rows); RawItemsPage uses backend ILIKE search.

**Architecture:** Two new shared primitives (`SearchBar` component + `useTableSearch` hook) handle all client-side pages. RawItemsPage uses a debounced search state passed to the existing `getRawItems` API call, backed by a new backend `ILIKE` filter. All pages also gain a hard 50-row display cap (after filtering).

**Tech Stack:** React 19, TypeScript, Tailwind CSS v4, shadcn/Radix UI, Lucide icons, Diesel/Rust (backend)

---

## Task 1: Add i18n keys for search

**Files:**
- Modify: `frontend/src/i18n/en.json`
- Modify: `frontend/src/i18n/zh-TW.json`
- Modify: `frontend/src/i18n/ja.json`

**Step 1: Add keys to en.json**

In `frontend/src/i18n/en.json`, inside the `"common"` object (after the last existing key `"none": "None"`), add:

```json
    "search": "Search",
    "searchPlaceholder": "Search...",
    "noResults": "No results"
```

So the end of `"common"` becomes:

```json
    "allStatuses": "All Statuses",
    "none": "None",
    "search": "Search",
    "searchPlaceholder": "Search...",
    "noResults": "No results"
  },
```

**Step 2: Add keys to zh-TW.json**

Find the `"common"` object in `frontend/src/i18n/zh-TW.json`. Add after the last key:

```json
    "search": "搜尋",
    "searchPlaceholder": "搜尋...",
    "noResults": "無結果"
```

**Step 3: Add keys to ja.json**

Find the `"common"` object in `frontend/src/i18n/ja.json`. Add after the last key:

```json
    "search": "検索",
    "searchPlaceholder": "検索...",
    "noResults": "結果なし"
```

**Step 4: Commit**

```bash
git add frontend/src/i18n/en.json frontend/src/i18n/zh-TW.json frontend/src/i18n/ja.json
git commit -m "feat(i18n): add search-related translation keys"
```

---

## Task 2: Create the `useTableSearch` hook

**Files:**
- Create: `frontend/src/hooks/useTableSearch.ts`

**Step 1: Create the file**

```ts
// frontend/src/hooks/useTableSearch.ts
import { useMemo } from "react"

/**
 * Recursively stringify a value to a flat string for searching.
 * Handles primitives, arrays, and plain objects.
 */
function stringifyValue(val: unknown): string {
  if (val === null || val === undefined) return ""
  if (typeof val === "string") return val
  if (typeof val === "number" || typeof val === "boolean") return String(val)
  if (Array.isArray(val)) return val.map(stringifyValue).join(" ")
  if (typeof val === "object") return Object.values(val as Record<string, unknown>).map(stringifyValue).join(" ")
  return ""
}

/**
 * Generic client-side search hook.
 * Returns items matching the query (any field, case-insensitive), capped at 50.
 * If query is empty/whitespace, returns first 50 items unchanged.
 */
export function useTableSearch<T>(data: T[], query: string): T[] {
  return useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return data.slice(0, 50)
    return data
      .filter((item) => stringifyValue(item).toLowerCase().includes(q))
      .slice(0, 50)
  }, [data, query])
}
```

**Step 2: Verify the file exists and is correct**

```bash
cat frontend/src/hooks/useTableSearch.ts
```

Expected: file content as above.

**Step 3: Commit**

```bash
git add frontend/src/hooks/useTableSearch.ts
git commit -m "feat(hooks): add useTableSearch generic client-side search hook"
```

---

## Task 3: Create the `SearchBar` component

**Files:**
- Create: `frontend/src/components/shared/SearchBar.tsx`

**Step 1: Create the component**

```tsx
// frontend/src/components/shared/SearchBar.tsx
import { Search, X } from "lucide-react"
import { useTranslation } from "react-i18next"
import { Input } from "@/components/ui/input"

interface SearchBarProps {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  className?: string
}

export function SearchBar({ value, onChange, placeholder, className }: SearchBarProps) {
  const { t } = useTranslation()

  return (
    <div className={`relative${className ? ` ${className}` : ""}`}>
      <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder ?? t("common.searchPlaceholder")}
        className="pl-9 pr-9"
      />
      {value && (
        <button
          type="button"
          onClick={() => onChange("")}
          className="absolute right-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground hover:text-foreground transition-colors"
          aria-label="Clear search"
        >
          <X className="h-4 w-4" />
        </button>
      )}
    </div>
  )
}
```

**Step 2: Commit**

```bash
git add frontend/src/components/shared/SearchBar.tsx
git commit -m "feat(ui): add SearchBar shared component"
```

---

## Task 4: Backend — add `search` param to `list_raw_items`

**Files:**
- Modify: `core-service/src/handlers/raw_items.rs`

**Step 1: Add `search` field to `ListRawItemsQuery`**

Find this struct in `core-service/src/handlers/raw_items.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct ListRawItemsQuery {
    pub status: Option<String>,
    pub subscription_id: Option<i32>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
```

Replace with:

```rust
#[derive(Debug, Deserialize)]
pub struct ListRawItemsQuery {
    pub status: Option<String>,
    pub subscription_id: Option<i32>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub search: Option<String>,
}
```

**Step 2: Add ILIKE filter in `list_raw_items` handler**

Find this block in `list_raw_items` (after the `subscription_id` filter block):

```rust
    if let Some(sub_id) = query.subscription_id {
        q = q.filter(raw_anime_items::subscription_id.eq(sub_id));
    }

    let limit = query.limit.unwrap_or(100).min(1000);
```

Add between those two blocks:

```rust
    if let Some(sub_id) = query.subscription_id {
        q = q.filter(raw_anime_items::subscription_id.eq(sub_id));
    }

    if let Some(ref search) = query.search {
        q = q.filter(raw_anime_items::title.ilike(format!("%{}%", search)));
    }

    let limit = query.limit.unwrap_or(100).min(1000);
```

**Step 3: Verify it compiles**

```bash
cd /workspace && cargo check -p core-service 2>&1 | tail -5
```

Expected: `Finished` with no errors.

**Step 4: Commit**

```bash
git add core-service/src/handlers/raw_items.rs
git commit -m "feat(backend): add search param to list_raw_items with ILIKE filter"
```

---

## Task 5: Frontend API — thread `search` through CoreApi + ApiLayer

**Files:**
- Modify: `frontend/src/services/CoreApi.ts`
- Modify: `frontend/src/layers/ApiLayer.ts`

**Step 1: Update `CoreApi.ts`**

Find the `getRawItems` signature:

```ts
    readonly getRawItems: (params: {
      status?: string
      subscription_id?: number
      limit?: number
      offset?: number
    }) => Effect.Effect<readonly RawAnimeItem[]>
```

Replace with:

```ts
    readonly getRawItems: (params: {
      status?: string
      subscription_id?: number
      limit?: number
      offset?: number
      search?: string
    }) => Effect.Effect<readonly RawAnimeItem[]>
```

**Step 2: Update `ApiLayer.ts`**

Find in `getRawItems`:

```ts
      if (params.offset != null) qs.set("offset", String(params.offset))
      return fetchJson(
```

Replace with:

```ts
      if (params.offset != null) qs.set("offset", String(params.offset))
      if (params.search) qs.set("search", params.search)
      return fetchJson(
```

**Step 3: Commit**

```bash
git add frontend/src/services/CoreApi.ts frontend/src/layers/ApiLayer.ts
git commit -m "feat(api): add search param to getRawItems API contract and layer"
```

---

## Task 6: Apply search to `FiltersPage`

**Files:**
- Modify: `frontend/src/pages/filters/FiltersPage.tsx`

**Step 1: Add imports**

At the top of `FiltersPage.tsx`, add to existing imports:

```ts
import { useState } from "react"   // already imported, just confirm
import { SearchBar } from "@/components/shared/SearchBar"
import { useTableSearch } from "@/hooks/useTableSearch"
```

Note: `useState` is already imported. Only add the two new import lines.

**Step 2: Add search state**

Inside `FiltersPage()`, after the existing `useState` declarations, add:

```ts
  const [searchQuery, setSearchQuery] = useState("")
```

**Step 3: Apply search after sort**

Find:

```ts
  const sortedRules = useMemo(() => {
    ...
  }, [rules])
```

After that block, add:

```ts
  const filteredRules = useTableSearch(sortedRules, searchQuery)
```

**Step 4: Add SearchBar to JSX and use `filteredRules`**

Find the return JSX. After the header div and before the loading/data check, add `<SearchBar>`:

```tsx
  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("filters.title")}</h1>
        <Button onClick={() => setAddOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          {t("filters.addFilter")}
        </Button>
      </div>

      <SearchBar value={searchQuery} onChange={setSearchQuery} />

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : error ? (
        <p className="text-destructive text-sm">
          {t("common.error")}: {String(error)}
        </p>
      ) : filteredRules.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {searchQuery ? t("common.noResults") : t("filters.noRules", "No filter rules found.")}
        </p>
      ) : (
        <DataTable
          columns={columns}
          data={filteredRules as unknown as Record<string, unknown>[]}
          keyField="rule_id"
        />
      )}
      ...
```

Replace `sortedRules` with `filteredRules` in the `DataTable` data prop.

**Step 5: Commit**

```bash
git add frontend/src/pages/filters/FiltersPage.tsx
git commit -m "feat(filters): add search bar with client-side filtering"
```

---

## Task 7: Apply search to `SubtitleGroupsPage`

**Files:**
- Modify: `frontend/src/pages/subtitle-groups/SubtitleGroupsPage.tsx`

**Step 1: Add imports**

```ts
import { SearchBar } from "@/components/shared/SearchBar"
import { useTableSearch } from "@/hooks/useTableSearch"
```

**Step 2: Add search state inside `SubtitleGroupsPage()`**

```ts
  const [searchQuery, setSearchQuery] = useState("")
```

**Step 3: Derive filtered data**

After the `groups` query, add:

```ts
  const filteredGroups = useTableSearch(groups ?? [], searchQuery)
```

**Step 4: Update JSX**

Find the return JSX header div + data conditional. Insert `<SearchBar>` between the header and the data display:

```tsx
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("subtitleGroups.title")}</h1>
        <Button size="sm" onClick={() => setCreateOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          {t("subtitleGroups.addGroup")}
        </Button>
      </div>

      <SearchBar value={searchQuery} onChange={setSearchQuery} />

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : filteredGroups.length > 0 ? (
        <DataTable
          columns={columns}
          data={filteredGroups as unknown as Record<string, unknown>[]}
          keyField="group_id"
          onRowClick={...}
        />
      ) : (
        <p className="text-sm text-muted-foreground">
          {searchQuery ? t("common.noResults") : t("subtitleGroups.noGroups")}
        </p>
      )}
```

Replace `groups as unknown as Record<string, unknown>[]` with `filteredGroups as unknown as Record<string, unknown>[]`.

**Step 5: Commit**

```bash
git add frontend/src/pages/subtitle-groups/SubtitleGroupsPage.tsx
git commit -m "feat(subtitle-groups): add search bar with client-side filtering"
```

---

## Task 8: Apply search to `AnimeWorksPage`

**Files:**
- Modify: `frontend/src/pages/anime/AnimePage.tsx`

**Step 1: Add imports**

```ts
import { SearchBar } from "@/components/shared/SearchBar"
import { useTableSearch } from "@/hooks/useTableSearch"
```

**Step 2: Add search state**

```ts
  const [searchQuery, setSearchQuery] = useState("")
```

**Step 3: Derive filtered data**

```ts
  const filteredAnimes = useTableSearch(animes ?? [], searchQuery)
```

**Step 4: Update JSX**

Insert `<SearchBar>` between the header div and the loading/table block:

```tsx
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("anime.title")}</h1>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          {t("anime.addAnime")}
        </Button>
      </div>

      <SearchBar value={searchQuery} onChange={setSearchQuery} />

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <DataTable
          columns={columns}
          data={(filteredAnimes) as unknown as Record<string, unknown>[]}
          keyField="anime_id"
          onRowClick={(item) => {
            const found = (animes ?? []).find((a) => a.anime_id === item.anime_id)
            if (found) setSelectedAnime(found)
          }}
        />
      )}
```

Note: `onRowClick` still looks up from the full `animes` array to get the typed object.

**Step 5: Commit**

```bash
git add frontend/src/pages/anime/AnimePage.tsx
git commit -m "feat(anime-works): add search bar with client-side filtering"
```

---

## Task 9: Apply search to `ParsersPage`

**Files:**
- Modify: `frontend/src/pages/parsers/ParsersPage.tsx`

**Step 1: Add imports**

```ts
import { SearchBar } from "@/components/shared/SearchBar"
import { useTableSearch } from "@/hooks/useTableSearch"
```

**Step 2: Add search state**

```ts
  const [searchQuery, setSearchQuery] = useState("")
```

**Step 3: Derive filtered data — apply AFTER the existing sort**

After the existing `parsers` useMemo block, add:

```ts
  const filteredParsers = useTableSearch(parsers ?? [], searchQuery)
```

**Step 4: Update JSX**

In the return block, insert `<SearchBar>` between the header and the loading/table:

```tsx
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("parsers.title")}</h1>
        <Button onClick={() => setDialogOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          {t("parsers.addParser")}
        </Button>
      </div>

      <SearchBar value={searchQuery} onChange={setSearchQuery} />

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <DataTable
          columns={columns}
          data={(filteredParsers ?? []) as unknown as Record<string, unknown>[]}
          keyField="parser_id"
          onRowClick={...}
        />
      )}
```

Replace the existing `parsers` data prop with `filteredParsers`.

**Step 5: Commit**

```bash
git add frontend/src/pages/parsers/ParsersPage.tsx
git commit -m "feat(parsers): add search bar with client-side filtering"
```

---

## Task 10: Apply search to `ConflictsPage`

**Files:**
- Modify: `frontend/src/pages/conflicts/ConflictsPage.tsx`

**Step 1: Add imports**

```ts
import { useState } from "react"
import { SearchBar } from "@/components/shared/SearchBar"
import { useTableSearch } from "@/hooks/useTableSearch"
```

**Step 2: Add search state inside `ConflictsPage()`**

```ts
  const [searchQuery, setSearchQuery] = useState("")
```

**Step 3: Derive filtered conflicts**

```ts
  const filteredConflicts = useTableSearch(conflicts ?? [], searchQuery)
```

**Step 4: Update JSX**

Insert `<SearchBar>` between the title row and the table, and replace `conflicts.map(...)` with `filteredConflicts.map(...)`:

```tsx
  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <h1 className="text-2xl font-bold">{t("conflicts.title")}</h1>
        {conflicts && conflicts.length > 0 && (
          <Badge variant="destructive">{conflicts.length}</Badge>
        )}
      </div>

      {conflicts && conflicts.length > 0 && (
        <SearchBar value={searchQuery} onChange={setSearchQuery} />
      )}

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : !conflicts?.length ? (
        <Card>...</Card>
      ) : filteredConflicts.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("common.noResults")}</p>
      ) : (
        <div className="rounded-md border">
          <table className="w-full text-sm">
            ...
            <tbody className="divide-y">
              {filteredConflicts.map((c) => (
                ...
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
```

Note: The badge still shows the total unfiltered conflict count.

**Step 5: Commit**

```bash
git add frontend/src/pages/conflicts/ConflictsPage.tsx
git commit -m "feat(conflicts): add search bar with client-side filtering"
```

---

## Task 11: Apply search to `AnimeSeriesPage` (Grid + List)

**Files:**
- Modify: `frontend/src/pages/anime-series/AnimeSeriesPage.tsx`

**Step 1: Add imports**

```ts
import { SearchBar } from "@/components/shared/SearchBar"
import { useTableSearch } from "@/hooks/useTableSearch"
```

**Step 2: Add search state**

```ts
  const [searchQuery, setSearchQuery] = useState("")
```

**Step 3: Derive filtered data**

```ts
  const filteredList = useTableSearch(seriesList ?? [], searchQuery)
```

**Step 4: Update JSX**

Insert `<SearchBar>` between the header row and the content area. Use `filteredList` in both Grid and List modes:

```tsx
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("animeSeries.title", "Anime Seasons")}</h1>
        <div className="flex items-center gap-1">
          {/* view mode buttons unchanged */}
        </div>
      </div>

      <SearchBar value={searchQuery} onChange={setSearchQuery} />

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : viewMode === "grid" ? (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4">
          {filteredList.map((series) => (
            <AnimeCard
              key={series.series_id}
              series={series}
              onClick={() => setSelected(series)}
            />
          ))}
        </div>
      ) : (
        <DataTable
          columns={columns}
          data={filteredList as unknown as Record<string, unknown>[]}
          keyField="series_id"
          onRowClick={(row) => {
            const rich = (seriesList ?? []).find((s) => s.series_id === row.series_id)
            if (rich) setSelected(rich)
          }}
        />
      )}
```

Note: `onRowClick` still looks up from the full `seriesList` to get the typed `AnimeRich` object.

**Step 5: Commit**

```bash
git add frontend/src/pages/anime-series/AnimeSeriesPage.tsx
git commit -m "feat(anime-series): add search bar filtering both grid and list modes"
```

---

## Task 12: Apply backend search to `RawItemsPage`

**Files:**
- Modify: `frontend/src/pages/raw-items/RawItemsPage.tsx`

**Step 1: Add imports**

```ts
import { useEffect } from "react"   // already imported; confirm it's there
import { SearchBar } from "@/components/shared/SearchBar"
```

**Step 2: Add search state**

Inside `RawItemsPage()`, after existing `useState` declarations, add:

```ts
  const [rawSearch, setRawSearch] = useState("")
  const [debouncedSearch, setDebouncedSearch] = useState("")
```

**Step 3: Debounce effect**

```ts
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(rawSearch)
      setOffset(0)
    }, 300)
    return () => clearTimeout(timer)
  }, [rawSearch])
```

**Step 4: Pass `search` and limit to the API call**

Find:

```ts
    return yield* api.getRawItems({
      status: status === "all" ? undefined : status,
      limit: PAGE_SIZE,
      offset,
    })
```

Replace with:

```ts
    return yield* api.getRawItems({
      status: status === "all" ? undefined : status,
      limit: PAGE_SIZE,
      offset,
      search: debouncedSearch || undefined,
    })
```

Also add `debouncedSearch` to the `useEffectQuery` dependency array:

```ts
    [status, offset, debouncedSearch],
```

**Step 5: Add `SearchBar` in header row next to status filter**

Find the header div with the `<Select>` for status filter:

```tsx
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("rawItems.title")}</h1>
        <div className="flex items-center gap-4">
          <Select ...>...</Select>
        </div>
      </div>
```

Replace with:

```tsx
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("rawItems.title")}</h1>
        <div className="flex items-center gap-4">
          <Select ...>...</Select>
        </div>
      </div>

      <SearchBar value={rawSearch} onChange={setRawSearch} />
```

**Step 6: Commit**

```bash
git add frontend/src/pages/raw-items/RawItemsPage.tsx
git commit -m "feat(raw-items): add debounced backend search via ILIKE"
```

---

## Task 13: Build verification

**Step 1: Check TypeScript compiles without errors**

```bash
cd /workspace/frontend && npx tsc --noEmit 2>&1 | tail -20
```

Expected: No errors.

**Step 2: Check Rust backend compiles**

```bash
cd /workspace && cargo check -p core-service 2>&1 | tail -5
```

Expected: `Finished` with no errors.

**Step 3: Spot-check the frontend dev build**

```bash
cd /workspace/frontend && npm run build 2>&1 | tail -10
```

Expected: `built in Xs` with no errors.

**Step 4: Final commit if any fixes needed**

If TypeScript errors arise (e.g., import paths, type mismatches), fix them and commit:

```bash
git add -p
git commit -m "fix(search): resolve TypeScript type issues in search integration"
```

---

## Summary of Files Changed

| File | Change |
|------|--------|
| `frontend/src/i18n/en.json` | Add `common.search`, `common.searchPlaceholder`, `common.noResults` |
| `frontend/src/i18n/zh-TW.json` | Same |
| `frontend/src/i18n/ja.json` | Same |
| `frontend/src/hooks/useTableSearch.ts` | **New** — generic client-side search hook |
| `frontend/src/components/shared/SearchBar.tsx` | **New** — search input component |
| `core-service/src/handlers/raw_items.rs` | Add `search: Option<String>` + ILIKE filter |
| `frontend/src/services/CoreApi.ts` | Add `search?: string` to `getRawItems` params |
| `frontend/src/layers/ApiLayer.ts` | Pass `search` to querystring |
| `frontend/src/pages/filters/FiltersPage.tsx` | SearchBar + useTableSearch |
| `frontend/src/pages/subtitle-groups/SubtitleGroupsPage.tsx` | SearchBar + useTableSearch |
| `frontend/src/pages/anime/AnimePage.tsx` | SearchBar + useTableSearch |
| `frontend/src/pages/parsers/ParsersPage.tsx` | SearchBar + useTableSearch |
| `frontend/src/pages/conflicts/ConflictsPage.tsx` | SearchBar + useTableSearch |
| `frontend/src/pages/anime-series/AnimeSeriesPage.tsx` | SearchBar + useTableSearch (grid + list) |
| `frontend/src/pages/raw-items/RawItemsPage.tsx` | SearchBar + debounced backend search |
