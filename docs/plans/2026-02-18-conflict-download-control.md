# Conflict Download Control & Frontend Display Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ensure conflicted anime links are never dispatched to the downloader, cancel in-progress downloads when links get filtered out or conflict is resolved, and display conflict status in the frontend.

**Architecture:** New `DownloadCancelService` centralises all cancellation logic (query DB for active downloads → call downloader cancel API → mark as cancelled). `filter_recalc` returns which link IDs became newly-filtered so callers can trigger cancellation. The resolve-conflict handler gains post-resolve dispatch + cancel steps. Frontend `AnimeLinkRich` gains `conflict_flag` and `conflicting_link_ids` fields; `AnimeSeriesDialog` shows a conflict badge that opens a detail dialog.

**Tech Stack:** Rust / Axum / Diesel (PostgreSQL), React / TypeScript / Effect, shadcn/ui

---

### Task 1: Add `DownloadCancelService`

**Files:**
- Create: `core-service/src/services/download_cancel.rs`
- Modify: `core-service/src/services/mod.rs`
- Modify: `core-service/src/state.rs`

**Step 1: Create the service file**

```rust
// core-service/src/services/download_cancel.rs
use crate::db::DbPool;
use crate::schema::{downloads, service_modules};
use diesel::prelude::*;
use shared::BatchCancelRequest;

pub struct DownloadCancelService {
    db_pool: DbPool,
    http_client: reqwest::Client,
}

impl DownloadCancelService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            http_client: reqwest::Client::new(),
        }
    }

    /// Cancel in-progress downloads for the given link IDs.
    /// Calls the downloader cancel API and marks DB records as 'cancelled'.
    /// Silently skips links with no active downloads.
    pub async fn cancel_downloads_for_links(&self, link_ids: &[i32]) -> Result<usize, String> {
        if link_ids.is_empty() {
            return Ok(0);
        }

        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        // Find active (downloading) download records for these links
        #[derive(Queryable)]
        struct ActiveDownload {
            download_id: i32,
            module_id: Option<i32>,
            torrent_hash: Option<String>,
        }

        let active: Vec<ActiveDownload> = downloads::table
            .filter(downloads::link_id.eq_any(link_ids))
            .filter(downloads::status.eq("downloading"))
            .select((
                downloads::download_id,
                downloads::module_id,
                downloads::torrent_hash,
            ))
            .load::<(i32, Option<i32>, Option<String>)>(&mut conn)
            .map_err(|e| format!("Failed to query active downloads: {}", e))?
            .into_iter()
            .map(|(download_id, module_id, torrent_hash)| ActiveDownload {
                download_id,
                module_id,
                torrent_hash,
            })
            .collect();

        if active.is_empty() {
            return Ok(0);
        }

        // Group by module_id to batch cancel per downloader
        let mut by_module: std::collections::HashMap<i32, Vec<(i32, String)>> =
            std::collections::HashMap::new();
        let mut no_module: Vec<i32> = Vec::new();

        for d in &active {
            match (d.module_id, d.torrent_hash.as_deref()) {
                (Some(mid), Some(hash)) => {
                    by_module
                        .entry(mid)
                        .or_default()
                        .push((d.download_id, hash.to_string()));
                }
                _ => no_module.push(d.download_id),
            }
        }

        let mut cancelled = 0;

        // Cancel per downloader
        for (module_id, items) in &by_module {
            let base_url: Option<String> = service_modules::table
                .filter(service_modules::module_id.eq(module_id))
                .select(service_modules::base_url)
                .first::<Option<String>>(&mut conn)
                .optional()
                .ok()
                .flatten()
                .flatten();

            if let Some(url) = base_url {
                let hashes: Vec<String> = items.iter().map(|(_, h)| h.clone()).collect();
                let cancel_url = format!("{}/downloads/cancel", url);
                let req = BatchCancelRequest { hashes };
                match self
                    .http_client
                    .post(&cancel_url)
                    .json(&req)
                    .timeout(std::time::Duration::from_secs(10))
                    .send()
                    .await
                {
                    Ok(_) => {
                        tracing::info!(
                            "Cancelled {} torrents via module {}",
                            items.len(),
                            module_id
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Best-effort cancel failed for module {}: {}", module_id, e);
                    }
                }
            }

            // Mark as cancelled in DB regardless of API result
            let download_ids: Vec<i32> = items.iter().map(|(id, _)| *id).collect();
            diesel::update(downloads::table.filter(downloads::download_id.eq_any(&download_ids)))
                .set(downloads::status.eq("cancelled"))
                .execute(&mut conn)
                .map_err(|e| format!("Failed to update download status: {}", e))?;
            cancelled += items.len();
        }

        // Mark no-module records as cancelled too (no torrent hash = never sent to downloader)
        if !no_module.is_empty() {
            diesel::update(downloads::table.filter(downloads::download_id.eq_any(&no_module)))
                .set(downloads::status.eq("cancelled"))
                .execute(&mut conn)
                .map_err(|e| format!("Failed to update no-module download status: {}", e))?;
            cancelled += no_module.len();
        }

        Ok(cancelled)
    }
}
```

**Step 2: Export from mod.rs**

In `core-service/src/services/mod.rs`, add:
```rust
pub mod download_cancel;
pub use download_cancel::DownloadCancelService;
```

**Step 3: Add to AppState**

In `core-service/src/state.rs`:
- Add `use crate::services::DownloadCancelService;` to the imports
- Add `pub cancel_service: Arc<DownloadCancelService>` to `AppState` struct
- In `AppState::new`, add: `let cancel_service = DownloadCancelService::new(db.clone());`
- Add `cancel_service: Arc::new(cancel_service)` to the `Self { ... }` block

**Step 4: Build to verify**

```bash
cd /workspace/core-service && cargo build 2>&1 | head -50
```
Expected: builds successfully (no errors).

**Step 5: Commit**

```bash
cd /workspace && git add core-service/src/services/download_cancel.rs core-service/src/services/mod.rs core-service/src/state.rs
git commit -m "feat(core): add DownloadCancelService for cancelling in-progress downloads"
```

---

### Task 2: Block dispatch of conflicted links

**Files:**
- Modify: `core-service/src/services/download_dispatch.rs:42-46`

**Step 1: Add `conflict_flag` filter**

In `dispatch_new_links`, the existing query loads anime_links at line 42-46. Add one filter line:

```rust
// BEFORE:
let links: Vec<AnimeLink> = anime_links::table
    .filter(anime_links::link_id.eq_any(&link_ids))
    .filter(anime_links::filtered_flag.eq(false))
    .load::<AnimeLink>(&mut conn)
    .map_err(|e| format!("Failed to load anime links: {}", e))?;

// AFTER:
let links: Vec<AnimeLink> = anime_links::table
    .filter(anime_links::link_id.eq_any(&link_ids))
    .filter(anime_links::filtered_flag.eq(false))
    .filter(anime_links::conflict_flag.eq(false))
    .load::<AnimeLink>(&mut conn)
    .map_err(|e| format!("Failed to load anime links: {}", e))?;
```

**Step 2: Build**

```bash
cd /workspace/core-service && cargo build 2>&1 | head -30
```
Expected: compiles with no errors.

**Step 3: Commit**

```bash
cd /workspace && git add core-service/src/services/download_dispatch.rs
git commit -m "fix(core): exclude conflicted links from download dispatch"
```

---

### Task 3: Cancel downloads when filter changes

**Files:**
- Modify: `core-service/src/services/filter_recalc.rs`
- Modify: `core-service/src/handlers/filters.rs`

**Step 1: Change return type of `recalculate_filtered_flags`**

The function currently returns `Result<usize, String>`. Change it to return `Result<(usize, Vec<i32>), String>` where the `Vec<i32>` is link IDs that were newly filtered out (changed from `false` to `true`).

Find this section (line ~29-48) and update:

```rust
// BEFORE:
    let mut updated = 0;

    for link in &affected_links {
        // 2. Collect all applicable rules for this link
        let rules = collect_all_rules_for_link(conn, link)?;

        // 3. Evaluate
        let engine = FilterEngine::with_priority_sorted(rules);
        let title = link.title.as_deref().unwrap_or("");
        // filtered_flag = true means filtered OUT (should NOT be included)
        let should_include = engine.should_include(title);
        let new_flag = !should_include;

        if new_flag != link.filtered_flag {
            diesel::update(anime_links::table.filter(anime_links::link_id.eq(link.link_id)))
                .set(anime_links::filtered_flag.eq(new_flag))
                .execute(conn)
                .map_err(|e| format!("Failed to update filtered_flag for link {}: {}", link.link_id, e))?;
            updated += 1;
        }
    }

    tracing::info!(
        "filter_recalc: checked {} links, updated {} for {:?}/{:?}",
        affected_links.len(),
        updated,
        target_type,
        target_id
    );

    Ok(updated)

// AFTER:
    let mut updated = 0;
    let mut newly_filtered: Vec<i32> = Vec::new();

    for link in &affected_links {
        let rules = collect_all_rules_for_link(conn, link)?;
        let engine = FilterEngine::with_priority_sorted(rules);
        let title = link.title.as_deref().unwrap_or("");
        let should_include = engine.should_include(title);
        let new_flag = !should_include;

        if new_flag != link.filtered_flag {
            diesel::update(anime_links::table.filter(anime_links::link_id.eq(link.link_id)))
                .set(anime_links::filtered_flag.eq(new_flag))
                .execute(conn)
                .map_err(|e| format!("Failed to update filtered_flag for link {}: {}", link.link_id, e))?;
            updated += 1;
            // Track links that became filtered (false → true): these need download cancellation
            if new_flag {
                newly_filtered.push(link.link_id);
            }
        }
    }

    tracing::info!(
        "filter_recalc: checked {} links, updated {} ({} newly filtered) for {:?}/{:?}",
        affected_links.len(),
        updated,
        newly_filtered.len(),
        target_type,
        target_id
    );

    Ok((updated, newly_filtered))
```

Also update the function signature line:
```rust
// BEFORE:
pub fn recalculate_filtered_flags(
    conn: &mut PgConnection,
    target_type: FilterTargetType,
    target_id: Option<i32>,
) -> Result<usize, String> {

// AFTER:
pub fn recalculate_filtered_flags(
    conn: &mut PgConnection,
    target_type: FilterTargetType,
    target_id: Option<i32>,
) -> Result<(usize, Vec<i32>), String> {
```

**Step 2: Update callers in `filters.rs`**

There are two `tokio::spawn` blocks in `filters.rs` (one in `create_filter_rule` at line ~85-95, one in `delete_filter_rule` at line ~204-214). Both call `recalculate_filtered_flags`. Update both:

```rust
// BEFORE (in both create and delete handlers):
tokio::spawn(async move {
    if let Ok(mut conn) = db.get() {
        match crate::services::filter_recalc::recalculate_filtered_flags(&mut conn, tt, tid) {
            Ok(n) => tracing::info!("filter_recalc after create: updated {} links", n),
            Err(e) => tracing::error!("filter_recalc after create failed: {}", e),
        }
    }
    if let Err(e) = conflict_detection.detect_and_mark_conflicts().await {
        tracing::error!("conflict re-detection after filter create failed: {}", e);
    }
});

// AFTER — for create_filter_rule (adjust the log message for delete_filter_rule):
let cancel_service = state.cancel_service.clone();
tokio::spawn(async move {
    let newly_filtered = if let Ok(mut conn) = db.get() {
        match crate::services::filter_recalc::recalculate_filtered_flags(&mut conn, tt, tid) {
            Ok((n, newly_filtered)) => {
                tracing::info!("filter_recalc after create: updated {} links", n);
                newly_filtered
            }
            Err(e) => {
                tracing::error!("filter_recalc after create failed: {}", e);
                vec![]
            }
        }
    } else {
        vec![]
    };

    if !newly_filtered.is_empty() {
        match cancel_service.cancel_downloads_for_links(&newly_filtered).await {
            Ok(n) => tracing::info!("Cancelled {} downloads for newly filtered links", n),
            Err(e) => tracing::warn!("Failed to cancel downloads for filtered links: {}", e),
        }
    }

    if let Err(e) = conflict_detection.detect_and_mark_conflicts().await {
        tracing::error!("conflict re-detection after filter create failed: {}", e);
    }
});
```

Do the same for `delete_filter_rule`, changing `"after create"` to `"after delete"` in log messages.

**Step 3: Build**

```bash
cd /workspace/core-service && cargo build 2>&1 | head -50
```
Expected: no errors.

**Step 4: Commit**

```bash
cd /workspace && git add core-service/src/services/filter_recalc.rs core-service/src/handlers/filters.rs
git commit -m "feat(core): cancel downloads for links newly filtered out by rule changes"
```

---

### Task 4: Cancel unchosen downloads and dispatch chosen link on conflict resolve

**Files:**
- Modify: `core-service/src/handlers/anime_link_conflicts.rs`

**Step 1: Read the current handler**

The `resolve_link_conflict` handler (line 118-154) currently only calls `state.conflict_detection.resolve_conflict(...)`. We need to:
1. Query the conflict group's links BEFORE resolving (to know which are unchosen)
2. After resolve: cancel unchosen links' downloads
3. After resolve: dispatch chosen link (if it passes filter)

Replace the handler body with:

```rust
/// POST /link-conflicts/:id/resolve - Resolve a link conflict
pub async fn resolve_link_conflict(
    State(state): State<AppState>,
    Path(conflict_id): Path<i32>,
    Json(payload): Json<ResolveAnimeLinkConflictRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };

    // Step 1: Get the conflict to find all links in this group
    let conflict = match state.repos.anime_link_conflict.find_by_id(conflict_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "conflict_not_found" })),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };

    // Step 2: Get all active links in the conflict group to find unchosen link_ids
    let all_links: Vec<AnimeLink> = match anime_links::table
        .filter(anime_links::series_id.eq(conflict.series_id))
        .filter(anime_links::group_id.eq(conflict.group_id))
        .filter(anime_links::episode_no.eq(conflict.episode_no))
        .filter(anime_links::link_status.eq("active"))
        .load::<AnimeLink>(&mut conn)
    {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };

    let unchosen_ids: Vec<i32> = all_links
        .iter()
        .filter(|l| l.link_id != payload.chosen_link_id)
        .map(|l| l.link_id)
        .collect();

    // Step 3: Resolve via ConflictDetectionService (sets chosen link conflict_flag=false, others to 'resolved' status)
    match state
        .conflict_detection
        .resolve_conflict(conflict_id, payload.chosen_link_id)
        .await
    {
        Ok(()) => {
            tracing::info!(
                "Resolved link conflict {}: chosen link_id={}",
                conflict_id,
                payload.chosen_link_id
            );
        }
        Err(e) => {
            tracing::error!("Failed to resolve link conflict {}: {}", conflict_id, e);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "resolve_failed", "message": e })),
            );
        }
    }

    // Step 4: Cancel downloads for unchosen links
    if !unchosen_ids.is_empty() {
        if let Err(e) = state.cancel_service.cancel_downloads_for_links(&unchosen_ids).await {
            tracing::warn!("Failed to cancel unchosen downloads: {}", e);
        }
    }

    // Step 5: Dispatch chosen link if it passes filter (filtered_flag=false, conflict_flag=false)
    // dispatch_new_links already checks both flags, so just pass the chosen link ID
    let dispatch_result = state
        .dispatch_service
        .dispatch_new_links(vec![payload.chosen_link_id])
        .await;

    match &dispatch_result {
        Ok(r) => tracing::info!(
            "Dispatched chosen link {}: dispatched={}, no_downloader={}, failed={}",
            payload.chosen_link_id, r.dispatched, r.no_downloader, r.failed
        ),
        Err(e) => tracing::warn!("Failed to dispatch chosen link: {}", e),
    }

    (
        StatusCode::OK,
        Json(json!({
            "message": "Conflict resolved successfully",
            "conflict_id": conflict_id,
            "chosen_link_id": payload.chosen_link_id
        })),
    )
}
```

Note: Also add `use crate::models::AnimeLink;` and `use crate::schema::anime_links;` to the imports at the top of the file if not already present (check what's imported at line 14).

**Step 2: Build**

```bash
cd /workspace/core-service && cargo build 2>&1 | head -50
```
Expected: no errors.

**Step 3: Commit**

```bash
cd /workspace && git add core-service/src/handlers/anime_link_conflicts.rs
git commit -m "feat(core): cancel unchosen downloads and dispatch chosen link on conflict resolve"
```

---

### Task 5: Add `conflict_flag` and `conflicting_link_ids` to `AnimeLinkRichResponse`

**Files:**
- Modify: `core-service/src/dto.rs`
- Modify: `core-service/src/handlers/links.rs`

**Step 1: Update the DTO**

In `dto.rs`, find `AnimeLinkRichResponse` (line 140-152) and add two fields:

```rust
// BEFORE:
#[derive(Debug, Serialize, Clone)]
pub struct AnimeLinkRichResponse {
    pub link_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub group_name: String,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub download: Option<DownloadInfo>,
    pub created_at: NaiveDateTime,
}

// AFTER:
#[derive(Debug, Serialize, Clone)]
pub struct AnimeLinkRichResponse {
    pub link_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub group_name: String,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub conflict_flag: bool,
    pub conflicting_link_ids: Vec<i32>,
    pub download: Option<DownloadInfo>,
    pub created_at: NaiveDateTime,
}
```

**Step 2: Populate `conflicting_link_ids` in `get_anime_links` handler**

In `handlers/links.rs`, the `get_anime_links` function builds results in a loop (lines 103-133). We need to compute `conflicting_link_ids` efficiently without N+1 queries.

**Strategy:** After loading all links, build a conflict groups map in memory. For links with `conflict_flag=true`, their conflicting IDs are all other link_ids in the same `(group_id, episode_no)` bucket that also have `conflict_flag=true`.

Replace the result-building loop:

```rust
    // Build conflict group map: (group_id, episode_no) -> Vec<link_id> for conflicted links
    use std::collections::HashMap;
    let mut conflict_groups: HashMap<(i32, i32), Vec<i32>> = HashMap::new();
    for (link, _) in &links_with_groups {
        if link.conflict_flag {
            conflict_groups
                .entry((link.group_id, link.episode_no))
                .or_default()
                .push(link.link_id);
        }
    }

    let mut results = Vec::new();
    for (link, group) in &links_with_groups {
        let download_info: Option<DownloadInfo> = downloads::table
            .filter(downloads::link_id.eq(link.link_id))
            .order(downloads::updated_at.desc())
            .first::<Download>(&mut conn)
            .optional()
            .ok()
            .flatten()
            .map(|d| DownloadInfo {
                download_id: d.download_id,
                status: d.status,
                progress: d.progress,
                torrent_hash: d.torrent_hash,
            });

        let conflicting_link_ids = if link.conflict_flag {
            conflict_groups
                .get(&(link.group_id, link.episode_no))
                .map(|ids| ids.iter().filter(|&&id| id != link.link_id).cloned().collect())
                .unwrap_or_default()
        } else {
            vec![]
        };

        results.push(AnimeLinkRichResponse {
            link_id: link.link_id,
            series_id: link.series_id,
            group_id: link.group_id,
            group_name: group.group_name.clone(),
            episode_no: link.episode_no,
            title: link.title.clone(),
            url: link.url.clone(),
            source_hash: link.source_hash.clone(),
            filtered_flag: link.filtered_flag,
            conflict_flag: link.conflict_flag,
            conflicting_link_ids,
            download: download_info,
            created_at: link.created_at,
        });
    }
```

**Step 3: Build**

```bash
cd /workspace/core-service && cargo build 2>&1 | head -50
```
Expected: no errors.

**Step 4: Commit**

```bash
cd /workspace && git add core-service/src/dto.rs core-service/src/handlers/links.rs
git commit -m "feat(core): add conflict_flag and conflicting_link_ids to AnimeLinkRich response"
```

---

### Task 6: Update frontend `AnimeLinkRich` schema

**Files:**
- Modify: `frontend/src/schemas/anime.ts`

**Step 1: Add fields to `AnimeLinkRich`**

In `schemas/anime.ts`, find `AnimeLinkRich` (lines 85-98) and add the two new fields:

```typescript
// BEFORE:
export const AnimeLinkRich = Schema.Struct({
  link_id: Schema.Number,
  series_id: Schema.Number,
  group_id: Schema.Number,
  group_name: Schema.String,
  episode_no: Schema.Number,
  title: Schema.NullOr(Schema.String),
  url: Schema.String,
  source_hash: Schema.String,
  filtered_flag: Schema.Boolean,
  download: Schema.NullOr(DownloadInfo),
  created_at: Schema.String,
})
export type AnimeLinkRich = typeof AnimeLinkRich.Type

// AFTER:
export const AnimeLinkRich = Schema.Struct({
  link_id: Schema.Number,
  series_id: Schema.Number,
  group_id: Schema.Number,
  group_name: Schema.String,
  episode_no: Schema.Number,
  title: Schema.NullOr(Schema.String),
  url: Schema.String,
  source_hash: Schema.String,
  filtered_flag: Schema.Boolean,
  conflict_flag: Schema.Boolean,
  conflicting_link_ids: Schema.Array(Schema.Number),
  download: Schema.NullOr(DownloadInfo),
  created_at: Schema.String,
})
export type AnimeLinkRich = typeof AnimeLinkRich.Type
```

**Step 2: Build frontend to verify types**

```bash
cd /workspace/frontend && npm run build 2>&1 | tail -20
```
Expected: builds without TypeScript errors.

**Step 3: Commit**

```bash
cd /workspace && git add frontend/src/schemas/anime.ts
git commit -m "feat(frontend): add conflict_flag and conflicting_link_ids to AnimeLinkRich schema"
```

---

### Task 7: Add conflict badge to `AnimeSeriesDialog` and create `AnimeLinkDetailDialog`

**Files:**
- Create: `frontend/src/pages/anime-series/AnimeLinkDetailDialog.tsx`
- Modify: `frontend/src/pages/anime-series/AnimeSeriesDialog.tsx`

**Step 1: Create `AnimeLinkDetailDialog`**

This dialog opens when a user clicks the conflict badge on a link. It shows the link's basic info and a list of conflicting links (cross-referenced from the parent's already-loaded `links` array).

```tsx
// frontend/src/pages/anime-series/AnimeLinkDetailDialog.tsx
import { useTranslation } from "react-i18next"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Badge } from "@/components/ui/badge"
import { CopyButton } from "@/components/shared/CopyButton"
import type { AnimeLinkRich } from "@/schemas/anime"

interface AnimeLinkDetailDialogProps {
  link: AnimeLinkRich
  allLinks: AnimeLinkRich[]
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function AnimeLinkDetailDialog({
  link,
  allLinks,
  open,
  onOpenChange,
}: AnimeLinkDetailDialogProps) {
  const { t } = useTranslation()

  const conflictingLinks = allLinks.filter((l) =>
    link.conflicting_link_ids.includes(l.link_id)
  )

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>
            {t("animeLink.detail", "Anime Link Detail")}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 text-sm">
          {/* This link info */}
          <div className="space-y-1">
            <p className="text-xs text-muted-foreground font-sans">
              {t("animeLink.thisLink", "This Link")}
            </p>
            <div className="rounded border p-2 font-mono text-xs space-y-1">
              <div className="flex items-center gap-2">
                <span className="text-muted-foreground">Ep{link.episode_no}</span>
                <span className="font-semibold">{link.group_name}</span>
                {link.conflict_flag && (
                  <Badge variant="destructive" className="text-xs">
                    {t("animeLink.conflict", "Conflict")}
                  </Badge>
                )}
              </div>
              <p className="opacity-70 truncate">{link.title ?? "-"}</p>
              <div className="flex items-center gap-1">
                <span className="truncate opacity-60">{link.url}</span>
                <CopyButton text={link.url} />
              </div>
            </div>
          </div>

          {/* Conflicting links */}
          {conflictingLinks.length > 0 && (
            <div className="space-y-1">
              <p className="text-xs text-muted-foreground font-sans">
                {t("animeLink.conflictsWith", "Conflicts With")}
              </p>
              <div className="rounded border divide-y font-mono text-xs">
                {conflictingLinks.map((cl) => (
                  <div key={cl.link_id} className="p-2 space-y-1">
                    <div className="flex items-center gap-2">
                      <span className="text-muted-foreground">Ep{cl.episode_no}</span>
                      <span className="font-semibold">{cl.group_name}</span>
                    </div>
                    <p className="opacity-70 truncate">{cl.title ?? "-"}</p>
                    <div className="flex items-center gap-1">
                      <span className="truncate opacity-60">{cl.url}</span>
                      <CopyButton text={cl.url} />
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
```

**Step 2: Update `AnimeSeriesDialog` to show conflict badge and open detail dialog**

In `AnimeSeriesDialog.tsx`, make the following changes:

1. Import the new dialog and the `AlertTriangle` icon at the top:
```tsx
import { Save, X, AlertTriangle } from "lucide-react"
import { AnimeLinkDetailDialog } from "@/pages/anime-series/AnimeLinkDetailDialog"
```

2. Add state for the detail dialog. In the `AnimeSeriesDialog` function body, after the existing `groupDialog` state:
```tsx
const [detailLink, setDetailLink] = useState<AnimeLinkRich | null>(null)
```

3. Update the `LinkRow` usage in the links tab to pass `allLinks` and a click handler. Change both `LinkRow` calls to:
```tsx
{passedLinks.map((link) => (
  <LinkRow
    key={link.link_id}
    link={link}
    passed
    onGroupClick={setGroupDialog}
    onConflictClick={setDetailLink}
  />
))}
{filteredLinks.map((link) => (
  <LinkRow
    key={link.link_id}
    link={link}
    passed={false}
    onGroupClick={setGroupDialog}
    onConflictClick={setDetailLink}
  />
))}
```

4. Add the detail dialog render after the `SubtitleGroupDialog` render:
```tsx
{detailLink && (
  <AnimeLinkDetailDialog
    link={detailLink}
    allLinks={links ?? []}
    open={!!detailLink}
    onOpenChange={(open) => {
      if (!open) setDetailLink(null)
    }}
  />
)}
```

5. Update the `LinkRow` component signature and body:

```tsx
// BEFORE signature:
function LinkRow({
  link,
  passed,
  onGroupClick,
}: {
  link: AnimeLinkRich
  passed: boolean
  onGroupClick: (g: { id: number; name: string }) => void
})

// AFTER signature:
function LinkRow({
  link,
  passed,
  onGroupClick,
  onConflictClick,
}: {
  link: AnimeLinkRich
  passed: boolean
  onGroupClick: (g: { id: number; name: string }) => void
  onConflictClick: (link: AnimeLinkRich) => void
})
```

6. In `LinkRow`'s JSX, add a conflict badge before the `CopyButton`. Add after the title span and before `<CopyButton>`:

```tsx
{link.conflict_flag && (
  <button
    type="button"
    title="This link has a conflict"
    className="shrink-0 text-amber-500 hover:text-amber-600"
    onClick={() => onConflictClick(link)}
  >
    <AlertTriangle className="h-3.5 w-3.5" />
  </button>
)}
```

**Step 3: Build frontend**

```bash
cd /workspace/frontend && npm run build 2>&1 | tail -30
```
Expected: builds without errors.

**Step 4: Commit**

```bash
cd /workspace && git add frontend/src/pages/anime-series/AnimeLinkDetailDialog.tsx frontend/src/pages/anime-series/AnimeSeriesDialog.tsx
git commit -m "feat(frontend): show conflict badge on anime links and open detail dialog"
```

---

### Task 8: End-to-end smoke test

**Step 1: Start the services**

```bash
cd /workspace && docker compose up -d 2>&1 | tail -10
```

Wait a few seconds for services to start.

**Step 2: Check core service is healthy**

```bash
curl -s http://localhost:8000/api/core/health | python3 -m json.tool
```
Expected: `{"status": "ok", "service": "core"}`

**Step 3: Verify dispatch endpoint rejects conflicted links (manual check)**

Look at the log for the core service after a fetch event or create a link manually via:
```bash
# Check the dispatch service compiles and the conflict_flag filter is present
grep -n "conflict_flag" /workspace/core-service/src/services/download_dispatch.rs
```
Expected: output shows `conflict_flag.eq(false)`.

**Step 4: Verify the API returns conflict fields**

```bash
# Get a series ID from the system
curl -s http://localhost:8000/api/core/series | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['series'][0]['series_id'] if d['series'] else 'no series')"

# Use the series_id from above (replace 1 with actual value)
curl -s http://localhost:8000/api/core/links/1 | python3 -c "import json,sys; d=json.load(sys.stdin); l=d['links'][0] if d['links'] else {}; print('conflict_flag' in l, 'conflicting_link_ids' in l)"
```
Expected: `True True`

**Step 5: Check frontend renders without errors**

Open `http://localhost:5173` (or wherever Vite serves) in a browser. Navigate to Anime Series page, open a dialog with links, verify no console errors.

**Step 6: Commit if any minor fixes were needed**

```bash
cd /workspace && git status
```
If clean: done. If fixes needed: commit them.

---

## Summary of Changes

| File | Change |
|------|--------|
| `core-service/src/services/download_cancel.rs` | **New** — centralised download cancellation service |
| `core-service/src/services/mod.rs` | Export `DownloadCancelService` |
| `core-service/src/state.rs` | Add `cancel_service: Arc<DownloadCancelService>` |
| `core-service/src/services/download_dispatch.rs` | Add `conflict_flag = false` filter |
| `core-service/src/services/filter_recalc.rs` | Return newly-filtered link IDs |
| `core-service/src/handlers/filters.rs` | Cancel downloads for newly-filtered links |
| `core-service/src/handlers/anime_link_conflicts.rs` | Cancel unchosen + dispatch chosen on resolve |
| `core-service/src/dto.rs` | Add `conflict_flag`, `conflicting_link_ids` to `AnimeLinkRichResponse` |
| `core-service/src/handlers/links.rs` | Populate the new fields |
| `frontend/src/schemas/anime.ts` | Add fields to `AnimeLinkRich` |
| `frontend/src/pages/anime-series/AnimeLinkDetailDialog.tsx` | **New** — detail dialog showing conflicting links |
| `frontend/src/pages/anime-series/AnimeSeriesDialog.tsx` | Add conflict badge + wire detail dialog |
