# Subscription UI Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve subscription management with soft/hard delete, editable names, raw-item-based filter preview, and filter status display in raw item details.

**Architecture:** Backend-first approach — add PATCH subscription endpoint, raw filter preview endpoint, and filter_passed field, then update frontend components. Shared EditableText component for inline editing across pages.

**Tech Stack:** Rust/Diesel (backend), React 19 + TypeScript + Effect + Radix UI + Tailwind (frontend), i18n via react-i18next

---

### Task 1: Backend — PATCH /subscriptions/:id endpoint

**Files:**
- Modify: `core-service/src/handlers/subscriptions.rs`
- Modify: `core-service/src/main.rs`

**Step 1: Add UpdateSubscriptionRequest DTO and handler**

In `core-service/src/handlers/subscriptions.rs`, add after the `DeleteSubscriptionQuery` struct (line 78):

```rust
#[derive(Debug, serde::Deserialize)]
pub struct UpdateSubscriptionRequest {
    pub name: Option<String>,
}

/// PATCH /subscriptions/:id — update subscription fields
pub async fn update_subscription(
    State(state): State<AppState>,
    Path(subscription_id): Path<i32>,
    Json(payload): Json<UpdateSubscriptionRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            );
        }
    };

    let now = Utc::now().naive_utc();

    // Build update: only provided fields
    let result = if let Some(ref name) = payload.name {
        diesel::update(
            subscriptions::table.filter(subscriptions::subscription_id.eq(subscription_id)),
        )
        .set((
            subscriptions::name.eq(Some(name.as_str())),
            subscriptions::updated_at.eq(now),
        ))
        .execute(&mut conn)
    } else {
        // Nothing to update
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "no_fields",
                "message": "No fields to update"
            })),
        );
    };

    match result {
        Ok(rows) if rows > 0 => {
            tracing::info!("Updated subscription {}", subscription_id);
            // Return the updated subscription
            match subscriptions::table
                .filter(subscriptions::subscription_id.eq(subscription_id))
                .select(Subscription::as_select())
                .first::<Subscription>(&mut conn)
            {
                Ok(sub) => (StatusCode::OK, Json(json!(sub))),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "database_error",
                        "message": format!("Failed to reload subscription: {}", e)
                    })),
                ),
            }
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "not_found",
                "message": format!("Subscription not found: {}", subscription_id)
            })),
        ),
        Err(e) => {
            tracing::error!("Failed to update subscription: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to update subscription: {}", e)
                })),
            )
        }
    }
}
```

**Step 2: Register route in main.rs**

In `core-service/src/main.rs`, change the existing `/subscriptions/:id` delete route (around line 164-167) to a method router:

```rust
        .route(
            "/subscriptions/:id",
            delete(handlers::subscriptions::delete_subscription)
                .patch(handlers::subscriptions::update_subscription),
        )
```

Add `patch` to the routing imports at the top of main.rs (line 3):
```rust
    routing::{delete, get, patch, post, put},
```

**Step 3: Build and verify**

Run: `cargo build -p core-service`
Expected: Compiles with no new errors.

**Step 4: Commit**

```
feat(core): add PATCH /subscriptions/:id endpoint for name updates
```

---

### Task 2: Backend — POST /filters/preview-raw endpoint

**Files:**
- Modify: `core-service/src/handlers/filters.rs`
- Modify: `core-service/src/main.rs`

**Step 1: Add RawPreviewItem struct and handler**

In `core-service/src/handlers/filters.rs`, add after the existing `FilterPreviewResponse` struct (after line 262):

```rust
#[derive(Debug, Serialize)]
pub struct RawPreviewItem {
    pub item_id: i32,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct RawFilterPreviewPanel {
    pub passed_items: Vec<RawPreviewItem>,
    pub filtered_items: Vec<RawPreviewItem>,
}

#[derive(Debug, Serialize)]
pub struct RawFilterPreviewResponse {
    pub regex_valid: bool,
    pub regex_error: Option<String>,
    pub before: RawFilterPreviewPanel,
    pub after: RawFilterPreviewPanel,
}
```

Then add the handler after `preview_filter` (after line 395):

```rust
/// POST /filters/preview-raw
///
/// Preview the effect of adding/removing a filter rule on raw_anime_items
/// scoped to a subscription (target_type must be "fetcher" or "subscription").
pub async fn preview_filter_raw(
    State(state): State<AppState>,
    Json(req): Json<FilterPreviewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate regex
    let new_regex = match Regex::new(&req.regex_pattern) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::OK,
                Json(json!(RawFilterPreviewResponse {
                    regex_valid: false,
                    regex_error: Some(e.to_string()),
                    before: RawFilterPreviewPanel { passed_items: vec![], filtered_items: vec![] },
                    after: RawFilterPreviewPanel { passed_items: vec![], filtered_items: vec![] },
                })),
            );
        }
    };

    let subscription_id = match req.target_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "target_id (subscription_id) is required for preview-raw" })),
            );
        }
    };

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("DB connection failed: {}", e) })),
            );
        }
    };

    // Load raw_anime_items for this subscription
    use crate::schema::raw_anime_items;
    let raw_items: Vec<crate::models::RawAnimeItem> = match raw_anime_items::table
        .filter(raw_anime_items::subscription_id.eq(subscription_id))
        .order(raw_anime_items::created_at.desc())
        .load(&mut conn)
    {
        Ok(items) => items,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to load raw items: {}", e) })),
            );
        }
    };

    // Load existing filter rules for this subscription (fetcher target type)
    use crate::models::FilterTargetType;
    let existing_rules: Vec<FilterRule> = match crate::schema::filter_rules::table
        .filter(crate::schema::filter_rules::target_type.eq(FilterTargetType::Fetcher))
        .filter(crate::schema::filter_rules::target_id.eq(subscription_id))
        .order(crate::schema::filter_rules::rule_order.asc())
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(_) => vec![],
    };

    // Also load global rules
    let global_rules: Vec<FilterRule> = match crate::schema::filter_rules::table
        .filter(crate::schema::filter_rules::target_type.eq(FilterTargetType::Global))
        .filter(crate::schema::filter_rules::target_id.is_null())
        .order(crate::schema::filter_rules::rule_order.asc())
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(_) => vec![],
    };

    // Build the new temporary rule
    let now = Utc::now().naive_utc();
    let new_rule = FilterRule {
        rule_id: -1,
        rule_order: 0,
        is_positive: req.is_positive,
        regex_pattern: req.regex_pattern.clone(),
        created_at: now,
        updated_at: now,
        target_type: FilterTargetType::Fetcher,
        target_id: Some(subscription_id),
    };

    let mut before_passed = vec![];
    let mut before_filtered = vec![];
    let mut after_passed = vec![];
    let mut after_filtered = vec![];

    for item in &raw_items {
        let title = &item.title;

        // "Before" rules = global + existing, excluding the one being edited
        let before_rules: Vec<FilterRule> = global_rules
            .iter()
            .chain(existing_rules.iter())
            .filter(|r| Some(r.rule_id) != req.exclude_filter_id)
            .cloned()
            .collect();

        let before_engine = FilterEngine::with_priority_sorted(before_rules.clone());
        let before_include = before_engine.should_include(title);

        // "After" rules = before + new rule
        let mut after_rules = before_rules;
        after_rules.push(new_rule.clone());
        let after_engine = FilterEngine::with_priority_sorted(after_rules);
        let after_include = after_engine.should_include(title);

        let preview_item_before = RawPreviewItem {
            item_id: item.item_id,
            title: title.clone(),
            status: item.status.clone(),
        };
        let preview_item_after = RawPreviewItem {
            item_id: item.item_id,
            title: title.clone(),
            status: item.status.clone(),
        };

        if before_include {
            before_passed.push(preview_item_before);
        } else {
            before_filtered.push(preview_item_before);
        }
        if after_include {
            after_passed.push(preview_item_after);
        } else {
            after_filtered.push(preview_item_after);
        }
    }

    (
        StatusCode::OK,
        Json(json!(RawFilterPreviewResponse {
            regex_valid: true,
            regex_error: None,
            before: RawFilterPreviewPanel { passed_items: before_passed, filtered_items: before_filtered },
            after: RawFilterPreviewPanel { passed_items: after_passed, filtered_items: after_filtered },
        })),
    )
}
```

**Step 2: Register route in main.rs**

In `core-service/src/main.rs`, add after the existing `/filters/preview` route (around line 137):

```rust
        .route(
            "/filters/preview-raw",
            post(handlers::filters::preview_filter_raw),
        )
```

**Step 3: Build and verify**

Run: `cargo build -p core-service`
Expected: Compiles with no new errors.

**Step 4: Commit**

```
feat(core): add POST /filters/preview-raw for raw item filter preview
```

---

### Task 3: Backend — Add filter_passed to raw item detail

**Files:**
- Modify: `core-service/src/handlers/raw_items.rs`

**Step 1: Add filter evaluation to get_raw_item response**

In `core-service/src/handlers/raw_items.rs`, add `filter_passed` field to `RawItemResponse` struct (around line 36):

```rust
pub filter_passed: Option<bool>,
```

Then in the `get_raw_item` handler (around line 173), after loading the raw item and download info, add filter evaluation logic before building the response:

```rust
    // Evaluate filter rules for this item
    let filter_passed = {
        use crate::models::{FilterRule, FilterTargetType};
        use crate::schema::filter_rules;
        use crate::services::filter::FilterEngine;

        // Load global rules
        let global_rules: Vec<FilterRule> = filter_rules::table
            .filter(filter_rules::target_type.eq(FilterTargetType::Global))
            .filter(filter_rules::target_id.is_null())
            .order(filter_rules::rule_order.asc())
            .load(&mut conn)
            .unwrap_or_default();

        // Load subscription-scoped rules
        let sub_rules: Vec<FilterRule> = filter_rules::table
            .filter(filter_rules::target_type.eq(FilterTargetType::Fetcher))
            .filter(filter_rules::target_id.eq(item.subscription_id))
            .order(filter_rules::rule_order.asc())
            .load(&mut conn)
            .unwrap_or_default();

        let all_rules: Vec<FilterRule> = global_rules.into_iter().chain(sub_rules).collect();

        if all_rules.is_empty() {
            None // No rules = no filter status to show
        } else {
            let engine = FilterEngine::with_priority_sorted(all_rules);
            Some(engine.should_include(&item.title))
        }
    };
```

Set the field in the response builder: `filter_passed,`

Also update `list_raw_items` to set `filter_passed: None` for each item in the list response (to avoid the extra query cost on list view).

**Step 2: Build and verify**

Run: `cargo build -p core-service`
Expected: Compiles with no new errors.

**Step 3: Commit**

```
feat(core): add filter_passed field to raw item detail response
```

---

### Task 4: Frontend — Update API layer and schemas

**Files:**
- Modify: `frontend/src/services/CoreApi.ts`
- Modify: `frontend/src/layers/ApiLayer.ts`
- Modify: `frontend/src/schemas/download.ts`
- Modify: `frontend/src/schemas/filter.ts`
- Modify: `frontend/src/schemas/common.ts`

**Step 1: Update RawAnimeItem schema to include filter_passed**

In `frontend/src/schemas/download.ts`, add to RawAnimeItem:
```typescript
filter_passed: Schema.NullOr(Schema.Boolean),
```

**Step 2: Add RawPreviewItem to common.ts**

In `frontend/src/schemas/common.ts`, add:
```typescript
export const RawPreviewItem = Schema.Struct({
  item_id: Schema.Number,
  title: Schema.String,
  status: Schema.String,
})
export type RawPreviewItem = typeof RawPreviewItem.Type
```

**Step 3: Add RawFilterPreviewResponse to filter.ts**

In `frontend/src/schemas/filter.ts`, add:
```typescript
import { RawPreviewItem } from "./common"

export const RawFilterPreviewResponse = Schema.Struct({
  regex_valid: Schema.Boolean,
  regex_error: Schema.NullOr(Schema.String),
  before: Schema.Struct({
    passed_items: Schema.Array(RawPreviewItem),
    filtered_items: Schema.Array(RawPreviewItem),
  }),
  after: Schema.Struct({
    passed_items: Schema.Array(RawPreviewItem),
    filtered_items: Schema.Array(RawPreviewItem),
  }),
})
export type RawFilterPreviewResponse = typeof RawFilterPreviewResponse.Type
```

**Step 4: Add new API methods to CoreApi.ts**

Add to the CoreApi interface:
```typescript
readonly updateSubscription: (id: number, req: { name?: string }) => Effect.Effect<Subscription>
readonly deleteSubscription: (id: number, purge?: boolean) => Effect.Effect<void>
readonly previewFilterRaw: (req: {
  target_type: string
  target_id?: number | null
  regex_pattern: string
  is_positive: boolean
  exclude_filter_id?: number
}) => Effect.Effect<RawFilterPreviewResponse>
```

Note: `deleteSubscription` signature changes to accept optional `purge` param.

**Step 5: Implement in ApiLayer.ts**

Update `deleteSubscription`:
```typescript
deleteSubscription: (id, purge) =>
  client
    .execute(HttpClientRequest.del(`/api/core/subscriptions/${id}${purge ? '?purge=true' : ''}`))
    .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),
```

Add `updateSubscription`:
```typescript
updateSubscription: (id, req) =>
  client
    .execute(
      HttpClientRequest.patch(`/api/core/subscriptions/${id}`).pipe(
        HttpClientRequest.jsonBody(req),
      ),
    )
    .pipe(
      Effect.flatMap((r) => r.json),
      Effect.flatMap(Schema.decodeUnknown(Subscription)),
      Effect.scoped,
      Effect.orDie,
    ),
```

Add `previewFilterRaw`:
```typescript
previewFilterRaw: (req) =>
  postJson("/api/core/filters/preview-raw", req, RawFilterPreviewResponse),
```

**Step 6: Verify frontend builds**

Run: `cd /workspace/frontend && npm run build`
Expected: Builds with no errors.

**Step 7: Commit**

```
feat(frontend): add API methods for subscription update, purge delete, and raw filter preview
```

---

### Task 5: Frontend — EditableText shared component

**Files:**
- Create: `frontend/src/components/shared/EditableText.tsx`

**Step 1: Create the component**

```tsx
import { useState, useRef, useEffect } from "react"
import { Input } from "@/components/ui/input"
import { Pencil, Loader2 } from "lucide-react"

interface EditableTextProps {
  value: string
  onSave: (newValue: string) => Promise<void>
  placeholder?: string
  className?: string
}

export function EditableText({ value, onSave, placeholder, className }: EditableTextProps) {
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(value)
  const [saving, setSaving] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (editing) {
      setDraft(value)
      setTimeout(() => inputRef.current?.select(), 0)
    }
  }, [editing, value])

  const handleSave = async () => {
    const trimmed = draft.trim()
    if (!trimmed || trimmed === value) {
      setEditing(false)
      return
    }
    setSaving(true)
    try {
      await onSave(trimmed)
      setEditing(false)
    } finally {
      setSaving(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleSave()
    if (e.key === "Escape") setEditing(false)
  }

  if (editing) {
    return (
      <div className="flex items-center gap-1">
        <Input
          ref={inputRef}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={handleSave}
          disabled={saving}
          className={`h-7 text-sm ${className ?? ""}`}
          placeholder={placeholder}
        />
        {saving && <Loader2 className="h-3 w-3 animate-spin" />}
      </div>
    )
  }

  return (
    <button
      type="button"
      className={`group inline-flex items-center gap-1 text-sm font-medium hover:text-primary transition-colors ${className ?? ""}`}
      onClick={() => setEditing(true)}
    >
      {value || <span className="text-muted-foreground">{placeholder}</span>}
      <Pencil className="h-3 w-3 opacity-0 group-hover:opacity-50 transition-opacity" />
    </button>
  )
}
```

**Step 2: Commit**

```
feat(frontend): add EditableText shared component for inline editing
```

---

### Task 6: Frontend — DeleteSubscriptionDialog component

**Files:**
- Create: `frontend/src/components/shared/DeleteSubscriptionDialog.tsx`
- Modify: `frontend/src/i18n/zh-TW.json`
- Modify: `frontend/src/i18n/en.json`
- Modify: `frontend/src/i18n/ja.json`

**Step 1: Add i18n keys**

In all three i18n files, add to the `subscriptions` section:

zh-TW.json:
```json
"deactivate": "停用訂閱",
"purgeDelete": "完全刪除（含已下載動畫）",
"deactivateDesc": "訂閱將停用，所有資料保留，可隨時重新啟用。",
"purgeDesc": "將刪除此訂閱的所有資料，包含已下載的動畫檔案，此操作無法復原。"
```

en.json:
```json
"deactivate": "Deactivate",
"purgeDelete": "Delete Everything (incl. downloaded anime)",
"deactivateDesc": "The subscription will be deactivated. All data is preserved and can be reactivated anytime.",
"purgeDesc": "All data for this subscription will be permanently deleted, including downloaded anime files. This cannot be undone."
```

ja.json:
```json
"deactivate": "無効化",
"purgeDelete": "完全削除（ダウンロード済みアニメ含む）",
"deactivateDesc": "サブスクリプションを無効化します。すべてのデータは保持され、いつでも再有効化できます。",
"purgeDesc": "このサブスクリプションのすべてのデータが完全に削除されます。ダウンロード済みのアニメファイルも含まれ、元に戻せません。"
```

**Step 2: Create the dialog component**

```tsx
import { useTranslation } from "react-i18next"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { AlertTriangle } from "lucide-react"

interface DeleteSubscriptionDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  subscriptionName: string
  onDeactivate: () => void
  onPurge: () => void
  loading?: boolean
}

export function DeleteSubscriptionDialog({
  open,
  onOpenChange,
  subscriptionName,
  onDeactivate,
  onPurge,
  loading,
}: DeleteSubscriptionDialogProps) {
  const { t } = useTranslation()

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("subscriptions.deleteSubscription")}</DialogTitle>
          <DialogDescription>
            {subscriptionName}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          {/* Deactivate option */}
          <div className="rounded-md border p-3 space-y-1">
            <p className="text-sm text-muted-foreground">
              {t("subscriptions.deactivateDesc")}
            </p>
            <Button
              variant="outline"
              className="w-full"
              onClick={onDeactivate}
              disabled={loading}
            >
              {t("subscriptions.deactivate")}
            </Button>
          </div>

          {/* Purge option */}
          <div className="rounded-md border border-destructive/30 bg-destructive/5 p-3 space-y-1">
            <div className="flex items-start gap-2">
              <AlertTriangle className="h-4 w-4 text-destructive mt-0.5 shrink-0" />
              <p className="text-sm text-destructive">
                {t("subscriptions.purgeDesc")}
              </p>
            </div>
            <Button
              variant="destructive"
              className="w-full"
              onClick={onPurge}
              disabled={loading}
            >
              {t("subscriptions.purgeDelete")}
            </Button>
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)} disabled={loading}>
            {t("common.cancel")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

**Step 3: Commit**

```
feat(frontend): add DeleteSubscriptionDialog with soft/hard delete options
```

---

### Task 7: Frontend — Wire up SubscriptionsPage with new delete dialog and API

**Files:**
- Modify: `frontend/src/pages/subscriptions/SubscriptionsPage.tsx`

**Step 1: Replace ConfirmDialog with DeleteSubscriptionDialog**

Key changes:
1. Import `DeleteSubscriptionDialog` instead of `ConfirmDialog`
2. Change `deleteSubscription` mutation to accept `{ id: number; purge: boolean }`
3. Replace the ConfirmDialog JSX with DeleteSubscriptionDialog
4. Remove `affectedCount` state and `getRawItemsCount` call (no longer needed)
5. `handleDeleteClick` just sets deleteTarget directly (no pre-fetch)

Replace the entire mutation (lines 56-62):
```typescript
const { mutate: deleteSubscription, isLoading: deleting } = useEffectMutation(
  ({ id, purge }: { id: number; purge: boolean }) =>
    Effect.gen(function* () {
      const api = yield* CoreApi
      return yield* api.deleteSubscription(id, purge)
    }),
)
```

Simplify `handleDeleteClick` (remove the AppRuntime.runPromise count fetch):
```typescript
const handleDeleteClick = (id: number, name: string) => {
  setDeleteTarget({ id, name })
}
```

Replace the ConfirmDialog JSX (lines 237-254) with:
```tsx
<DeleteSubscriptionDialog
  open={!!deleteTarget}
  onOpenChange={(open) => !open && setDeleteTarget(null)}
  subscriptionName={deleteTarget?.name ?? ""}
  loading={deleting}
  onDeactivate={() => {
    if (deleteTarget) {
      deleteSubscription({ id: deleteTarget.id, purge: false }).then(() => {
        setDeleteTarget(null)
        refetch()
      })
    }
  }}
  onPurge={() => {
    if (deleteTarget) {
      deleteSubscription({ id: deleteTarget.id, purge: true }).then(() => {
        setDeleteTarget(null)
        refetch()
      })
    }
  }}
/>
```

Remove: `affectedCount` state, `AppRuntime` import (if no longer used), `ConfirmDialog` import.

**Step 2: Verify frontend builds**

Run: `cd /workspace/frontend && npm run build`

**Step 3: Commit**

```
feat(frontend): wire SubscriptionsPage with new delete dialog and purge API
```

---

### Task 8: Frontend — Editable subscription name in SubscriptionDialog

**Files:**
- Modify: `frontend/src/pages/subscriptions/SubscriptionDialog.tsx`

**Step 1: Add EditableText for name field**

Import `EditableText` and add mutation for updating subscription name. Replace the name InfoItem with EditableText.

The key change is:
1. Import `EditableText`, `useEffectMutation`, `Effect`, `CoreApi`
2. Add `updateSubscription` mutation
3. Replace `<InfoItem label={t("common.name")} value={subscription.name ?? ""} />` with:

```tsx
<div>
  <p className="text-xs text-muted-foreground">{t("common.name")}</p>
  <EditableText
    value={subscription.name ?? ""}
    placeholder={t("subscriptions.name")}
    onSave={async (name) => {
      await updateName({ id: subscription.subscription_id, name })
    }}
  />
</div>
```

Add `onSubscriptionChange?: () => void` to props for notifying parent to refetch.

**Step 2: Commit**

```
feat(frontend): add inline subscription name editing in SubscriptionDialog
```

---

### Task 9: Frontend — Editable anime title in AnimeDialog

**Files:**
- Modify: `frontend/src/pages/anime/AnimeDialog.tsx`

**Step 1: Use EditableText for anime title**

Since there's no `updateAnime` endpoint for title, we need to check if one exists. Based on research, there is no PATCH anime title endpoint. Two options:
- Add a backend endpoint (extra scope)
- Skip for now and only do subscriptions

**Decision:** Skip anime title editing for now — the design doc says "bonus". Only implement for subscriptions. If the user wants it, a `PATCH /anime/:id` endpoint needs to be added first.

**Step 2: Commit** — N/A (skipped)

---

### Task 10: Frontend — FilterPreviewPanel with status badges

**Files:**
- Modify: `frontend/src/components/shared/FilterPreviewPanel.tsx`

**Step 1: Update FilterPreviewPanel to show optional status**

The panel receives items with optional `status` field. When present, show a StatusBadge.

Update the `MergedItem` type to include optional status:
```typescript
interface MergedItem {
  item_id: number
  title: string
  status?: string  // raw item status if available
  state: "passed" | "filtered" | "newly-passed" | "newly-filtered"
}
```

In the `mergeItems` function, carry `status` through from the source items (use type assertion or update the props to accept items with optional status).

In `FilterPreviewRow`, add status badge display:
```tsx
{item.status && (
  <StatusBadge status={item.status} />
)}
```

Import `StatusBadge` component.

**Step 2: Commit**

```
feat(frontend): show raw item status badges in FilterPreviewPanel
```

---

### Task 11: Frontend — FilterRuleEditor uses preview-raw for subscriptions

**Files:**
- Modify: `frontend/src/components/shared/FilterRuleEditor.tsx`

**Step 1: Call preview-raw when targetType is "fetcher" (subscription context)**

In the `loadBaseline` callback and the debounced preview effect, check if `targetType === "fetcher"`. If so, call `api.previewFilterRaw(...)` instead of `api.previewFilter(...)`.

Update `loadBaseline`:
```typescript
const loadBaseline = useCallback(() => {
  const apiCall = targetType === "fetcher"
    ? Effect.flatMap(CoreApi, (api) => api.previewFilterRaw({ ... }))
    : Effect.flatMap(CoreApi, (api) => api.previewFilter({ ... }))
  AppRuntime.runPromise(apiCall).then(setBaseline).catch(() => setBaseline(null))
}, [targetType, targetId])
```

Same pattern for the debounced preview (lines 96-107) and the delete preview (lines 129-139).

**Step 2: Verify frontend builds**

Run: `cd /workspace/frontend && npm run build`

**Step 3: Commit**

```
feat(frontend): use preview-raw API for subscription filter preview
```

---

### Task 12: Frontend — Raw item detail shows filter status

**Files:**
- Modify: `frontend/src/pages/raw-items/RawItemDialog.tsx`

**Step 1: Add filter_passed display to metadata grid**

In the metadata grid section (around line 71-98), add a new row after the status row:

```tsx
{item.filter_passed !== null && item.filter_passed !== undefined && (
  <>
    <p className="text-xs text-muted-foreground">{t("rawItems.filterStatus")}</p>
    <StatusBadge status={item.filter_passed ? "parsed" : "failed"} />
  </>
)}
```

Add i18n key `rawItems.filterStatus` to all three locale files:
- zh-TW: `"filterStatus": "篩選狀態"`
- en: `"filterStatus": "Filter Status"`
- ja: `"filterStatus": "フィルター状態"`

Import `StatusBadge` if not already imported.

**Step 2: Commit**

```
feat(frontend): show filter pass/fail status in raw item detail
```

---

### Task 13: Final verification and build

**Step 1: Build backend**

Run: `cargo build`
Expected: All workspace crates compile.

**Step 2: Run backend tests**

Run: `cargo test`
Expected: All tests pass.

**Step 3: Build frontend**

Run: `cd /workspace/frontend && npm run build`
Expected: Builds successfully.

**Step 4: Final commit (if any lint/type fixes needed)**

```
chore: fix lint and type errors from subscription UI improvements
```
