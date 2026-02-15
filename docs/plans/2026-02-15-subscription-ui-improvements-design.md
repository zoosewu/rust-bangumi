# Subscription UI Improvements Design

## Overview

Four improvements to the subscription management frontend:
1. Soft/hard delete options with clear user-facing language
2. Editable subscription name (shared component with anime page)
3. Filter preview operates on raw_anime_items (including failed) instead of anime_links
4. Raw item detail shows filter pass/fail status

---

## 1. Delete Subscription Dialog

### Current
Single ConfirmDialog with one "Delete" button, shows pending/failed raw item count.

### New
Replace with a custom `DeleteSubscriptionDialog`:

- **Title**: "刪除訂閱" / "Delete Subscription"
- **Description**: Shows subscription name
- **Two action buttons**:
  - "停用訂閱" (outline button) — calls `DELETE /subscriptions/:id` (soft delete, sets `is_active=false`)
  - "完全刪除（含已下載動畫）" (destructive button) — calls `DELETE /subscriptions/:id?purge=true`, with explicit warning that downloaded anime files will be removed
- **Cancel** button to dismiss

### API Changes
- `deleteSubscription(id: number)` → `deleteSubscription(id: number, purge?: boolean)`
- ApiLayer: append `?purge=true` when purge flag is set

### i18n Keys
- `subscriptions.deactivate`: "停用訂閱"
- `subscriptions.purgeDelete`: "完全刪除（含已下載動畫）"
- `subscriptions.deactivateConfirm`: "訂閱將停用，所有資料保留。"
- `subscriptions.purgeConfirm`: "將刪除此訂閱的所有資料，包含已下載的動畫檔案，此操作無法復原。"

---

## 2. Editable Subscription Name

### Shared Component: `EditableText`
Reusable inline-edit component used by both SubscriptionDialog and AnimeDialog.

**Props**:
```typescript
interface EditableTextProps {
  value: string
  onSave: (newValue: string) => Promise<void>
  placeholder?: string
  className?: string
}
```

**Behavior**:
- Display mode: text + pencil icon
- Click → inline input with current value
- Enter → save (calls onSave), Escape → cancel
- Shows loading state during save

### Backend: `PATCH /subscriptions/:id`
New endpoint in `core-service/src/handlers/subscriptions.rs`:
```rust
pub struct UpdateSubscriptionRequest {
    pub name: Option<String>,
}
```
Updates only provided fields (currently just `name`).

### Frontend Integration
- `SubscriptionDialog`: name InfoItem → `EditableText`
- `AnimeDialog`: title InfoItem → `EditableText` (bonus, using existing `updateAnime` or new endpoint)
- New API methods: `updateSubscriptionName(id, name)` in CoreApi

---

## 3. Filter Preview on Raw Items

### Current
`POST /filters/preview` operates on `anime_links` — shows which links pass/fail the filter rule.

### New: `POST /filters/preview-raw`
New backend endpoint that operates on `raw_anime_items` instead of `anime_links`.

**Request** (same as existing preview):
```typescript
{
  target_type: string       // "subscription"
  target_id: number | null  // subscription_id
  regex_pattern: string
  is_positive: boolean
  exclude_filter_id?: number
}
```

**Response** (enhanced PreviewItem):
```typescript
interface RawPreviewItem {
  item_id: number
  title: string
  status: string  // "pending" | "parsed" | "failed" | "no_match" | "skipped"
}

interface FilterPreviewResponse {
  regex_valid: boolean
  regex_error: string | null
  before: { passed_items: RawPreviewItem[], filtered_items: RawPreviewItem[] }
  after: { passed_items: RawPreviewItem[], filtered_items: RawPreviewItem[] }
}
```

### Frontend Changes
- `FilterPreviewPanel`: each item shows a `StatusBadge` next to the title
- `FilterRuleEditor`: when `targetType === "subscription"`, call `preview-raw` instead of `preview`
- For other target types (anime, anime_series, etc.), keep using existing `preview` endpoint on anime_links

### Backend Implementation
- New handler `preview_filter_raw()` in `filters.rs`
- Queries `raw_anime_items` by `subscription_id` (from target_id)
- Applies filter rules to each item's `title` field
- Returns items with their status

---

## 4. Raw Item Detail: Filter Status

### Current
`RawItemDialog` shows status, error, download info, but no filter pass/fail indication.

### New
Add a "篩選狀態" / "Filter Status" field in the metadata grid.

**Approach**: Backend adds `filter_passed: bool` to the raw item detail response.

### Backend
In `GET /raw-items/:item_id` response, add:
- Query all applicable filter rules for the item's subscription
- Apply filter engine to the item's title
- Return `filter_passed: boolean` field

### Frontend
- `RawItemDialog` metadata grid: new row with StatusBadge
  - `filter_passed === true` → green "通過" badge
  - `filter_passed === false` → red "已過濾" badge

---

## Files to Modify

### Backend (core-service)
1. `handlers/subscriptions.rs` — add `PATCH /subscriptions/:id` handler
2. `handlers/filters.rs` — add `preview_filter_raw()` handler
3. `handlers/raw_items.rs` — add `filter_passed` to raw item detail response
4. `main.rs` — add new routes

### Frontend
1. **New**: `components/shared/EditableText.tsx` — reusable inline edit
2. **New**: `components/shared/DeleteSubscriptionDialog.tsx` — dual-action delete dialog
3. `pages/subscriptions/SubscriptionsPage.tsx` — use new delete dialog, API changes
4. `pages/subscriptions/SubscriptionDialog.tsx` — editable name
5. `pages/anime/AnimeDialog.tsx` — editable title (reuse EditableText)
6. `components/shared/FilterRuleEditor.tsx` — call preview-raw for subscription target
7. `components/shared/FilterPreviewPanel.tsx` — show status badge per item
8. `pages/raw-items/RawItemDialog.tsx` — show filter status
9. `services/CoreApi.ts` — add new API methods
10. `layers/ApiLayer.ts` — implement new API calls
11. `schemas/` — update types
12. `i18n/zh-TW.json`, `en.json`, `ja.json` — new translation keys
