# Subscription Editing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow editing subscription name, fetch_interval_minutes, and is_active in the SubscriptionDialog using the same edit-mode pattern as AnimeSeriesDialog.

**Architecture:** Expand backend UpdateSubscriptionRequest to accept additional fields, remove is_active filter from list_subscriptions, and rework SubscriptionDialog to use pencil/edit/save/cancel mode instead of EditableText inline editing.

**Tech Stack:** Rust/Diesel (backend), React/TypeScript/Effect (frontend), shadcn/ui components

---

### Task 1: Backend — Expand UpdateSubscriptionRequest

**Files:**
- Modify: `core-service/src/handlers/subscriptions.rs:83-86` (UpdateSubscriptionRequest struct)
- Modify: `core-service/src/handlers/subscriptions.rs:608-684` (update_subscription handler)

**Step 1: Expand the UpdateSubscriptionRequest struct**

Change lines 83-86 from:
```rust
#[derive(Debug, serde::Deserialize)]
pub struct UpdateSubscriptionRequest {
    pub name: Option<String>,
}
```
to:
```rust
#[derive(Debug, serde::Deserialize)]
pub struct UpdateSubscriptionRequest {
    pub name: Option<String>,
    pub fetch_interval_minutes: Option<i32>,
    pub is_active: Option<bool>,
}
```

**Step 2: Rewrite the update_subscription handler**

Replace the handler body (lines 629-646) to build a dynamic update. Replace the `if let Some(ref name)...else return 400` block with logic that builds a tuple of set clauses for all provided fields:

```rust
    let now = Utc::now().naive_utc();

    // Check at least one field is provided
    if payload.name.is_none() && payload.fetch_interval_minutes.is_none() && payload.is_active.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "no_fields",
                "message": "No fields to update"
            })),
        );
    }

    // Build changeset — always update updated_at
    let mut name_val = None;
    let mut interval_val = None;
    let mut active_val = None;

    if let Some(ref n) = payload.name {
        name_val = Some(n.as_str());
    }
    if let Some(i) = payload.fetch_interval_minutes {
        interval_val = Some(i);
    }
    if let Some(a) = payload.is_active {
        active_val = Some(a);
    }

    let target = subscriptions::table.filter(subscriptions::subscription_id.eq(subscription_id));

    // Apply updates using separate queries for each provided field + updated_at
    let result = conn.transaction::<_, diesel::result::Error, _>(|conn| {
        diesel::update(target.clone())
            .set(subscriptions::updated_at.eq(now))
            .execute(conn)?;

        if let Some(name) = name_val {
            diesel::update(target.clone())
                .set(subscriptions::name.eq(Some(name)))
                .execute(conn)?;
        }
        if let Some(interval) = interval_val {
            diesel::update(target.clone())
                .set(subscriptions::fetch_interval_minutes.eq(interval))
                .execute(conn)?;
        }
        if let Some(active) = active_val {
            diesel::update(target.clone())
                .set(subscriptions::is_active.eq(active))
                .execute(conn)?;
        }
        Ok(1usize)
    });
```

Keep the existing match on `result` below (lines 648-683) as-is.

Note: You need to add `use diesel::Connection;` at the top if not already imported, for `.transaction()`.

**Step 3: Verify compilation**

Run: `cd /workspace/core-service && cargo check 2>&1 | head -30`

**Step 4: Commit**

```bash
git add core-service/src/handlers/subscriptions.rs
git commit -m "feat(core): expand subscription update to support interval and is_active"
```

---

### Task 2: Backend — Remove is_active filter from list_subscriptions

**Files:**
- Modify: `core-service/src/handlers/subscriptions.rs:413-474` (list_subscriptions handler)

**Step 1: Remove the is_active filter**

Change line 420 from:
```rust
                .filter(subscriptions::is_active.eq(true))
```
to removing that line entirely, so the query becomes:
```rust
            match subscriptions::table
                .select(Subscription::as_select())
                .load::<Subscription>(&mut conn)
```

Also update the log message on line 446 from `"Listed {} active subscriptions"` to `"Listed {} subscriptions"`.

**Step 2: Verify compilation**

Run: `cd /workspace/core-service && cargo check 2>&1 | head -30`

**Step 3: Commit**

```bash
git add core-service/src/handlers/subscriptions.rs
git commit -m "feat(core): list all subscriptions including inactive ones"
```

---

### Task 3: Frontend — Expand CoreApi updateSubscription type

**Files:**
- Modify: `frontend/src/services/CoreApi.ts:81` (updateSubscription type)

**Step 1: Expand the request type**

Change line 81 from:
```typescript
    readonly updateSubscription: (id: number, req: { name?: string }) => Effect.Effect<Subscription>
```
to:
```typescript
    readonly updateSubscription: (id: number, req: { name?: string; fetch_interval_minutes?: number; is_active?: boolean }) => Effect.Effect<Subscription>
```

**Step 2: Commit**

```bash
git add frontend/src/services/CoreApi.ts
git commit -m "feat(frontend): expand updateSubscription API type for interval and is_active"
```

---

### Task 4: Frontend — Rework SubscriptionDialog with edit mode

**Files:**
- Modify: `frontend/src/pages/subscriptions/SubscriptionDialog.tsx` (full rework)

**Step 1: Rewrite SubscriptionDialog**

Replace the entire file content with:

```tsx
import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"
import { ParserEditor } from "@/components/shared/ParserEditor"
import { CopyButton } from "@/components/shared/CopyButton"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Pencil, Save, X } from "lucide-react"
import { toast } from "sonner"
import type { Subscription } from "@/schemas/subscription"

interface SubscriptionDialogProps {
  subscription: Subscription
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubscriptionChange?: () => void
}

export function SubscriptionDialog({ subscription, open, onOpenChange, onSubscriptionChange }: SubscriptionDialogProps) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  const [editForm, setEditForm] = useState({
    name: subscription.name ?? "",
    fetch_interval_minutes: subscription.fetch_interval_minutes,
    is_active: subscription.is_active,
  })

  const { mutate: doUpdate, isLoading: saving } = useEffectMutation(
    (req: { name?: string; fetch_interval_minutes?: number; is_active?: boolean }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.updateSubscription(subscription.subscription_id, req)
      }),
  )

  const handleSave = () => {
    doUpdate({
      name: editForm.name || undefined,
      fetch_interval_minutes: editForm.fetch_interval_minutes,
      is_active: editForm.is_active,
    }).then(() => {
      toast.success(t("common.saved", "Saved"))
      setEditing(false)
      onSubscriptionChange?.()
    }).catch(() => {
      toast.error(t("common.saveFailed", "Save failed"))
    })
  }

  return (
    <FullScreenDialog
      open={open}
      onOpenChange={onOpenChange}
      title={subscription.name ?? String(subscription.source_url)}
    >
      <div className="space-y-6">
        {/* Source URL — standalone at top */}
        <div>
          <p className="text-xs text-muted-foreground mb-1">{t("subscriptions.sourceUrl", "Source URL")}</p>
          <div className="flex items-start gap-1 bg-muted/50 rounded p-2">
            <p className="text-sm font-mono break-all flex-1">{String(subscription.source_url)}</p>
            <CopyButton text={String(subscription.source_url)} />
          </div>
        </div>

        {/* Subscription info with edit mode */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-medium text-muted-foreground">{t("dialog.info", "Info")}</h3>
            {!editing ? (
              <Button variant="ghost" size="sm" onClick={() => {
                setEditForm({
                  name: subscription.name ?? "",
                  fetch_interval_minutes: subscription.fetch_interval_minutes,
                  is_active: subscription.is_active,
                })
                setEditing(true)
              }}>
                <Pencil className="h-3.5 w-3.5 mr-1" />
                {t("common.edit", "Edit")}
              </Button>
            ) : (
              <div className="flex gap-1">
                <Button variant="ghost" size="sm" onClick={() => setEditing(false)} disabled={saving}>
                  <X className="h-3.5 w-3.5 mr-1" />
                  {t("common.cancel", "Cancel")}
                </Button>
                <Button size="sm" onClick={handleSave} disabled={saving}>
                  <Save className="h-3.5 w-3.5 mr-1" />
                  {t("common.save", "Save")}
                </Button>
              </div>
            )}
          </div>

          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <InfoItem label={t("common.id")} value={String(subscription.subscription_id)} />
            {editing ? (
              <>
                <div>
                  <p className="text-xs text-muted-foreground mb-1">{t("common.name")}</p>
                  <Input
                    value={editForm.name}
                    onChange={(e) => setEditForm((f) => ({ ...f, name: e.target.value }))}
                    placeholder={t("subscriptions.name")}
                    className="h-8 text-sm"
                  />
                </div>
                <div>
                  <p className="text-xs text-muted-foreground mb-1">{t("subscriptions.interval", "Interval")}</p>
                  <div className="flex items-center gap-1">
                    <Input
                      type="number"
                      min={1}
                      value={editForm.fetch_interval_minutes}
                      onChange={(e) => setEditForm((f) => ({ ...f, fetch_interval_minutes: Number(e.target.value) }))}
                      className="h-8 text-sm w-20"
                    />
                    <span className="text-xs text-muted-foreground">min</span>
                  </div>
                </div>
                <div>
                  <p className="text-xs text-muted-foreground mb-1">{t("common.status")}</p>
                  <div className="flex items-center gap-2 h-8">
                    <Switch
                      checked={editForm.is_active}
                      onCheckedChange={(checked) => setEditForm((f) => ({ ...f, is_active: checked }))}
                    />
                    <span className="text-sm">{editForm.is_active ? "Active" : "Inactive"}</span>
                  </div>
                </div>
              </>
            ) : (
              <>
                <InfoItem label={t("common.name")} value={subscription.name ?? "-"} />
                <InfoItem label={t("subscriptions.interval", "Interval")} value={`${subscription.fetch_interval_minutes} min`} />
                <InfoItem
                  label={t("common.status")}
                  value={subscription.is_active ? "Active" : "Inactive"}
                />
              </>
            )}
            <InfoItem
              label={t("subscriptions.lastFetched", "Last Fetched")}
              value={subscription.last_fetched_at ? String(subscription.last_fetched_at).slice(0, 19).replace("T", " ") : t("common.never")}
            />
          </div>
        </div>

        {/* Sub-tabs for filter rules and parsers */}
        <Tabs defaultValue="filters">
          <TabsList variant="line">
            <TabsTrigger value="filters">{t("dialog.filterRules", "Filter Rules")}</TabsTrigger>
            <TabsTrigger value="parsers">{t("dialog.parsers", "Parsers")}</TabsTrigger>
          </TabsList>
          <TabsContent value="filters" className="mt-4">
            <FilterRuleEditor
              targetType="fetcher"
              targetId={subscription.subscription_id}
            />
          </TabsContent>
          <TabsContent value="parsers" className="mt-4">
            <ParserEditor
              createdFromType="subscription"
              createdFromId={subscription.subscription_id}
            />
          </TabsContent>
        </Tabs>
      </div>
    </FullScreenDialog>
  )
}

function InfoItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="text-sm font-medium break-all">{value}</p>
    </div>
  )
}
```

**Step 2: Verify frontend builds**

Run: `cd /workspace/frontend && npx tsc --noEmit 2>&1 | head -30`

**Step 3: Commit**

```bash
git add frontend/src/pages/subscriptions/SubscriptionDialog.tsx
git commit -m "feat(frontend): rework SubscriptionDialog with edit mode for name, interval, and status"
```

---

### Task 5: Cleanup — Remove EditableText import if unused elsewhere

**Files:**
- Check: `frontend/src/pages/subscriptions/SubscriptionDialog.tsx` (already done in Task 4 — EditableText removed)

**Step 1: Verify EditableText is still used elsewhere**

Run: `grep -r "EditableText" frontend/src/ --include="*.tsx" --include="*.ts"` to check if it's used in other files. If it's only defined in `EditableText.tsx` and no longer imported anywhere, leave it in place (it's a shared component that might be used later).

**Step 2: Verify the full app builds**

Run: `cd /workspace/frontend && npx tsc --noEmit 2>&1 | head -30`

**Step 3: Commit (if any changes)**

Only commit if there were changes to make.

---

### Task 6: End-to-end verification

**Step 1: Build backend**

Run: `cd /workspace/core-service && cargo check 2>&1 | head -30`

**Step 2: Build frontend**

Run: `cd /workspace/frontend && npx tsc --noEmit 2>&1 | head -30`

**Step 3: Final commit if needed**

Fix any issues found during verification.
