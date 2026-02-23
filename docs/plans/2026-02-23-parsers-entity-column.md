# ParsersPage Entity Column Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 ParsersPage 的解析器列表中新增「Entity」欄位，顯示解析器所屬實體的名稱，點擊後開啟對應實體的 Dialog。

**Architecture:** 後端在 `list_parsers` 中額外做 4 次批次 Diesel 查詢（按 type 分組）取得實體名稱，附加到 `ParserResponse.created_from_name`。前端在 ParsersPage 新增 Entity 欄位；點擊時以 `AppRuntime.runPromise` 懶惰獲取完整實體資料，再開啟對應 Dialog（AnimeDialog / AnimeSeriesDialog / SubtitleGroupDialog / SubscriptionDialog）。

**Tech Stack:** Rust (Diesel ORM, axum), React/TypeScript (Effect.ts, AppRuntime)

---

### Task 1: 後端 — `ParserResponse` 加入 `created_from_name`

**Files:**
- Modify: `core-service/src/handlers/parsers.rs`

**背景：**
`ParserResponse` 目前有 `created_from_type: Option<String>` 和 `created_from_id: Option<i32>`，但沒有名稱。`list_parsers` 以 Diesel 查詢取得 parsers 後，需要額外批次查詢各實體表取得名稱。

以下是批次查詢策略（分 4 種 type 分別查詢對應資料表，避免 id 碰撞）：
- `anime` → `animes` 表，key = `anime_id`
- `anime_series` → `anime_series INNER JOIN animes`，key = `series_id`，值 = `"{anime_title} S{series_no}"`
- `subtitle_group` → `subtitle_groups` 表，key = `group_id`
- `subscription` / `fetcher` → `subscriptions` 表，key = `subscription_id`，值 = `name` 或 `"#{id}"`
- `global` → 不需查詢，直接回傳 `Some("Global".to_string())`

**Step 1: 在 `ParserResponse` struct 加入 `created_from_name` 欄位**

找到 `parsers.rs` 第 74–101 行的 `ParserResponse` struct，在 `created_from_id` 後加一行：

```rust
pub created_from_name: Option<String>,
```

**Step 2: 在 `From<TitleParser> for ParserResponse` 加入預設 `None`**

找到 `impl From<TitleParser> for ParserResponse`（第 103–133 行），在最後的 `}` 前（`created_from_id: p.created_from_id,` 之後）加：

```rust
created_from_name: None,
```

**Step 3: 在檔案末尾加入 `resolve_parser_names` helper**

在 `cleanup_empty_series` 函式之後（約第 851 行後），加入以下函式：

```rust
/// 批次查詢各實體名稱，分別按 created_from_type 分組。
/// 回傳值為 4 個獨立的 HashMap，以各自的主鍵對應名稱。
fn resolve_parser_names(
    conn: &mut diesel::PgConnection,
    parsers: &[TitleParser],
) -> (
    std::collections::HashMap<i32, String>, // anime_id -> title
    std::collections::HashMap<i32, String>, // series_id -> "Title S{n}"
    std::collections::HashMap<i32, String>, // group_id -> group_name
    std::collections::HashMap<i32, String>, // subscription_id -> name
) {
    use crate::schema::{animes, anime_series, subtitle_groups, subscriptions};

    let mut anime_ids: Vec<i32> = vec![];
    let mut series_ids: Vec<i32> = vec![];
    let mut group_ids: Vec<i32> = vec![];
    let mut sub_ids: Vec<i32> = vec![];

    for p in parsers {
        if let (Some(t), Some(id)) = (&p.created_from_type, p.created_from_id) {
            match t {
                FilterTargetType::Anime => anime_ids.push(id),
                FilterTargetType::AnimeSeries => series_ids.push(id),
                FilterTargetType::SubtitleGroup => group_ids.push(id),
                FilterTargetType::Subscription | FilterTargetType::Fetcher => sub_ids.push(id),
                FilterTargetType::Global => {}
            }
        }
    }

    let mut anime_names: std::collections::HashMap<i32, String> = Default::default();
    let mut series_names: std::collections::HashMap<i32, String> = Default::default();
    let mut group_names: std::collections::HashMap<i32, String> = Default::default();
    let mut sub_names: std::collections::HashMap<i32, String> = Default::default();

    if !anime_ids.is_empty() {
        if let Ok(rows) = animes::table
            .filter(animes::anime_id.eq_any(&anime_ids))
            .select((animes::anime_id, animes::title))
            .load::<(i32, String)>(conn)
        {
            for (id, title) in rows {
                anime_names.insert(id, title);
            }
        }
    }

    if !series_ids.is_empty() {
        if let Ok(rows) = anime_series::table
            .inner_join(animes::table)
            .filter(anime_series::series_id.eq_any(&series_ids))
            .select((anime_series::series_id, animes::title, anime_series::series_no))
            .load::<(i32, String, i32)>(conn)
        {
            for (id, title, series_no) in rows {
                series_names.insert(id, format!("{} S{}", title, series_no));
            }
        }
    }

    if !group_ids.is_empty() {
        if let Ok(rows) = subtitle_groups::table
            .filter(subtitle_groups::group_id.eq_any(&group_ids))
            .select((subtitle_groups::group_id, subtitle_groups::group_name))
            .load::<(i32, String)>(conn)
        {
            for (id, name) in rows {
                group_names.insert(id, name);
            }
        }
    }

    if !sub_ids.is_empty() {
        if let Ok(rows) = subscriptions::table
            .filter(subscriptions::subscription_id.eq_any(&sub_ids))
            .select((subscriptions::subscription_id, subscriptions::name))
            .load::<(i32, Option<String>)>(conn)
        {
            for (id, name) in rows {
                sub_names.insert(id, name.unwrap_or_else(|| format!("#{}", id)));
            }
        }
    }

    (anime_names, series_names, group_names, sub_names)
}
```

**Step 4: 修改 `list_parsers` 呼叫 `resolve_parser_names` 並附加名稱**

找到 `list_parsers` 函式（第 144–175 行），將 `Ok(Json(...))` 前的部分修改如下：

原本：
```rust
    let parsers = q
        .load::<TitleParser>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        parsers.into_iter().map(ParserResponse::from).collect(),
    ))
```

改為：
```rust
    let parsers = q
        .load::<TitleParser>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (anime_names, series_names, group_names, sub_names) =
        resolve_parser_names(&mut conn, &parsers);

    let responses: Vec<ParserResponse> = parsers
        .into_iter()
        .map(|p| {
            let type_ref = p.created_from_type.as_ref();
            let id = p.created_from_id;
            let name = match (type_ref, id) {
                (Some(FilterTargetType::Global), _) => Some("Global".to_string()),
                (Some(FilterTargetType::Anime), Some(id)) => anime_names.get(&id).cloned(),
                (Some(FilterTargetType::AnimeSeries), Some(id)) => series_names.get(&id).cloned(),
                (Some(FilterTargetType::SubtitleGroup), Some(id)) => group_names.get(&id).cloned(),
                (Some(FilterTargetType::Subscription), Some(id))
                | (Some(FilterTargetType::Fetcher), Some(id)) => sub_names.get(&id).cloned(),
                _ => None,
            };
            let mut resp = ParserResponse::from(p);
            resp.created_from_name = name;
            resp
        })
        .collect();

    Ok(Json(responses))
```

**Step 5: 確認編譯通過**

```bash
cd /workspace/core-service && cargo build 2>&1 | head -50
```

Expected: 無錯誤，可能有 warning 但不影響。

**Step 6: Commit**

```bash
git add core-service/src/handlers/parsers.rs
git commit -m "feat(api): add created_from_name to ParserResponse via batch lookup"
```

---

### Task 2: 前端 — Schema 加入 `created_from_name`

**Files:**
- Modify: `frontend/src/schemas/parser.ts`

**Step 1: 在 `TitleParser` schema 加入 `created_from_name`**

找到 `parser.ts` 第 25–26 行：
```typescript
  created_from_type: Schema.NullOr(Schema.String),
  created_from_id: Schema.NullOr(Schema.Number),
```

改為：
```typescript
  created_from_type: Schema.NullOr(Schema.String),
  created_from_id: Schema.NullOr(Schema.Number),
  created_from_name: Schema.NullOr(Schema.String),
```

**Step 2: 確認 TypeScript 型別無誤**

```bash
cd /workspace/frontend && npx tsc --noEmit 2>&1 | head -30
```

Expected: 無錯誤（或只有與此次無關的既有警告）。

**Step 3: Commit**

```bash
git add frontend/src/schemas/parser.ts
git commit -m "feat(frontend): add created_from_name to TitleParser schema"
```

---

### Task 3: 前端 — ParsersPage 加入 Entity 欄位與 Dialog

**Files:**
- Modify: `frontend/src/pages/parsers/ParsersPage.tsx`

**背景：**
ParsersPage 的 `columns` 陣列目前共 5 欄（id、name、priority、condition、enabled）加 actions。
本任務新增第 6 欄「Entity」，在 `is_enabled` 欄位和 `actions` 之間插入。

各 Dialog 的 props：
- `SubtitleGroupDialog` — `{ groupId: number, groupName: string, open, onOpenChange }`
- `AnimeDialog` — `{ anime: Anime, open, onOpenChange }`
- `AnimeSeriesDialog` — `{ series: AnimeSeriesRich, open, onOpenChange }`
- `SubscriptionDialog` — `{ subscription: Subscription, open, onOpenChange }`

**Step 1: 加入 import**

在現有 imports 下方加入：

```typescript
import { AnimeDialog } from "@/pages/anime/AnimeDialog"
import { AnimeSeriesDialog } from "@/pages/anime-series/AnimeSeriesDialog"
import { SubtitleGroupDialog } from "@/pages/subtitle-groups/SubtitleGroupDialog"
import { SubscriptionDialog } from "@/pages/subscriptions/SubscriptionDialog"
import type { Anime, AnimeSeriesRich } from "@/schemas/anime"
import type { Subscription } from "@/schemas/subscription"
import { AppRuntime } from "@/runtime/AppRuntime"
```

注意：`AppRuntime` 已在第 24 行被 import，不要重複。確認 `Anime`, `AnimeSeriesRich`, `Subscription` 是否尚未 import，若已有則跳過。

**Step 2: 加入 `entityDialog` state**

在 `ParsersPage` 函式內、現有 `const [deleteTarget, ...]` 行後面加入：

```typescript
type EntityDialog =
  | { type: "subtitle_group"; id: number; name: string }
  | { type: "anime"; data: Anime }
  | { type: "anime_series"; data: AnimeSeriesRich }
  | { type: "subscription"; data: Subscription }

const [entityDialog, setEntityDialog] = useState<EntityDialog | null>(null)
```

**Step 3: 加入 `handleEntityClick` 函式**

在 `handleSave` 函式之後加入：

```typescript
const handleEntityClick = useCallback(async (row: Record<string, unknown>) => {
  const type = row.created_from_type as string | null
  const id = row.created_from_id as number | null
  const name = row.created_from_name as string | null
  if (!type || !id) return

  if (type === "subtitle_group") {
    setEntityDialog({ type: "subtitle_group", id, name: name ?? `#${id}` })
  } else if (type === "anime") {
    const animes = await AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) => api.getAnimes),
    ).catch(() => null)
    const anime = animes?.find((a: Anime) => a.anime_id === id)
    if (anime) setEntityDialog({ type: "anime", data: anime })
  } else if (type === "anime_series") {
    const allSeries = await AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) => api.getAllAnimeSeries),
    ).catch(() => null)
    const series = allSeries?.find((s: AnimeSeriesRich) => s.series_id === id)
    if (series) setEntityDialog({ type: "anime_series", data: series })
  } else if (type === "subscription" || type === "fetcher") {
    const subs = await AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) => api.getSubscriptions),
    ).catch(() => null)
    const sub = subs?.find((s: Subscription) => s.subscription_id === id)
    if (sub) setEntityDialog({ type: "subscription", data: sub })
  }
}, [])
```

**Step 4: 在 `columns` 陣列中插入 Entity 欄位**

找到 `columns` 陣列中 `is_enabled` 欄位與 `actions` 欄位之間（第 155–178 行），在 `is_enabled` 物件後、`actions` 物件前插入：

```typescript
{
  key: "created_from_name",
  header: t("parsers.entity", "Entity"),
  render: (item) => {
    const type = item.created_from_type as string | null
    const name = item.created_from_name as string | null
    const id = item.created_from_id as number | null
    if (!type || type === "global") {
      return <span className="text-muted-foreground text-xs">Global</span>
    }
    return (
      <button
        type="button"
        className="text-xs underline hover:opacity-70 text-left"
        onClick={(e) => {
          e.stopPropagation()
          handleEntityClick(item)
        }}
      >
        {name ?? `#${id}`}
      </button>
    )
  },
},
```

**Step 5: 在 JSX 末尾加入 Dialog 渲染**

在 `</div>` 最後關閉標籤之前（delete confirm dialog 之後），加入：

```tsx
{/* Entity dialogs */}
{entityDialog?.type === "subtitle_group" && (
  <SubtitleGroupDialog
    groupId={entityDialog.id}
    groupName={entityDialog.name}
    open={true}
    onOpenChange={(open) => { if (!open) setEntityDialog(null) }}
  />
)}
{entityDialog?.type === "anime" && (
  <AnimeDialog
    anime={entityDialog.data}
    open={true}
    onOpenChange={(open) => { if (!open) setEntityDialog(null) }}
  />
)}
{entityDialog?.type === "anime_series" && (
  <AnimeSeriesDialog
    series={entityDialog.data}
    open={true}
    onOpenChange={(open) => { if (!open) setEntityDialog(null) }}
  />
)}
{entityDialog?.type === "subscription" && (
  <SubscriptionDialog
    subscription={entityDialog.data}
    open={true}
    onOpenChange={(open) => { if (!open) setEntityDialog(null) }}
  />
)}
```

**Step 6: 確認 TypeScript 型別無誤**

```bash
cd /workspace/frontend && npx tsc --noEmit 2>&1 | head -30
```

Expected: 無錯誤。

**Step 7: Commit**

```bash
git add frontend/src/pages/parsers/ParsersPage.tsx
git commit -m "feat(frontend): add entity column with dialog click to ParsersPage"
```
