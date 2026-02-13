# Subscription Management & Naming Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add subscription create/delete functionality and rename "動畫季度"→"動畫", "動畫"→"動畫作品" across all UI languages.

**Architecture:** Backend changes are minimal — add a raw-items count endpoint, change delete subscription to use ID and cascade-delete pending/failed raw items. Frontend adds create/delete UI to SubscriptionsPage following the existing AnimePage pattern.

**Tech Stack:** Rust/Axum/Diesel (backend), React/TypeScript/Effect (frontend), i18next (i18n)

---

### Task 1: Update i18n translation files

**Files:**
- Modify: `frontend/src/i18n/en.json`
- Modify: `frontend/src/i18n/zh-TW.json`
- Modify: `frontend/src/i18n/ja.json`

**Step 1: Update English translations**

In `frontend/src/i18n/en.json`, make these changes:

```json
"sidebar": {
  "animeSeries": "Anime",          // was "Anime Seasons"
  "anime": "Anime Titles",         // was "Anime"
}
"dashboard": {
  "totalAnime": "Anime Titles",    // was "Anime"
  "totalSeries": "Anime",          // was "Seasons"
}
"animeSeries": {
  "title": "Anime",                // was "Anime Seasons"
}
"anime": {
  "title": "Anime Titles",         // was "Anime"
  "addAnime": "Add Anime Title",   // was "Add Anime"
  "animeTitle": "Title",           // was "Anime title"
  "deleteAnime": "Delete Anime Title",  // was "Delete Anime"
  "notFound": "Anime title not found",  // was "Anime not found"
  "noRules": "No filter rules for this anime title.",  // was "...this anime."
}
```

Add new subscription keys:

```json
"subscriptions": {
  // ... existing keys ...
  "addSubscription": "Add Subscription",
  "deleteSubscription": "Delete Subscription",
  "deleteConfirm": "Are you sure you want to delete \"{{name}}\"? This will also delete {{count}} pending/failed raw items.",
  "name": "Name",
  "fetchInterval": "Fetch Interval (min)"
}
```

**Step 2: Update Traditional Chinese translations**

In `frontend/src/i18n/zh-TW.json`:

```json
"sidebar": {
  "animeSeries": "動畫",           // was "動畫季度"
  "anime": "動畫作品",             // was "動畫"
}
"dashboard": {
  "totalAnime": "動畫作品",        // was "動畫"
  "totalSeries": "動畫",           // was "季度"
}
"animeSeries": {
  "title": "動畫",                 // was "動畫季度"
}
"anime": {
  "title": "動畫作品",             // was "動畫"
  "addAnime": "新增動畫作品",       // was "新增動畫"
  "animeTitle": "標題",            // was "動畫標題"
  "deleteAnime": "刪除動畫作品",    // was "刪除動畫"
  "notFound": "找不到動畫作品",     // was "找不到動畫"
  "noRules": "此動畫作品沒有篩選規則。",  // was "此動畫沒有篩選規則。"
}
```

Add new subscription keys:

```json
"subscriptions": {
  // ... existing keys ...
  "addSubscription": "新增訂閱",
  "deleteSubscription": "刪除訂閱",
  "deleteConfirm": "確定要刪除「{{name}}」嗎？將同時刪除 {{count}} 筆未完成的原始項目。",
  "name": "名稱",
  "fetchInterval": "擷取間隔（分鐘）"
}
```

**Step 3: Update Japanese translations**

In `frontend/src/i18n/ja.json`:

```json
"sidebar": {
  "animeSeries": "アニメ",          // was "アニメシーズン"
  "anime": "アニメ作品",            // was "アニメ"
}
"dashboard": {
  "totalAnime": "アニメ作品",       // was "アニメ"
  "totalSeries": "アニメ",          // was "シーズン"
}
"animeSeries": {
  "title": "アニメ",                // was "アニメシーズン"
}
"anime": {
  "title": "アニメ作品",            // was "アニメ"
  "addAnime": "アニメ作品追加",      // was "アニメ追加"
  "animeTitle": "タイトル",         // was "アニメタイトル"
  "deleteAnime": "アニメ作品を削除",  // was "アニメを削除"
  "notFound": "アニメ作品が見つかりません",  // was "アニメが見つかりません"
  "noRules": "このアニメ作品にはフィルタールールがありません。",
}
```

Add new subscription keys:

```json
"subscriptions": {
  // ... existing keys ...
  "addSubscription": "サブスクリプション追加",
  "deleteSubscription": "サブスクリプションを削除",
  "deleteConfirm": "「{{name}}」を削除してもよろしいですか？未処理の生データ {{count}} 件も削除されます。",
  "name": "名前",
  "fetchInterval": "取得間隔（分）"
}
```

**Step 4: Commit**

```bash
git add frontend/src/i18n/en.json frontend/src/i18n/zh-TW.json frontend/src/i18n/ja.json
git commit -m "feat: rename anime labels and add subscription i18n keys"
```

---

### Task 2: Backend — Add raw items count endpoint

**Files:**
- Modify: `core-service/src/handlers/raw_items.rs`
- Modify: `core-service/src/main.rs`

**Step 1: Add count handler to raw_items.rs**

At the end of `core-service/src/handlers/raw_items.rs`, add:

```rust
#[derive(Debug, Deserialize)]
pub struct CountRawItemsQuery {
    pub subscription_id: i32,
    pub status: Option<String>, // comma-separated: "pending,failed"
}

/// GET /raw-items/count - count raw items by subscription and status
pub async fn count_raw_items(
    State(state): State<AppState>,
    Query(query): Query<CountRawItemsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut q = raw_anime_items::table
        .filter(raw_anime_items::subscription_id.eq(query.subscription_id))
        .into_boxed();

    if let Some(status) = &query.status {
        let statuses: Vec<&str> = status.split(',').collect();
        q = q.filter(raw_anime_items::status.eq_any(statuses));
    }

    let count: i64 = q
        .count()
        .get_result(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "count": count })))
}
```

**Step 2: Register route in main.rs**

In `core-service/src/main.rs`, add the route near the existing raw-items routes. Add this line **before** the `/raw-items/:item_id` route:

```rust
.route("/raw-items/count", get(handlers::raw_items::count_raw_items))
```

**Step 3: Verify compilation**

Run: `cd /workspace/core-service && cargo check`
Expected: compiles successfully

**Step 4: Commit**

```bash
git add core-service/src/handlers/raw_items.rs core-service/src/main.rs
git commit -m "feat: add GET /raw-items/count endpoint"
```

---

### Task 3: Backend — Modify delete subscription to use ID and cascade

**Files:**
- Modify: `core-service/src/handlers/subscriptions.rs`
- Modify: `core-service/src/main.rs`

**Step 1: Update delete_subscription handler**

Replace the existing `delete_subscription` function in `core-service/src/handlers/subscriptions.rs` (lines 595-656) with:

```rust
/// Delete a subscription by ID, cascade-deleting pending/failed raw items
pub async fn delete_subscription(
    State(state): State<AppState>,
    Path(subscription_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            // First, delete pending/failed raw items for this subscription
            let raw_deleted = diesel::delete(
                crate::schema::raw_anime_items::table
                    .filter(crate::schema::raw_anime_items::subscription_id.eq(subscription_id))
                    .filter(crate::schema::raw_anime_items::status.eq_any(vec!["pending", "failed"])),
            )
            .execute(&mut conn);

            let raw_count = match raw_deleted {
                Ok(count) => count,
                Err(e) => {
                    tracing::error!("Failed to delete raw items: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to delete raw items: {}", e)
                        })),
                    );
                }
            };

            match diesel::delete(
                subscriptions::table.filter(subscriptions::subscription_id.eq(subscription_id)),
            )
            .execute(&mut conn)
            {
                Ok(rows_deleted) => {
                    if rows_deleted > 0 {
                        tracing::info!(
                            "Deleted subscription {} (and {} raw items)",
                            subscription_id,
                            raw_count
                        );
                        (
                            StatusCode::OK,
                            Json(json!({
                                "message": "Subscription deleted successfully",
                                "subscription_id": subscription_id,
                                "raw_items_deleted": raw_count
                            })),
                        )
                    } else {
                        tracing::warn!("Subscription not found: {}", subscription_id);
                        (
                            StatusCode::NOT_FOUND,
                            Json(json!({
                                "error": "not_found",
                                "message": format!("Subscription not found: {}", subscription_id)
                            })),
                        )
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to delete subscription: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to delete subscription: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}
```

**Step 2: Update route in main.rs**

Change the route from `:rss_url` to `:id`:

```rust
// Before:
.route("/subscriptions/:rss_url", delete(handlers::subscriptions::delete_subscription))
// After:
.route("/subscriptions/:id", delete(handlers::subscriptions::delete_subscription))
```

**Step 3: Verify compilation**

Run: `cd /workspace/core-service && cargo check`
Expected: compiles successfully

**Step 4: Commit**

```bash
git add core-service/src/handlers/subscriptions.rs core-service/src/main.rs
git commit -m "feat: delete subscription by ID with cascade raw item cleanup"
```

---

### Task 4: Frontend — Add API methods to CoreApi

**Files:**
- Modify: `frontend/src/services/CoreApi.ts`
- Modify: `frontend/src/layers/ApiLayer.ts`

**Step 1: Add type definitions to CoreApi.ts**

Add these methods to the `CoreApi` interface in `frontend/src/services/CoreApi.ts` (inside the Context.Tag type, after the existing `getRawItem` method):

```typescript
readonly createSubscription: (req: {
  source_url: string
  name?: string
  fetch_interval_minutes?: number
}) => Effect.Effect<Subscription>
readonly deleteSubscription: (id: number) => Effect.Effect<void>
readonly getRawItemsCount: (subscriptionId: number, status: string) => Effect.Effect<number>
```

**Step 2: Implement in ApiLayer.ts**

Add these implementations in `frontend/src/layers/ApiLayer.ts`, inside the `CoreApi.of({...})` block, after `getRawItem`:

```typescript
createSubscription: (req) =>
  postJson("/api/core/subscriptions", req, Subscription),

deleteSubscription: (id) =>
  client
    .execute(HttpClientRequest.del(`/api/core/subscriptions/${id}`))
    .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

getRawItemsCount: (subscriptionId, status) =>
  fetchJson(
    HttpClientRequest.get(
      `/api/core/raw-items/count?subscription_id=${subscriptionId}&status=${status}`,
    ),
    Schema.Struct({ count: Schema.Number }),
  ).pipe(Effect.map((r) => r.count)),
```

**Step 3: Commit**

```bash
git add frontend/src/services/CoreApi.ts frontend/src/layers/ApiLayer.ts
git commit -m "feat: add createSubscription, deleteSubscription, getRawItemsCount API methods"
```

---

### Task 5: Frontend — Add create/delete UI to SubscriptionsPage

**Files:**
- Modify: `frontend/src/pages/subscriptions/SubscriptionsPage.tsx`

**Step 1: Add create/delete functionality**

Rewrite `SubscriptionsPage.tsx` to include:
- "Add Subscription" button (top-right, matching AnimePage pattern)
- Create dialog with source_url, name, fetch_interval_minutes fields
- Delete button in the actions column
- Delete confirmation dialog that shows affected raw items count

The full implementation follows the existing `AnimePage.tsx` pattern. Key additions:

1. Import `useEffectMutation`, `ConfirmDialog`, `Button`, `Dialog`, `Input`, `Plus` (same as AnimePage)
2. Add states: `createOpen`, form fields (`newUrl`, `newName`, `newInterval`), `deleteTarget`, `affectedCount`
3. `createSubscription` mutation → `api.createSubscription({...})`
4. `deleteSubscription` mutation → `api.deleteSubscription(id)`
5. When delete button clicked: first call `api.getRawItemsCount(id, "pending,failed")`, store count, then open confirm dialog
6. Confirm dialog description uses `t("subscriptions.deleteConfirm", { name, count })`
7. Add actions column to DataTable with delete button (same pattern as AnimePage)

```tsx
import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { StatusBadge } from "@/components/shared/StatusBadge"
import { ConfirmDialog } from "@/components/shared/ConfirmDialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Plus } from "lucide-react"
import { SubscriptionDialog } from "./SubscriptionDialog"
import type { Subscription } from "@/schemas/subscription"

export default function SubscriptionsPage() {
  const { t } = useTranslation()
  const [selectedSub, setSelectedSub] = useState<Subscription | null>(null)
  const [createOpen, setCreateOpen] = useState(false)
  const [newUrl, setNewUrl] = useState("")
  const [newName, setNewName] = useState("")
  const [newInterval, setNewInterval] = useState("30")
  const [deleteTarget, setDeleteTarget] = useState<{
    id: number
    name: string
  } | null>(null)
  const [affectedCount, setAffectedCount] = useState(0)

  const { data: subscriptions, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getSubscriptions
      }),
    [],
  )

  const { mutate: createSubscription, isLoading: creating } = useEffectMutation(
    (req: { source_url: string; name?: string; fetch_interval_minutes?: number }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createSubscription(req)
      }),
  )

  const { mutate: deleteSubscription, isLoading: deleting } = useEffectMutation(
    (id: number) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.deleteSubscription(id)
      }),
  )

  const handleDeleteClick = (id: number, name: string) => {
    Effect.gen(function* () {
      const api = yield* CoreApi
      return yield* api.getRawItemsCount(id, "pending,failed")
    }).pipe(Effect.runPromise).then((count) => {
      setAffectedCount(count)
      setDeleteTarget({ id, name })
    })
  }

  const columns: Column<Record<string, unknown>>[] = [
    {
      key: "subscription_id",
      header: t("common.id"),
      render: (item) => String(item.subscription_id),
    },
    {
      key: "name",
      header: t("common.name"),
      render: (item) => String(item.name ?? item.source_url),
    },
    {
      key: "source_url",
      header: t("subscriptions.sourceUrl"),
      render: (item) => (
        <span className="text-xs font-mono truncate max-w-[300px] block">
          {String(item.source_url)}
        </span>
      ),
    },
    {
      key: "fetch_interval_minutes",
      header: t("subscriptions.interval"),
      render: (item) => `${item.fetch_interval_minutes} min`,
    },
    {
      key: "is_active",
      header: t("common.status"),
      render: (item) => (
        <StatusBadge status={item.is_active ? "parsed" : "failed"} />
      ),
    },
    {
      key: "last_fetched_at",
      header: t("subscriptions.lastFetched"),
      render: (item) =>
        item.last_fetched_at
          ? String(item.last_fetched_at).slice(0, 19).replace("T", " ")
          : t("common.never"),
    },
    {
      key: "actions",
      header: "",
      render: (item) => (
        <Button
          variant="ghost"
          size="sm"
          className="text-destructive"
          onClick={(e) => {
            e.stopPropagation()
            handleDeleteClick(
              item.subscription_id as number,
              (item.name ?? item.source_url) as string,
            )
          }}
        >
          {t("common.delete")}
        </Button>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("subscriptions.title")}</h1>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          {t("subscriptions.addSubscription")}
        </Button>
      </div>

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <DataTable
          columns={columns}
          data={(subscriptions ?? []) as unknown as Record<string, unknown>[]}
          keyField="subscription_id"
          onRowClick={(row) => {
            const found = (subscriptions ?? []).find(
              (s) => s.subscription_id === row.subscription_id,
            )
            if (found) setSelectedSub(found)
          }}
        />
      )}

      {selectedSub && (
        <SubscriptionDialog
          subscription={selectedSub}
          open={!!selectedSub}
          onOpenChange={(open) => {
            if (!open) {
              setSelectedSub(null)
              refetch()
            }
          }}
        />
      )}

      {/* Create Dialog */}
      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.addSubscription")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label>{t("subscriptions.sourceUrl")}</Label>
              <Input
                placeholder="https://mikanani.me/RSS/..."
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label>{t("subscriptions.name")}</Label>
              <Input
                placeholder={t("subscriptions.name")}
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
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              disabled={!newUrl.trim() || creating}
              onClick={() => {
                createSubscription({
                  source_url: newUrl.trim(),
                  name: newName.trim() || undefined,
                  fetch_interval_minutes: parseInt(newInterval) || 30,
                }).then(() => {
                  setNewUrl("")
                  setNewName("")
                  setNewInterval("30")
                  setCreateOpen(false)
                  refetch()
                })
              }}
            >
              {creating ? t("common.creating") : t("common.create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirm */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title={t("subscriptions.deleteSubscription")}
        description={t("subscriptions.deleteConfirm", {
          name: deleteTarget?.name,
          count: affectedCount,
        })}
        loading={deleting}
        onConfirm={() => {
          if (deleteTarget) {
            deleteSubscription(deleteTarget.id).then(() => {
              setDeleteTarget(null)
              refetch()
            })
          }
        }}
      />
    </div>
  )
}
```

**Step 2: Check Label component exists**

Run: `ls frontend/src/components/ui/label.tsx`
If missing, run: `cd /workspace/frontend && npx shadcn@latest add label`

**Step 3: Verify build**

Run: `cd /workspace/frontend && npm run build`
Expected: builds successfully

**Step 4: Commit**

```bash
git add frontend/src/pages/subscriptions/SubscriptionsPage.tsx
git commit -m "feat: add subscription create/delete UI with confirmation dialog"
```

---

### Task 6: Frontend — Fix Effect.runPromise usage for getRawItemsCount

**Files:**
- Modify: `frontend/src/pages/subscriptions/SubscriptionsPage.tsx`

**Step 1: Check how Effect runtime is used in the project**

The `handleDeleteClick` function uses `Effect.runPromise` but needs the `CoreApi` layer provided. Check the existing `useEffectQuery` / `useEffectMutation` hooks to see how they provide the layer, and replicate the same pattern.

Look at `frontend/src/hooks/useEffectQuery.ts` and `frontend/src/runtime/index.ts` to find the `AppRuntime` reference. The count fetch should use the same runtime:

```typescript
import { AppRuntime } from "@/runtime"

const handleDeleteClick = (id: number, name: string) => {
  AppRuntime.runPromise(
    Effect.gen(function* () {
      const api = yield* CoreApi
      return yield* api.getRawItemsCount(id, "pending,failed")
    })
  ).then((count) => {
    setAffectedCount(count)
    setDeleteTarget({ id, name })
  })
}
```

**Step 2: Verify build**

Run: `cd /workspace/frontend && npm run build`
Expected: builds successfully

**Step 3: Commit (if changes made)**

```bash
git add frontend/src/pages/subscriptions/SubscriptionsPage.tsx
git commit -m "fix: use AppRuntime for raw items count fetch"
```

---

### Task 7: Final verification

**Step 1: Verify backend compiles**

Run: `cd /workspace/core-service && cargo check`

**Step 2: Verify frontend builds**

Run: `cd /workspace/frontend && npm run build`

**Step 3: Spot-check all three translation files have matching key sets**

Verify that `en.json`, `zh-TW.json`, and `ja.json` all have:
- `subscriptions.addSubscription`
- `subscriptions.deleteSubscription`
- `subscriptions.deleteConfirm`
- `subscriptions.name`
- `subscriptions.fetchInterval`
- Updated `sidebar.animeSeries`, `sidebar.anime`, `anime.title`, etc.

**Step 4: Final commit if any cleanup needed**
