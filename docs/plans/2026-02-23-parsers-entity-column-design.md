# ParsersPage Entity Column Design

**Goal:** 在 ParsersPage 的解析器列表中新增一個欄位，顯示該解析器所屬實體的名稱，點擊後開啟對應實體的 Dialog。

**Architecture:**
後端以 raw SQL LEFT JOIN 在單一查詢中計算 `created_from_name`（依 `created_from_type` 分別 JOIN animes、anime_series、subtitle_groups、subscriptions）。前端在 ParsersPage 新增 Entity 欄位，點擊後依型別 fetch 完整實體資料並開啟對應 Dialog（AnimeDialog / AnimeSeriesDialog / SubtitleGroupDialog / SubscriptionDialog）。

**Tech Stack:** Rust (Diesel `sql_query` + `QueryableByName`), React/TypeScript (Effect.ts, DataTable)

---

## Data Flow

```
ParsersPage
  → GET /parsers?created_from_type=global
  ← [{ ..., created_from_type, created_from_id, created_from_name }]

User clicks "Entity" column
  → fetch full entity data (endpoint depends on type)
  → open corresponding Dialog
```

## Backend

**File:** `core-service/src/handlers/parsers.rs`

### `ParserResponse` 新增欄位
```rust
created_from_name: Option<String>,
```

### `list_parsers` 改為 raw SQL

新增 `#[derive(QueryableByName)]` struct `ParserRow` 承接 raw query，再轉成 `ParserResponse`。

```sql
SELECT tp.*,
  CASE tp.created_from_type
    WHEN 'global'         THEN 'Global'
    WHEN 'anime'          THEN a.title
    WHEN 'anime_series'   THEN a2.title || ' S' || s.series_no::text
    WHEN 'subtitle_group' THEN sg.group_name
    WHEN 'subscription'   THEN COALESCE(sub.name, '#' || sub.subscription_id::text)
    ELSE NULL
  END AS created_from_name
FROM title_parsers tp
LEFT JOIN animes a
  ON tp.created_from_type = 'anime' AND a.anime_id = tp.created_from_id
LEFT JOIN anime_series s
  ON tp.created_from_type = 'anime_series' AND s.series_id = tp.created_from_id
LEFT JOIN animes a2
  ON tp.created_from_type = 'anime_series' AND a2.anime_id = s.anime_id
LEFT JOIN subtitle_groups sg
  ON tp.created_from_type = 'subtitle_group' AND sg.group_id = tp.created_from_id
LEFT JOIN subscriptions sub
  ON tp.created_from_type = 'subscription' AND sub.subscription_id = tp.created_from_id
WHERE ...   -- existing filter conditions unchanged
ORDER BY tp.priority DESC
```

## Frontend

**Files:**
- `frontend/src/schemas/parser.ts` — 加 `created_from_name`
- `frontend/src/pages/parsers/ParsersPage.tsx` — 新增欄位 + Dialog 狀態

### Schema
```typescript
created_from_name: Schema.NullOr(Schema.String)
```

### ParsersPage state
```typescript
type SelectedEntity =
  | { type: "anime"; id: number }
  | { type: "anime_series"; id: number }
  | { type: "subtitle_group"; id: number; name: string }
  | { type: "subscription"; id: number }

const [selectedEntity, setSelectedEntity] = useState<SelectedEntity | null>(null)
```

### Entity 欄位 render
```typescript
{
  key: "created_from_name",
  header: t("parsers.entity", "Entity"),
  render: (row) =>
    row.created_from_type === "global" ? (
      <span className="text-muted-foreground text-xs">Global</span>
    ) : (
      <button className="text-xs underline hover:opacity-70"
        onClick={() => handleEntityClick(row)}>
        {row.created_from_name ?? `#${row.created_from_id}`}
      </button>
    ),
}
```

### handleEntityClick + Dialog open logic
- `subtitle_group` → 直接用 id + name，不需 API call → 開 `SubtitleGroupDialog`
- `anime` → `api.getAnime(id)` → 開 `AnimeDialog`
- `anime_series` → `api.getAnimeSeries(id)` → 開 `AnimeSeriesDialog`
- `subscription` → `api.getSubscription(id)` → 開 `SubscriptionDialog`

### Dialog 渲染（條件式）
```tsx
{selectedEntity?.type === "subtitle_group" && subtitleGroupData && (
  <SubtitleGroupDialog groupId={...} groupName={...} open onOpenChange={...} />
)}
{selectedEntity?.type === "anime" && animeData && (
  <AnimeDialog anime={animeData} open onOpenChange={...} />
)}
// etc.
```
