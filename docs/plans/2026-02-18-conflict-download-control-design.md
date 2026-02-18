# Conflict Download Control & Frontend Display Design

Date: 2026-02-18

## Background

The core service manages anime links where "conflicts" occur when the same episode (same `series_id` + `group_id` + `episode_no`) has more than one active, unfiltered download link. Currently, conflicted links are still dispatched to the downloader, which wastes bandwidth and may cause duplicate downloads.

Two types of conflicts exist in the system:
- **Subscription conflicts** (`subscription_conflicts` table): Multiple fetchers can handle one RSS subscription. Managed via `/api/core/conflicts`. **This endpoint IS used by the frontend and must NOT be removed.**
- **Anime link conflicts** (`anime_link_conflicts` table): Same episode has multiple download sources. Managed via `/api/core/link-conflicts`. Backend is fully implemented but frontend has no UI yet.

## Requirements

1. **Block conflict downloads**: Links with `conflict_flag = true` must not be dispatched to the downloader.
2. **Cancel on filter change**: When filter recalculation marks links as `filtered_flag = true`, any in-progress (pending/downloading) downloads for those links must be cancelled via the Downloader API.
3. **Cancel on conflict resolution**: When a conflict is resolved by choosing one link, cancel downloads for all unchosen links; then dispatch the chosen link if it passes filter.
4. **Frontend conflict display**: Show a conflict indicator on conflicted links in `AnimeSeriesDialog`; clicking it opens an `AnimeLinkDetailDialog` showing the conflicting related links.

## Architecture

### New Service: `DownloadCancelService`

File: `core-service/src/services/download_cancel.rs`

```
cancel_downloads_for_links(pool, http_client, link_ids: &[i32])
  → Query downloads WHERE link_id IN link_ids AND status IN ('pending', 'downloading')
  → Group by module_id to find the responsible downloader
  → POST {downloader}/downloads/cancel with BatchCancelRequest { hashes }
  → UPDATE downloads SET status = 'cancelled' for cancelled records
```

Pattern follows existing cancel logic in `subscriptions.rs`.

### Modified: `download_dispatch.rs`

Add `conflict_flag = false` filter alongside existing `filtered_flag = false`:

```rust
.filter(anime_links::filtered_flag.eq(false))
.filter(anime_links::conflict_flag.eq(false))  // NEW
```

### Modified: `filter_recalc.rs`

After recalculating filtered flags, track which link_ids had `filtered_flag` change from `false → true`. Call `cancel_downloads_for_links(newly_filtered_ids)`.

### Modified: `anime_link_conflicts.rs` resolve handler

When resolving a conflict:
1. Get all link_ids in the conflict group (excluding chosen_link_id)
2. Call `cancel_downloads_for_links(unchosen_link_ids)`
3. If `chosen_link_id.filtered_flag = false`, call `dispatch_new_links([chosen_link_id])`

### Modified: `AnimeLinkRichResponse` DTO

Add fields to the rich response struct and DB query:
- `conflict_flag: bool`
- `conflicting_link_ids: Vec<i32>` (other link_ids sharing the same conflict group, populated via JOIN with `anime_link_conflicts`)

## Frontend Changes

### `AnimeLinkRich` schema (`schemas/anime.ts`)

```typescript
conflict_flag: z.boolean(),
conflicting_link_ids: z.array(z.number()),
```

### `AnimeSeriesDialog`

For links where `conflict_flag === true`, display an orange Warning badge. Clicking the badge opens `AnimeLinkDetailDialog` for that link.

### New: `AnimeLinkDetailDialog`

Displays the anime link's details and a list of conflicting links (cross-referenced from the already-loaded links list using `conflicting_link_ids`). Each conflicting link shows its title/name and a button to view its own detail dialog.

## API Notes

- `/api/core/conflicts` — subscription conflicts, used by frontend, do NOT remove
- `/api/core/link-conflicts` — anime link conflicts, backend complete, frontend display being added in this work
- Cancel endpoint on downloader: `POST {base_url}/downloads/cancel` with `BatchCancelRequest { hashes: Vec<String> }`
- No new API endpoints needed for frontend: `conflicting_link_ids` embedded in existing `GET /api/core/links/:series_id` response
