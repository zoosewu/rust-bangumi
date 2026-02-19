# Cancelled Item Re-dispatch Design

## Problem

When items are cancelled (via filter changes, parser changes, or conflict resolution),
they are never re-dispatched even when they become eligible again.

## Three Trigger Points

### 1. Filter Rule Changes
- **Cancel**: Already implemented (newly_filtered links → cancel downloads)
- **Re-dispatch**: MISSING — newly_unfiltered links should be dispatched

### 2. Parser Changes (Create/Update/Delete)
- **Cancel**: MISSING — links that become filtered after reparse
- **Re-dispatch**: MISSING — existing links that become eligible after reparse

### 3. Conflict Auto-Resolution
- **Re-dispatch**: MISSING — when conflict group shrinks to ≤1 link, remaining
  link should be set back to `active` and dispatched

## Changes

### 1. `services/filter_recalc.rs`
- `recalculate_filtered_flags` returns `(count, newly_filtered_ids, newly_unfiltered_ids)`
- `newly_unfiltered` = links whose `filtered_flag` changed `true → false`

### 2. `services/download_dispatch.rs`
- `dispatch_new_links` filters out links that already have an active download
  (`status IN ('downloading', 'completed', 'syncing', 'synced')`)

### 3. `handlers/filters.rs`
- After recalc, call `dispatch_new_links(newly_unfiltered)` in addition to
  cancelling newly_filtered

### 4. `handlers/parsers.rs`
- In `reparse_affected_items`, track links whose filtered_flag changed
- Cancel downloads for newly filtered links
- After conflict detection, dispatch all updated (non-new) link_ids that are eligible

### 5. `services/conflict_detection.rs`
- `detect_and_mark_conflicts` returns `(conflicts_found, auto_dispatch_link_ids)`
- When auto-resolving: restore remaining link's `link_status` to `'active'`
- Return these link_ids for dispatch by the caller

### 6. All callers of `detect_and_mark_conflicts`
- Handle the returned `auto_dispatch_link_ids` by calling `dispatch_new_links`
