# Resync Mechanism Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** When Parser CRUD triggers reparse and metadata changes on already-synced downloads, automatically resync those files in the Viewer (rename/move + regenerate NFO).

**Architecture:** Core detects metadata changes during `upsert_anime_link`, collects affected synced downloads, sends `ViewerResyncRequest` to the Viewer's new `/resync` endpoint. Viewer looks up the file's actual current path from its own `sync_tasks` DB, moves/renames if needed, regenerates NFO, and callbacks Core with the new path.

**Tech Stack:** Rust, Axum, Diesel (PostgreSQL), shared crate for DTOs

---

### Task 1: Add ViewerResyncRequest to shared crate

**Files:**
- Modify: `shared/src/models.rs:212-232` (after ViewerSyncRequest)

**Step 1: Add the new DTO**

In `shared/src/models.rs`, add after the `ViewerSyncRequest` struct (line 223):

```rust
/// Core → Viewer: request to resync a previously synced download with updated metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerResyncRequest {
    pub download_id: i32,
    pub series_id: i32,
    pub anime_title: String,
    pub series_no: i32,
    pub episode_no: i32,
    pub subtitle_group: String,
    pub old_target_path: String,
    pub callback_url: String,
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p shared`
Expected: OK

**Step 3: Commit**

```bash
git add shared/src/models.rs
git commit -m "feat(shared): add ViewerResyncRequest DTO"
```

---

### Task 2: Viewer migration — extend sync_tasks table

**Files:**
- Create: `viewers/jellyfin/migrations/2026-02-14-000001-resync-support/up.sql`
- Create: `viewers/jellyfin/migrations/2026-02-14-000001-resync-support/down.sql`

**Step 1: Write the migration**

`up.sql`:
```sql
ALTER TABLE sync_tasks ADD COLUMN anime_title TEXT;
ALTER TABLE sync_tasks ADD COLUMN series_no INT;
ALTER TABLE sync_tasks ADD COLUMN subtitle_group TEXT;
ALTER TABLE sync_tasks ADD COLUMN task_type VARCHAR(10) NOT NULL DEFAULT 'sync';
```

`down.sql`:
```sql
ALTER TABLE sync_tasks DROP COLUMN anime_title;
ALTER TABLE sync_tasks DROP COLUMN series_no;
ALTER TABLE sync_tasks DROP COLUMN subtitle_group;
ALTER TABLE sync_tasks DROP COLUMN task_type;
```

**Step 2: Run migration and regenerate schema**

Run: `cd /workspace/viewers/jellyfin && diesel migration run && diesel print-schema > src/schema.rs`
Expected: Migration applied, schema.rs updated with new columns

**Step 3: Update Diesel models**

In `viewers/jellyfin/src/models.rs`, update `SyncTask` and `NewSyncTask`:

```rust
// ============ SyncTask ============

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::sync_tasks)]
pub struct SyncTask {
    pub task_id: i32,
    pub download_id: i32,
    pub core_series_id: i32,
    pub episode_no: i32,
    pub source_path: String,
    pub target_path: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
    pub anime_title: Option<String>,
    pub series_no: Option<i32>,
    pub subtitle_group: Option<String>,
    pub task_type: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::sync_tasks)]
pub struct NewSyncTask {
    pub download_id: i32,
    pub core_series_id: i32,
    pub episode_no: i32,
    pub source_path: String,
    pub status: String,
    pub anime_title: Option<String>,
    pub series_no: Option<i32>,
    pub subtitle_group: Option<String>,
    pub task_type: String,
}
```

**Step 4: Verify it compiles**

Run: `cargo check -p viewer-jellyfin`
Expected: OK (may need to fix usages of `NewSyncTask` in handlers.rs)

**Step 5: Commit**

```bash
git add viewers/jellyfin/migrations/2026-02-14-000001-resync-support/ viewers/jellyfin/src/schema.rs viewers/jellyfin/src/models.rs
git commit -m "feat(viewer): add resync columns to sync_tasks"
```

---

### Task 3: Update Viewer's existing sync handler to store metadata

**Files:**
- Modify: `viewers/jellyfin/src/handlers.rs:43-58` (sync function, NewSyncTask creation)

**Step 1: Update the NewSyncTask creation in sync()**

Replace the `NewSyncTask` creation block (lines 44-50) with:

```rust
        let new_task = NewSyncTask {
            download_id: req.download_id,
            core_series_id: req.series_id,
            episode_no: req.episode_no,
            source_path: req.file_path.clone(),
            status: "processing".to_string(),
            anime_title: Some(req.anime_title.clone()),
            series_no: Some(req.series_no),
            subtitle_group: Some(req.subtitle_group.clone()),
            task_type: "sync".to_string(),
        };
```

**Step 2: Verify it compiles**

Run: `cargo check -p viewer-jellyfin`
Expected: OK

**Step 3: Commit**

```bash
git add viewers/jellyfin/src/handlers.rs
git commit -m "feat(viewer): store anime metadata in sync_tasks during sync"
```

---

### Task 4: Add FileOrganizer::move_episode for resync

**Files:**
- Modify: `viewers/jellyfin/src/file_organizer.rs`

**Step 1: Add the move_episode method and cleanup_empty_dirs helper**

Add after `organize_episode` (after line 93):

```rust
    /// Move an already-organized episode to a new location based on updated metadata.
    /// Returns the new target path.
    pub async fn move_episode(
        &self,
        current_path: &Path,
        new_anime_title: &str,
        new_season: u32,
        new_episode: u32,
    ) -> anyhow::Result<PathBuf> {
        if !current_path.exists() {
            return Err(anyhow::anyhow!(
                "Current file does not exist: {}",
                current_path.display()
            ));
        }

        let new_target = self
            .library_dir
            .join(Self::sanitize_filename(new_anime_title))
            .join(format!("Season {:02}", new_season));

        fs::create_dir_all(&new_target).await?;

        let extension = current_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mkv");

        let new_filename = format!(
            "{} - S{:02}E{:02}.{}",
            Self::sanitize_filename(new_anime_title),
            new_season,
            new_episode,
            extension
        );

        let new_path = new_target.join(new_filename);

        if new_path == current_path {
            return Ok(new_path);
        }

        if let Err(_) = fs::rename(current_path, &new_path).await {
            fs::copy(current_path, &new_path).await?;
            let _ = fs::remove_file(current_path).await;
        }

        tracing::info!(
            "Resync moved: {} -> {}",
            current_path.display(),
            new_path.display()
        );

        // Clean up empty parent directories
        self.cleanup_empty_dirs(current_path).await;

        Ok(new_path)
    }

    /// Remove empty Season and anime directories after a file is moved out.
    async fn cleanup_empty_dirs(&self, old_file_path: &Path) {
        // Try to remove the old Season directory if empty
        if let Some(season_dir) = old_file_path.parent() {
            if self.is_empty_dir(season_dir).await {
                let _ = fs::remove_dir(season_dir).await;
                tracing::info!("Removed empty directory: {}", season_dir.display());

                // Try to remove the old anime directory if empty
                if let Some(anime_dir) = season_dir.parent() {
                    if anime_dir != self.library_dir && self.is_empty_dir(anime_dir).await {
                        let _ = fs::remove_dir(anime_dir).await;
                        tracing::info!("Removed empty directory: {}", anime_dir.display());
                    }
                }
            }
        }
    }

    async fn is_empty_dir(&self, dir: &Path) -> bool {
        match fs::read_dir(dir).await {
            Ok(mut entries) => entries.next_entry().await.ok().flatten().is_none(),
            Err(_) => false,
        }
    }
```

**Step 2: Verify it compiles**

Run: `cargo check -p viewer-jellyfin`
Expected: OK

**Step 3: Commit**

```bash
git add viewers/jellyfin/src/file_organizer.rs
git commit -m "feat(viewer): add move_episode and cleanup_empty_dirs for resync"
```

---

### Task 5: Add Viewer resync handler

**Files:**
- Modify: `viewers/jellyfin/src/handlers.rs` (add `resync` function)
- Modify: `viewers/jellyfin/src/main.rs` (add route)

**Step 1: Add the resync handler**

Add at the end of `viewers/jellyfin/src/handlers.rs` (before the helper functions `cache_subject`, etc.):

```rust
pub async fn resync(State(state): State<AppState>, Json(req): Json<shared::ViewerResyncRequest>) -> StatusCode {
    tracing::info!(
        "Received resync request: download_id={}, anime={} S{:02}E{:02}",
        req.download_id,
        req.anime_title,
        req.series_no,
        req.episode_no
    );

    // Record resync task
    let task_id = if let Ok(mut conn) = state.db.get() {
        let new_task = NewSyncTask {
            download_id: req.download_id,
            core_series_id: req.series_id,
            episode_no: req.episode_no,
            source_path: req.old_target_path.clone(),
            status: "processing".to_string(),
            anime_title: Some(req.anime_title.clone()),
            series_no: Some(req.series_no),
            subtitle_group: Some(req.subtitle_group.clone()),
            task_type: "resync".to_string(),
        };
        diesel::insert_into(sync_tasks::table)
            .values(&new_task)
            .returning(sync_tasks::task_id)
            .get_result::<i32>(&mut conn)
            .ok()
    } else {
        None
    };

    let organizer = state.organizer.clone();
    let db = state.db.clone();
    let bangumi = state.bangumi.clone();
    tokio::spawn(async move {
        process_resync(organizer, db, bangumi, req, task_id).await;
    });

    StatusCode::ACCEPTED
}

async fn process_resync(
    organizer: Arc<FileOrganizer>,
    db: DbPool,
    bangumi: Arc<BangumiClient>,
    req: shared::ViewerResyncRequest,
    task_id: Option<i32>,
) {
    let result = do_resync(&organizer, &db, &bangumi, &req).await;

    // Update sync_task record
    if let (Some(tid), Ok(mut conn)) = (task_id, db.get()) {
        let now = chrono::Utc::now().naive_utc();
        match &result {
            Ok(target_path) => {
                let _ = diesel::update(sync_tasks::table.filter(sync_tasks::task_id.eq(tid)))
                    .set((
                        sync_tasks::status.eq("completed"),
                        sync_tasks::target_path.eq(target_path),
                        sync_tasks::completed_at.eq(Some(now)),
                    ))
                    .execute(&mut conn);
            }
            Err(e) => {
                let _ = diesel::update(sync_tasks::table.filter(sync_tasks::task_id.eq(tid)))
                    .set((
                        sync_tasks::status.eq("failed"),
                        sync_tasks::error_message.eq(Some(e.to_string())),
                        sync_tasks::completed_at.eq(Some(now)),
                    ))
                    .execute(&mut conn);
            }
        }
    }

    // Callback to Core
    let (status, target_path, error_message) = match result {
        Ok(path) => ("synced".to_string(), Some(path), None),
        Err(e) => ("failed".to_string(), None, Some(e.to_string())),
    };

    let callback = shared::ViewerSyncCallback {
        download_id: req.download_id,
        status,
        target_path,
        error_message,
    };

    let client = reqwest::Client::new();
    if let Err(e) = client
        .post(&req.callback_url)
        .json(&callback)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        tracing::error!(
            "Failed to callback Core for resync download {}: {}",
            req.download_id,
            e
        );
    }
}

async fn do_resync(
    organizer: &FileOrganizer,
    db: &DbPool,
    bangumi: &BangumiClient,
    req: &shared::ViewerResyncRequest,
) -> anyhow::Result<String> {
    // 1. Find the actual current file path from our DB
    let current_path = {
        let mut conn = db.get().map_err(|e| anyhow::anyhow!("{}", e))?;
        let latest_task: Option<SyncTask> = sync_tasks::table
            .filter(sync_tasks::download_id.eq(req.download_id))
            .filter(sync_tasks::status.eq("completed"))
            .order(sync_tasks::completed_at.desc())
            .first::<SyncTask>(&mut conn)
            .optional()
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        match latest_task.and_then(|t| t.target_path) {
            Some(path) => std::path::PathBuf::from(path),
            None => {
                // Fallback to old_target_path from Core
                std::path::PathBuf::from(&req.old_target_path)
            }
        }
    };

    if !current_path.exists() {
        return Err(anyhow::anyhow!(
            "File not found at expected path: {}",
            current_path.display()
        ));
    }

    // 2. Move/rename the file if path-affecting metadata changed
    let new_path = organizer
        .move_episode(
            &current_path,
            &req.anime_title,
            req.series_no as u32,
            req.episode_no as u32,
        )
        .await?;

    // 3. Regenerate NFO metadata (delete old NFO first so it gets recreated)
    //    Delete old episode NFO (next to old file location)
    let old_nfo = current_path.with_extension("nfo");
    if old_nfo.exists() && old_nfo != new_path.with_extension("nfo") {
        let _ = tokio::fs::remove_file(&old_nfo).await;
    }

    // 4. Fetch/update bangumi metadata and regenerate NFOs
    if let Err(e) = fetch_and_generate_metadata(
        db,
        bangumi,
        organizer,
        req.series_id,
        &req.anime_title,
        req.series_no,
        req.episode_no,
        &new_path,
    )
    .await
    {
        tracing::warn!(
            "Metadata fetch failed during resync for download {} (non-fatal): {}",
            req.download_id,
            e
        );
    }

    Ok(new_path.display().to_string())
}
```

**Step 2: Add the route in main.rs**

In `viewers/jellyfin/src/main.rs`, update the router (line 83-86):

```rust
    let app = Router::new()
        .route("/sync", post(handlers::sync))
        .route("/resync", post(handlers::resync))
        .route("/health", get(handlers::health_check))
        .with_state(state);
```

**Step 3: Verify it compiles**

Run: `cargo check -p viewer-jellyfin`
Expected: OK

**Step 4: Commit**

```bash
git add viewers/jellyfin/src/handlers.rs viewers/jellyfin/src/main.rs
git commit -m "feat(viewer): add POST /resync endpoint for metadata-changed files"
```

---

### Task 6: Handle stale tvshow.nfo on anime title change

**Files:**
- Modify: `viewers/jellyfin/src/nfo_generator.rs:6-12` (remove skip-if-exists for tvshow.nfo)
- Modify: `viewers/jellyfin/src/handlers.rs` (do_resync — clean up old anime dir tvshow.nfo)

**Step 1: Make tvshow.nfo generation overwrite existing files**

In `viewers/jellyfin/src/nfo_generator.rs`, change `generate_tvshow_nfo` to accept a `force` parameter. Replace lines 6-12:

```rust
pub async fn generate_tvshow_nfo(anime_dir: &Path, subject: &SubjectDetail, force: bool) -> anyhow::Result<()> {
    let nfo_path = anime_dir.join("tvshow.nfo");

    // Skip if already exists (unless forced, e.g. during resync)
    if nfo_path.exists() && !force {
        return Ok(());
    }
```

**Step 2: Update all callers**

In `viewers/jellyfin/src/handlers.rs`, the call to `generate_tvshow_nfo` in `fetch_and_generate_metadata` (line 245):

```rust
        nfo_generator::generate_tvshow_nfo(&anime_dir, &subject_detail, false).await?;
```

In `do_resync`, the call goes through `fetch_and_generate_metadata` which already passes `force: false`. For resync, we want to force regeneration. Add a `force_nfo` parameter to `fetch_and_generate_metadata`:

Change the signature of `fetch_and_generate_metadata` (line 175):

```rust
async fn fetch_and_generate_metadata(
    db: &DbPool,
    bangumi: &BangumiClient,
    organizer: &FileOrganizer,
    series_id: i32,
    anime_title: &str,
    series_no: i32,
    episode_no: i32,
    target_path: &std::path::Path,
    force_nfo: bool,
) -> anyhow::Result<()> {
```

Update the `generate_tvshow_nfo` call inside (line 245):

```rust
        nfo_generator::generate_tvshow_nfo(&anime_dir, &subject_detail, force_nfo).await?;
```

Update the call in `do_sync` (line 152):

```rust
    if let Err(e) = fetch_and_generate_metadata(
        db, bangumi, organizer, req.series_id, &req.anime_title,
        req.series_no, req.episode_no, &target_path, false,
    )
```

Update the call in `do_resync`:

```rust
    if let Err(e) = fetch_and_generate_metadata(
        db, bangumi, organizer, req.series_id, &req.anime_title,
        req.series_no, req.episode_no, &new_path, true,
    )
```

**Step 3: Verify it compiles**

Run: `cargo check -p viewer-jellyfin`
Expected: OK

**Step 4: Commit**

```bash
git add viewers/jellyfin/src/nfo_generator.rs viewers/jellyfin/src/handlers.rs
git commit -m "feat(viewer): support forced NFO regeneration during resync"
```

---

### Task 7: Core — modify upsert_anime_link to detect metadata changes

**Files:**
- Modify: `core-service/src/handlers/parsers.rs:593-711` (upsert_anime_link function)

**Step 1: Add UpsertResult struct**

Add near the top of the file (after `ReparseStats` around line 53):

```rust
struct UpsertResult {
    link_id: i32,
    is_new: bool,
    metadata_changed: bool,
}
```

**Step 2: Modify upsert_anime_link return type and detect changes**

Change the function signature (line 593):

```rust
fn upsert_anime_link(
    conn: &mut diesel::PgConnection,
    raw_item: &RawAnimeItem,
    parsed: &crate::services::title_parser::ParsedResult,
) -> Result<UpsertResult, String> {
```

In the existing-link branch (line 629), before the update, capture old values:

```rust
    if let Some(link) = existing_link {
        let old_series_id = link.series_id;
        let old_group_id = link.group_id;
        let old_episode_no = link.episode_no;

        // 3a. 更新既有 link（保留 link_id → downloads 不受影響）
        diesel::update(anime_links::table.filter(anime_links::link_id.eq(link.link_id)))
            .set((
                anime_links::series_id.eq(series.series_id),
                anime_links::group_id.eq(group.group_id),
                anime_links::episode_no.eq(parsed.episode_no),
                anime_links::title.eq(Some(&raw_item.title)),
            ))
            .execute(conn)
            .map_err(|e| format!("Failed to update anime link: {}", e))?;

        // ... existing cleanup_empty_series and filter_recalc code ...

        let metadata_changed = old_series_id != series.series_id
            || old_group_id != group.group_id
            || old_episode_no != parsed.episode_no;

        Ok(UpsertResult {
            link_id: link.link_id,
            is_new: false,
            metadata_changed,
        })
```

In the new-link branch (line 667), return:

```rust
        Ok(UpsertResult {
            link_id: created_link.link_id,
            is_new: true,
            metadata_changed: false,
        })
```

**Step 3: Update callers in reparse_affected_items**

In `reparse_affected_items` (around line 486), update to use `UpsertResult`:

```rust
                match upsert_anime_link(&mut conn, item, &parsed) {
                    Ok(result) => {
                        if result.is_new {
                            new_link_ids.push(result.link_id);
                        }
                        if result.metadata_changed {
                            resync_link_ids.push(result.link_id);
                        }
                        // ... existing status update code ...
```

Add `resync_link_ids` declaration alongside `new_link_ids` (after line 481):

```rust
    let mut new_link_ids: Vec<i32> = Vec::new();
    let mut resync_link_ids: Vec<i32> = Vec::new();
```

**Step 4: Verify it compiles**

Run: `cargo check -p core-service`
Expected: OK (might have warnings about unused `resync_link_ids`)

**Step 5: Commit**

```bash
git add core-service/src/handlers/parsers.rs
git commit -m "feat(core): detect metadata changes in upsert_anime_link"
```

---

### Task 8: Core — add notify_viewer_resync to SyncService

**Files:**
- Modify: `core-service/src/services/sync_service.rs`

**Step 1: Add the resync notification method**

Add after `retry_completed_downloads` (after line 186):

```rust
    /// Notify viewer to resync a download whose metadata has changed.
    pub async fn notify_viewer_resync(&self, download: &Download) -> Result<bool, String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        // Find a viewer module
        let viewer = service_modules::table
            .filter(service_modules::is_enabled.eq(true))
            .filter(service_modules::module_type.eq(ModuleTypeEnum::Viewer))
            .order(service_modules::priority.desc())
            .first::<ServiceModule>(&mut conn)
            .optional()
            .map_err(|e| format!("Failed to query viewers: {}", e))?;

        let viewer = match viewer {
            Some(v) => v,
            None => {
                tracing::warn!(
                    "No viewer module available for resync download {}",
                    download.download_id
                );
                return Ok(false);
            }
        };

        let resync_request = self.build_resync_request(&mut conn, download)?;

        let resync_url = format!("{}/resync", viewer.base_url);
        tracing::info!(
            "Notifying viewer {} for resync download {} at {}",
            viewer.name,
            download.download_id,
            resync_url
        );

        let response = self
            .http_client
            .post(&resync_url)
            .json(&resync_request)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Failed to notify viewer for resync: {}", e))?;

        if response.status() == reqwest::StatusCode::ACCEPTED || response.status().is_success() {
            let now = chrono::Utc::now().naive_utc();
            diesel::update(
                downloads::table.filter(downloads::download_id.eq(download.download_id)),
            )
            .set((
                downloads::status.eq("resyncing"),
                downloads::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to update download status: {}", e))?;

            Ok(true)
        } else {
            Err(format!("Viewer returned status: {}", response.status()))
        }
    }

    fn build_resync_request(
        &self,
        conn: &mut diesel::PgConnection,
        download: &Download,
    ) -> Result<shared::ViewerResyncRequest, String> {
        let link: AnimeLink = anime_links::table
            .filter(anime_links::link_id.eq(download.link_id))
            .first::<AnimeLink>(conn)
            .map_err(|e| format!("Failed to find anime link {}: {}", download.link_id, e))?;

        let series: AnimeSeries = anime_series::table
            .filter(anime_series::series_id.eq(link.series_id))
            .first::<AnimeSeries>(conn)
            .map_err(|e| format!("Failed to find series {}: {}", link.series_id, e))?;

        let anime_title: String = animes::table
            .filter(animes::anime_id.eq(series.anime_id))
            .select(animes::title)
            .first::<String>(conn)
            .map_err(|e| format!("Failed to find anime {}: {}", series.anime_id, e))?;

        let subtitle_group: String = subtitle_groups::table
            .filter(subtitle_groups::group_id.eq(link.group_id))
            .select(subtitle_groups::group_name)
            .first::<String>(conn)
            .map_err(|e| format!("Failed to find subtitle group {}: {}", link.group_id, e))?;

        let old_target_path = download
            .file_path
            .clone()
            .ok_or_else(|| "Download has no file_path for resync".to_string())?;

        let callback_url = format!("{}/sync-callback", self.core_service_url);

        Ok(shared::ViewerResyncRequest {
            download_id: download.download_id,
            series_id: link.series_id,
            anime_title,
            series_no: series.series_no,
            episode_no: link.episode_no,
            subtitle_group,
            old_target_path,
            callback_url,
        })
    }
```

**Step 2: Add import for ViewerResyncRequest**

At the top of `sync_service.rs`, the `shared::ViewerResyncRequest` is used with full path so no new import needed.

**Step 3: Verify it compiles**

Run: `cargo check -p core-service`
Expected: OK

**Step 4: Commit**

```bash
git add core-service/src/services/sync_service.rs
git commit -m "feat(core): add notify_viewer_resync to SyncService"
```

---

### Task 9: Core — trigger resync after reparse

**Files:**
- Modify: `core-service/src/handlers/parsers.rs:442-587` (reparse_affected_items)

**Step 1: Update reparse_affected_items signature to accept SyncService**

Change the function signature (line 442):

```rust
async fn reparse_affected_items(
    db: crate::db::DbPool,
    dispatch_service: std::sync::Arc<crate::services::DownloadDispatchService>,
    sync_service: std::sync::Arc<crate::services::SyncService>,
    item_ids: &[i32],
) -> ReparseStats {
```

Also update `reparse_all_items` (line 399):

```rust
async fn reparse_all_items(
    db: crate::db::DbPool,
    dispatch_service: std::sync::Arc<crate::services::DownloadDispatchService>,
    sync_service: std::sync::Arc<crate::services::SyncService>,
) -> ReparseStats {
```

And its call to `reparse_affected_items` (line 429):

```rust
    reparse_affected_items(db, dispatch_service, sync_service, &all_ids).await
```

**Step 2: Add resync triggering logic at the end of reparse_affected_items**

After the dispatch block (after line 579), add:

```rust
    // 觸發 resync（metadata 變更的已 synced downloads）
    if !resync_link_ids.is_empty() {
        let mut conn_for_resync = match db.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("reparse: 無法取得 DB 連線用於 resync: {}", e);
                return ReparseStats { total, parsed: parsed_count, failed: failed_count, no_match: no_match_count };
            }
        };

        // Find synced downloads for these links
        let synced_downloads: Vec<crate::models::Download> = crate::schema::downloads::table
            .filter(crate::schema::downloads::link_id.eq_any(&resync_link_ids))
            .filter(crate::schema::downloads::status.eq("synced"))
            .filter(crate::schema::downloads::file_path.is_not_null())
            .load::<crate::models::Download>(&mut conn_for_resync)
            .unwrap_or_default();

        drop(conn_for_resync);

        if !synced_downloads.is_empty() {
            tracing::info!(
                "reparse: 偵測到 {} 筆已 synced 的 downloads 需要 resync",
                synced_downloads.len()
            );
            for download in &synced_downloads {
                match sync_service.notify_viewer_resync(download).await {
                    Ok(true) => {
                        tracing::info!("reparse: resync 通知已發送 download_id={}", download.download_id);
                    }
                    Ok(false) => {
                        tracing::warn!("reparse: 無 viewer 可用於 resync download_id={}", download.download_id);
                    }
                    Err(e) => {
                        tracing::error!("reparse: resync 失敗 download_id={}: {}", download.download_id, e);
                    }
                }
            }
        }
    }
```

**Step 3: Update all callers of reparse_all_items**

In `create_parser` (line 261):

```rust
    let stats =
        reparse_all_items(state.db.clone(), state.dispatch_service.clone(), state.sync_service.clone()).await;
```

In `update_parser` (line 351):

```rust
    let stats =
        reparse_all_items(state.db.clone(), state.dispatch_service.clone(), state.sync_service.clone()).await;
```

In `delete_parser` (line 382):

```rust
    let stats =
        reparse_all_items(state.db.clone(), state.dispatch_service.clone(), state.sync_service.clone()).await;
```

**Step 4: Verify it compiles**

Run: `cargo check -p core-service`
Expected: OK

**Step 5: Commit**

```bash
git add core-service/src/handlers/parsers.rs
git commit -m "feat(core): trigger resync for synced downloads after reparse metadata changes"
```

---

### Task 10: Update ReparseStats to include resync count

**Files:**
- Modify: `core-service/src/handlers/parsers.rs`

**Step 1: Add resync_triggered field to ReparseStats**

```rust
#[derive(Debug, Serialize, Default)]
pub struct ReparseStats {
    pub total: usize,
    pub parsed: usize,
    pub failed: usize,
    pub no_match: usize,
    pub resync_triggered: usize,
}
```

**Step 2: Count resync_triggered in reparse_affected_items**

After the resync loop, count how many were successful and include in return:

```rust
    let mut resync_triggered = 0;
    // ... (in the resync loop, increment resync_triggered on Ok(true))

    ReparseStats {
        total,
        parsed: parsed_count,
        failed: failed_count,
        no_match: no_match_count,
        resync_triggered,
    }
```

**Step 3: Verify it compiles**

Run: `cargo check -p core-service`
Expected: OK

**Step 4: Commit**

```bash
git add core-service/src/handlers/parsers.rs
git commit -m "feat(core): include resync_triggered count in ReparseStats"
```

---

### Task 11: Verify full workspace compiles

**Step 1: Check all crates**

Run: `cargo check --workspace`
Expected: OK

**Step 2: Run existing tests**

Run: `cargo test --workspace`
Expected: All existing tests pass

**Step 3: Final commit if any fixups needed**

```bash
git add -A
git commit -m "fix: address compilation issues from resync implementation"
```
