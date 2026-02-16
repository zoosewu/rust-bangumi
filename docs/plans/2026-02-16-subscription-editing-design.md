# Subscription Editing Design

## Goal
Allow editing subscription name, fetch interval, and active status in the SubscriptionDialog, using the same edit-mode pattern as AnimeSeriesDialog.

## Current State
- SubscriptionDialog: name editable via EditableText inline; interval and is_active are read-only
- Backend `update_subscription` only accepts `name` field
- `list_subscriptions` filters to `is_active = true` only
- Delete dialog offers "Deactivate" (soft delete) and "Purge" (hard delete)

## Design

### Backend Changes

1. **Expand `UpdateSubscriptionRequest`** in `core-service/src/handlers/subscriptions.rs`:
   - Add `fetch_interval_minutes: Option<i32>`
   - Add `is_active: Option<bool>`
   - Update handler to apply all provided fields

2. **Modify `list_subscriptions`**: Remove `is_active = true` filter so deactivated subscriptions are visible and can be reactivated.

### Frontend Changes

1. **Expand `CoreApi.updateSubscription`**: Accept `fetch_interval_minutes` and `is_active` in the request body.

2. **Rework `SubscriptionDialog`**:
   - Remove `EditableText` for name
   - Add edit-mode toggle (pencil icon → edit mode → cancel/save), same pattern as AnimeSeriesDialog
   - View mode: InfoItem display for all fields
   - Edit mode:
     - Name → text input
     - Interval → number input (minutes)
     - Status → toggle switch (active/inactive)
   - Save calls `updateSubscription` with changed fields
   - Cancel restores original values

3. **SubscriptionsPage table**: StatusBadge will now show inactive subscriptions in the list.

### Unchanged
- Delete dialog (deactivate + purge) remains as-is
- AnimeSeriesDialog not modified
- FilterRuleEditor / ParserEditor tabs unchanged
