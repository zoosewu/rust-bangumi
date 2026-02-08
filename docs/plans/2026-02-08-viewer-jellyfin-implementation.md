# Viewer Jellyfin Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the full Viewer sync pipeline: Core detects download completion → notifies Viewer → Viewer moves files, fetches bangumi.tv metadata, generates Jellyfin NFO → reports back to Core.

**Architecture:** DownloadScheduler detects `completed` status, queries qBittorrent for file paths, then POST `/sync` to Viewer. Viewer ACKs immediately (202), processes async (move file, fetch metadata, generate NFO), and calls back Core's `/sync-callback`. Viewer has its own PostgreSQL database (`viewer_jellyfin`) for bangumi.tv metadata cache.

**Tech Stack:** Rust, Axum, Diesel (PostgreSQL), reqwest, tokio, serde, bangumi.tv API

**Design doc:** `docs/plans/2026-02-06-viewer-jellyfin-design.md`

---

### Task 1: Extend shared types for Viewer sync

**Files:**
- Modify: `shared/src/models.rs`

**Step 1: Update SyncRequest and add SyncCallback types**

Replace the existing `SyncRequest` and `SyncResponse` in `shared/src/models.rs` (lines 192-208) with:

```rust
// ============ Viewer/Sync ============

/// Core → Viewer: request to sync a completed download
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerSyncRequest {
    pub download_id: i32,
    pub series_id: i32,
    pub anime_title: String,
    pub series_no: i32,
    pub episode_no: i32,
    pub subtitle_group: String,
    pub file_path: String,
    pub callback_url: String,
}

/// Viewer → Core: callback after sync processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerSyncCallback {
    pub download_id: i32,
    pub status: String, // "synced" | "failed"
    pub target_path: Option<String>,
    pub error_message: Option<String>,
}
```

Keep the old `SyncRequest` / `SyncResponse` with a `#[deprecated]` attribute temporarily — remove in Task 15 when the viewer handler is revamped.

**Step 2: Add content_path to DownloadStatusItem**

In `shared/src/models.rs` (line 350-355), add `content_path`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStatusItem {
    pub hash: String,
    pub status: String,
    pub progress: f64,
    pub size: u64,
    pub content_path: Option<String>, // NEW: file path on disk
}
```

**Step 3: Run test to verify compilation**

Run: `cargo check -p shared`
Expected: PASS (no errors)

**Step 4: Commit**

```bash
git add shared/src/models.rs
git commit -m "feat(shared): add ViewerSyncRequest, ViewerSyncCallback, content_path to DownloadStatusItem"
```

---

### Task 2: qBittorrent — expose content_path in status query

**Files:**
- Modify: `downloaders/qbittorrent/src/qbittorrent_client.rs`

**Step 1: Add content_path to TorrentInfo**

In `qbittorrent_client.rs` (line 16-24), add the field:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TorrentInfo {
    pub hash: String,
    pub name: String,
    pub state: String,
    pub progress: f64,
    pub dlspeed: i64,
    pub size: i64,
    pub downloaded: i64,
    pub content_path: Option<String>, // NEW: full path to downloaded content
}
```

Note: qBittorrent API `/api/v2/torrents/info` already returns `content_path` — Serde will automatically deserialize it.

**Step 2: Populate content_path in query_status()**

In `query_status()` (line 304-312), update the mapping:

```rust
let mut results: Vec<DownloadStatusItem> = torrents
    .iter()
    .map(|t| DownloadStatusItem {
        hash: t.hash.clone(),
        status: Self::map_torrent_state(&t.state),
        progress: t.progress,
        size: t.size as u64,
        content_path: t.content_path.clone(),
    })
    .collect();
```

Also update the "not_found" fallback (line 315-323):

```rust
for hash in &hashes {
    if !returned_hashes.contains(hash) {
        results.push(DownloadStatusItem {
            hash: hash.clone(),
            status: "not_found".to_string(),
            progress: 0.0,
            size: 0,
            content_path: None,
        });
    }
}
```

**Step 3: Run tests**

Run: `cargo test -p downloader-qbittorrent`
Expected: PASS

**Step 4: Commit**

```bash
git add downloaders/qbittorrent/src/qbittorrent_client.rs
git commit -m "feat(downloader): expose content_path in torrent status query"
```

---

### Task 3: Core — migration for sync tracking fields

**Files:**
- Create: `core-service/migrations/2026-02-08-000001-viewer-sync-tracking/up.sql`
- Create: `core-service/migrations/2026-02-08-000001-viewer-sync-tracking/down.sql`

**Step 1: Write up.sql**

```sql
-- Add file_path and sync_retry_count to downloads
ALTER TABLE downloads ADD COLUMN file_path TEXT;
ALTER TABLE downloads ADD COLUMN sync_retry_count INT NOT NULL DEFAULT 0;

-- Expand status constraint to include sync statuses
ALTER TABLE downloads DROP CONSTRAINT IF EXISTS downloads_status_check;
ALTER TABLE downloads ADD CONSTRAINT downloads_status_check
    CHECK (status IN ('pending', 'downloading', 'completed', 'failed', 'cancelled', 'downloader_error', 'no_downloader', 'syncing', 'synced', 'sync_failed'));
```

**Step 2: Write down.sql**

```sql
ALTER TABLE downloads DROP COLUMN IF EXISTS file_path;
ALTER TABLE downloads DROP COLUMN IF EXISTS sync_retry_count;

ALTER TABLE downloads DROP CONSTRAINT IF EXISTS downloads_status_check;
ALTER TABLE downloads ADD CONSTRAINT downloads_status_check
    CHECK (status IN ('pending', 'downloading', 'completed', 'failed', 'cancelled', 'downloader_error', 'no_downloader'));
```

**Step 3: Run migration**

Run: `cd /workspace/core-service && diesel migration run`
Expected: Migration applied successfully

**Step 4: Update schema.rs**

Run: `cd /workspace/core-service && diesel print-schema > src/schema.rs`

Verify `downloads` table now includes `file_path` and `sync_retry_count`.

**Step 5: Commit**

```bash
git add core-service/migrations/2026-02-08-000001-viewer-sync-tracking/
git add core-service/src/schema.rs
git commit -m "feat(core): add file_path, sync_retry_count to downloads, expand status for sync"
```

---

### Task 4: Core — update Download model

**Files:**
- Modify: `core-service/src/models/db.rs`

**Step 1: Add new fields to Download struct**

In `db.rs` (lines 290-305), add:

```rust
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::downloads)]
pub struct Download {
    pub download_id: i32,
    pub link_id: i32,
    pub downloader_type: String,
    pub status: String,
    pub progress: Option<f32>,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub module_id: Option<i32>,
    pub torrent_hash: Option<String>,
    pub file_path: Option<String>,         // NEW
    pub sync_retry_count: i32,             // NEW
}
```

**Step 2: Verify compilation**

Run: `cargo check -p core-service 2>&1 | grep -c "^error"` (expect pre-existing Jsonb errors only, no new errors related to Download)

**Step 3: Commit**

```bash
git add core-service/src/models/db.rs
git commit -m "feat(core): add file_path, sync_retry_count to Download model"
```

---

### Task 5: Core — create SyncService

**Files:**
- Create: `core-service/src/services/sync_service.rs`
- Modify: `core-service/src/services/mod.rs`

**Step 1: Write SyncService**

Create `core-service/src/services/sync_service.rs`:

```rust
use crate::db::DbPool;
use crate::models::{
    AnimeLink, AnimeSeries, Download, ModuleTypeEnum, ServiceModule, SubtitleGroup,
};
use crate::schema::{
    anime_links, anime_series, animes, downloads, service_modules, subtitle_groups,
};
use diesel::prelude::*;
use shared::ViewerSyncRequest;

pub struct SyncService {
    db_pool: DbPool,
    http_client: reqwest::Client,
    core_service_url: String,
}

impl SyncService {
    pub fn new(db_pool: DbPool) -> Self {
        let core_service_url = std::env::var("CORE_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());
        Self {
            db_pool,
            http_client: reqwest::Client::new(),
            core_service_url,
        }
    }

    /// Notify viewer of a completed download. Returns Ok(true) if notification sent.
    pub async fn notify_viewer(&self, download: &Download) -> Result<bool, String> {
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
                tracing::warn!("No viewer module available for download {}", download.download_id);
                return Ok(false);
            }
        };

        // Build the sync request by joining anime metadata
        let sync_request = self.build_sync_request(&mut conn, download)?;

        let sync_url = format!("{}/sync", viewer.base_url);
        tracing::info!(
            "Notifying viewer {} for download {} at {}",
            viewer.name,
            download.download_id,
            sync_url
        );

        // Send the notification
        let response = self
            .http_client
            .post(&sync_url)
            .json(&sync_request)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Failed to notify viewer: {}", e))?;

        if response.status() == reqwest::StatusCode::ACCEPTED
            || response.status().is_success()
        {
            // Update status to syncing
            let now = chrono::Utc::now().naive_utc();
            diesel::update(
                downloads::table.filter(downloads::download_id.eq(download.download_id)),
            )
            .set((
                downloads::status.eq("syncing"),
                downloads::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to update download status: {}", e))?;

            Ok(true)
        } else {
            Err(format!("Viewer returned status: {}", response.status()))
        }
    }

    fn build_sync_request(
        &self,
        conn: &mut PgConnection,
        download: &Download,
    ) -> Result<ViewerSyncRequest, String> {
        // Get anime_link
        let link: AnimeLink = anime_links::table
            .filter(anime_links::link_id.eq(download.link_id))
            .first::<AnimeLink>(conn)
            .map_err(|e| format!("Failed to find anime link {}: {}", download.link_id, e))?;

        // Get anime_series
        let series: AnimeSeries = anime_series::table
            .filter(anime_series::series_id.eq(link.series_id))
            .first::<AnimeSeries>(conn)
            .map_err(|e| format!("Failed to find series {}: {}", link.series_id, e))?;

        // Get anime title
        let anime_title: String = animes::table
            .filter(animes::anime_id.eq(series.anime_id))
            .select(animes::title)
            .first::<String>(conn)
            .map_err(|e| format!("Failed to find anime {}: {}", series.anime_id, e))?;

        // Get subtitle group name
        let subtitle_group: String = subtitle_groups::table
            .filter(subtitle_groups::group_id.eq(link.group_id))
            .select(subtitle_groups::group_name)
            .first::<String>(conn)
            .map_err(|e| format!("Failed to find subtitle group {}: {}", link.group_id, e))?;

        let file_path = download
            .file_path
            .clone()
            .ok_or_else(|| "Download has no file_path".to_string())?;

        let callback_url = format!("{}/sync-callback", self.core_service_url);

        Ok(ViewerSyncRequest {
            download_id: download.download_id,
            series_id: link.series_id,
            anime_title,
            series_no: series.series_no,
            episode_no: link.episode_no,
            subtitle_group,
            file_path,
            callback_url,
        })
    }

    /// Handle sync callback from viewer
    pub fn handle_callback(
        &self,
        conn: &mut PgConnection,
        download_id: i32,
        status: &str,
        target_path: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().naive_utc();

        match status {
            "synced" => {
                diesel::update(
                    downloads::table.filter(downloads::download_id.eq(download_id)),
                )
                .set((
                    downloads::status.eq("synced"),
                    downloads::file_path.eq(target_path),
                    downloads::updated_at.eq(now),
                ))
                .execute(conn)
                .map_err(|e| format!("Failed to update download: {}", e))?;

                tracing::info!("Download {} synced to {}", download_id, target_path.unwrap_or("unknown"));
            }
            "failed" => {
                // Check retry count
                let download: Download = downloads::table
                    .filter(downloads::download_id.eq(download_id))
                    .first::<Download>(conn)
                    .map_err(|e| format!("Download not found: {}", e))?;

                let new_retry_count = download.sync_retry_count + 1;
                let new_status = if new_retry_count >= 3 {
                    "sync_failed"
                } else {
                    "completed" // back to completed so scheduler will re-trigger
                };

                diesel::update(
                    downloads::table.filter(downloads::download_id.eq(download_id)),
                )
                .set((
                    downloads::status.eq(new_status),
                    downloads::sync_retry_count.eq(new_retry_count),
                    downloads::error_message.eq(error_message),
                    downloads::updated_at.eq(now),
                ))
                .execute(conn)
                .map_err(|e| format!("Failed to update download: {}", e))?;

                tracing::warn!(
                    "Download {} sync failed (attempt {}/3): {}",
                    download_id,
                    new_retry_count,
                    error_message.unwrap_or("unknown")
                );
            }
            _ => {
                return Err(format!("Unknown callback status: {}", status));
            }
        }

        Ok(())
    }
}
```

**Step 2: Register in mod.rs**

In `core-service/src/services/mod.rs`, add:

```rust
mod sync_service;
pub use sync_service::SyncService;
```

**Step 3: Verify compilation**

Run: `cargo check -p core-service 2>&1 | grep "sync_service"` — should have no errors related to sync_service (ignore pre-existing Jsonb errors).

**Step 4: Commit**

```bash
git add core-service/src/services/sync_service.rs core-service/src/services/mod.rs
git commit -m "feat(core): add SyncService for viewer notification and callback handling"
```

---

### Task 6: Core — add /sync-callback endpoint

**Files:**
- Create: `core-service/src/handlers/sync.rs`
- Modify: `core-service/src/handlers/mod.rs`
- Modify: `core-service/src/main.rs`

**Step 1: Create sync handler**

Create `core-service/src/handlers/sync.rs`:

```rust
use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use shared::ViewerSyncCallback;

use crate::state::AppState;

pub async fn sync_callback(
    State(state): State<AppState>,
    Json(payload): Json<ViewerSyncCallback>,
) -> (StatusCode, Json<serde_json::Value>) {
    tracing::info!(
        "Received sync callback for download {}: status={}",
        payload.download_id,
        payload.status
    );

    match state.db.get() {
        Ok(mut conn) => {
            match state.sync_service.handle_callback(
                &mut conn,
                payload.download_id,
                &payload.status,
                payload.target_path.as_deref(),
                payload.error_message.as_deref(),
            ) {
                Ok(()) => (
                    StatusCode::OK,
                    Json(json!({ "status": "ok" })),
                ),
                Err(e) => {
                    tracing::error!("Failed to handle sync callback: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": e })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database connection error: {}", e) })),
            )
        }
    }
}
```

**Step 2: Register handler in mod.rs**

In `core-service/src/handlers/mod.rs`, add:

```rust
pub mod sync;
```

**Step 3: Add SyncService to AppState**

In `core-service/src/state.rs`, add `sync_service` field:

```rust
use crate::services::{DownloadDispatchService, ServiceRegistry, SyncService};

pub struct AppState {
    pub db: DbPool,
    pub registry: Arc<ServiceRegistry>,
    pub repos: Arc<Repositories>,
    pub dispatch_service: Arc<DownloadDispatchService>,
    pub sync_service: Arc<SyncService>,           // NEW
}

impl AppState {
    pub fn new(db: DbPool, registry: ServiceRegistry) -> Self {
        let repos = Repositories::new(db.clone());
        let dispatch_service = DownloadDispatchService::new(db.clone());
        let sync_service = SyncService::new(db.clone());   // NEW
        Self {
            db,
            registry: Arc::new(registry),
            repos: Arc::new(repos),
            dispatch_service: Arc::new(dispatch_service),
            sync_service: Arc::new(sync_service),           // NEW
        }
    }
}
```

**Step 4: Add route in main.rs**

In `core-service/src/main.rs`, add after the conflicts routes (around line 192):

```rust
        // Viewer 同步回呼
        .route("/sync-callback", post(handlers::sync::sync_callback))
```

**Step 5: Verify compilation**

Run: `cargo check -p core-service 2>&1 | grep sync`

**Step 6: Commit**

```bash
git add core-service/src/handlers/sync.rs core-service/src/handlers/mod.rs
git add core-service/src/state.rs core-service/src/main.rs
git commit -m "feat(core): add /sync-callback endpoint and SyncService to AppState"
```

---

### Task 7: Core — extend DownloadScheduler to trigger sync

**Files:**
- Modify: `core-service/src/services/download_scheduler.rs`

**Step 1: Add SyncService reference to DownloadScheduler**

Update the struct and constructor:

```rust
pub struct DownloadScheduler {
    db_pool: DbPool,
    poll_interval_secs: u64,
    http_client: reqwest::Client,
    sync_service: Arc<super::SyncService>,    // NEW
}

impl DownloadScheduler {
    pub fn new(db_pool: DbPool, sync_service: Arc<super::SyncService>) -> Self {
        let poll_interval_secs = std::env::var("DOWNLOAD_POLL_INTERVAL")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60);
        Self {
            db_pool,
            poll_interval_secs,
            http_client: reqwest::Client::new(),
            sync_service,
        }
    }
```

**Step 2: Store content_path and trigger sync on completion**

In `poll_downloader()` (lines 107-130), update the completion handling:

```rust
for status_item in &response.statuses {
    let new_status = match status_item.status.as_str() {
        "completed" => "completed",
        "error" => "failed",
        "downloading" | "stalledDL" | "metaDL" | "queuedDL" | "checkingDL"
        | "forcedDL" | "allocating" | "moving" => "downloading",
        _ => continue,
    };

    let now = chrono::Utc::now().naive_utc();

    // Store content_path when status is completed
    let mut update = diesel::update(
        downloads::table
            .filter(downloads::torrent_hash.eq(&status_item.hash))
            .filter(downloads::module_id.eq(downloader.module_id)),
    );

    if new_status == "completed" {
        update
            .set((
                downloads::status.eq(new_status),
                downloads::progress.eq(status_item.progress as f32),
                downloads::total_bytes.eq(status_item.size as i64),
                downloads::file_path.eq(&status_item.content_path),
                downloads::updated_at.eq(now),
            ))
            .execute(conn)
            .ok();
    } else {
        update
            .set((
                downloads::status.eq(new_status),
                downloads::progress.eq(status_item.progress as f32),
                downloads::total_bytes.eq(status_item.size as i64),
                downloads::updated_at.eq(now),
            ))
            .execute(conn)
            .ok();
    }
}

// After processing all statuses, trigger sync for newly completed downloads
self.trigger_sync_for_completed(conn).await;
```

**Step 3: Add trigger_sync_for_completed method**

Add this method to `DownloadScheduler`:

```rust
async fn trigger_sync_for_completed(&self, conn: &mut PgConnection) {
    // Find downloads that just became "completed" and have file_path set
    let completed: Vec<Download> = match downloads::table
        .filter(downloads::status.eq("completed"))
        .filter(downloads::file_path.is_not_null())
        .filter(downloads::sync_retry_count.lt(3))
        .load::<Download>(conn)
    {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to query completed downloads: {}", e);
            return;
        }
    };

    for download in completed {
        match self.sync_service.notify_viewer(&download).await {
            Ok(true) => {
                tracing::info!("Triggered sync for download {}", download.download_id);
            }
            Ok(false) => {
                // No viewer available, skip
            }
            Err(e) => {
                tracing::error!(
                    "Failed to trigger sync for download {}: {}",
                    download.download_id,
                    e
                );
            }
        }
    }
}
```

**Step 4: Update DownloadScheduler creation in main.rs**

In `core-service/src/main.rs` (lines 66-72), pass sync_service:

```rust
let download_scheduler = std::sync::Arc::new(services::DownloadScheduler::new(
    app_state.db.clone(),
    app_state.sync_service.clone(),
));
```

**Step 5: Verify compilation**

Run: `cargo check -p core-service 2>&1 | grep download_scheduler`

**Step 6: Commit**

```bash
git add core-service/src/services/download_scheduler.rs core-service/src/main.rs
git commit -m "feat(core): DownloadScheduler triggers viewer sync on download completion"
```

---

### Task 8: Viewer — set up viewer_jellyfin database

**Files:**
- Create: `viewers/jellyfin/diesel.toml`
- Create: `viewers/jellyfin/migrations/00000000000000_diesel_initial_setup/up.sql`
- Create: `viewers/jellyfin/migrations/00000000000000_diesel_initial_setup/down.sql`
- Create: `viewers/jellyfin/migrations/2026-02-08-000001-viewer-schema/up.sql`
- Create: `viewers/jellyfin/migrations/2026-02-08-000001-viewer-schema/down.sql`
- Modify: `viewers/jellyfin/Cargo.toml`

**Step 1: Add diesel dependencies to Cargo.toml**

Append to `[dependencies]` in `viewers/jellyfin/Cargo.toml`:

```toml
diesel = { version = "2.2", features = ["postgres", "r2d2", "chrono"] }
diesel_migrations = "2.2"
```

**Step 2: Create diesel.toml**

Create `viewers/jellyfin/diesel.toml`:

```toml
[print_schema]
file = "src/schema.rs"

[migrations_directory]
dir = "migrations"
```

**Step 3: Create the database**

Run:
```bash
PGPASSWORD=bangumi_dev_password psql -h 172.20.0.4 -U bangumi -d postgres -c "CREATE DATABASE viewer_jellyfin OWNER bangumi;"
```

**Step 4: Create initial diesel setup migration**

Create `viewers/jellyfin/migrations/00000000000000_diesel_initial_setup/up.sql`:

```sql
SELECT 1;
```

Create `viewers/jellyfin/migrations/00000000000000_diesel_initial_setup/down.sql`:

```sql
SELECT 1;
```

**Step 5: Create viewer schema migration**

Create `viewers/jellyfin/migrations/2026-02-08-000001-viewer-schema/up.sql`:

```sql
-- bangumi.tv metadata cache
CREATE TABLE bangumi_subjects (
    bangumi_id      INT PRIMARY KEY,
    title           TEXT NOT NULL,
    title_cn        TEXT,
    summary         TEXT,
    rating          REAL,
    cover_url       TEXT,
    air_date        DATE,
    episode_count   INT,
    raw_json        JSONB,
    fetched_at      TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Single episode metadata cache
CREATE TABLE bangumi_episodes (
    bangumi_ep_id   INT PRIMARY KEY,
    bangumi_id      INT NOT NULL REFERENCES bangumi_subjects(bangumi_id) ON DELETE CASCADE,
    episode_no      INT NOT NULL,
    title           TEXT,
    title_cn        TEXT,
    air_date        DATE,
    summary         TEXT,
    fetched_at      TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Core series_id → bangumi.tv subject_id mapping
CREATE TABLE bangumi_mapping (
    core_series_id  INT PRIMARY KEY,
    bangumi_id      INT NOT NULL REFERENCES bangumi_subjects(bangumi_id),
    title_cache     TEXT,
    source          VARCHAR(20) NOT NULL DEFAULT 'auto_search',
    created_at      TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Sync task history
CREATE TABLE sync_tasks (
    task_id         SERIAL PRIMARY KEY,
    download_id     INT NOT NULL,
    core_series_id  INT NOT NULL,
    episode_no      INT NOT NULL,
    source_path     TEXT NOT NULL,
    target_path     TEXT,
    status          VARCHAR(20) NOT NULL DEFAULT 'pending',
    error_message   TEXT,
    created_at      TIMESTAMP NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMP
);

CREATE INDEX idx_sync_tasks_status ON sync_tasks(status);
CREATE INDEX idx_sync_tasks_download_id ON sync_tasks(download_id);
```

Create `viewers/jellyfin/migrations/2026-02-08-000001-viewer-schema/down.sql`:

```sql
DROP TABLE IF EXISTS sync_tasks;
DROP TABLE IF EXISTS bangumi_mapping;
DROP TABLE IF EXISTS bangumi_episodes;
DROP TABLE IF EXISTS bangumi_subjects;
```

**Step 6: Run migration**

Run:
```bash
cd /workspace/viewers/jellyfin && DATABASE_URL=postgresql://bangumi:bangumi_dev_password@172.20.0.4:5432/viewer_jellyfin diesel migration run
```

**Step 7: Generate schema.rs**

Run:
```bash
cd /workspace/viewers/jellyfin && DATABASE_URL=postgresql://bangumi:bangumi_dev_password@172.20.0.4:5432/viewer_jellyfin diesel print-schema > src/schema.rs
```

**Step 8: Commit**

```bash
git add viewers/jellyfin/diesel.toml viewers/jellyfin/Cargo.toml
git add viewers/jellyfin/migrations/ viewers/jellyfin/src/schema.rs
git commit -m "feat(viewer): set up viewer_jellyfin database with diesel migrations"
```

---

### Task 9: Viewer — create database models and connection pool

**Files:**
- Create: `viewers/jellyfin/src/db.rs`
- Create: `viewers/jellyfin/src/models.rs`

**Step 1: Create db.rs**

Create `viewers/jellyfin/src/db.rs`:

```rust
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub fn create_pool(database_url: &str) -> DbPool {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .max_size(5)
        .build(manager)
        .expect("Failed to create database pool")
}
```

**Step 2: Create models.rs**

Create `viewers/jellyfin/src/models.rs`:

```rust
use chrono::{NaiveDate, NaiveDateTime};
use diesel::prelude::*;

// ============ BangumiSubject ============

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::bangumi_subjects)]
pub struct BangumiSubject {
    pub bangumi_id: i32,
    pub title: String,
    pub title_cn: Option<String>,
    pub summary: Option<String>,
    pub rating: Option<f32>,
    pub cover_url: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub episode_count: Option<i32>,
    pub raw_json: Option<serde_json::Value>,
    pub fetched_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::bangumi_subjects)]
pub struct NewBangumiSubject {
    pub bangumi_id: i32,
    pub title: String,
    pub title_cn: Option<String>,
    pub summary: Option<String>,
    pub rating: Option<f32>,
    pub cover_url: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub episode_count: Option<i32>,
    pub raw_json: Option<serde_json::Value>,
}

// ============ BangumiEpisode ============

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::bangumi_episodes)]
pub struct BangumiEpisode {
    pub bangumi_ep_id: i32,
    pub bangumi_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub summary: Option<String>,
    pub fetched_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::bangumi_episodes)]
pub struct NewBangumiEpisode {
    pub bangumi_ep_id: i32,
    pub bangumi_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub summary: Option<String>,
}

// ============ BangumiMapping ============

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::bangumi_mapping)]
pub struct BangumiMapping {
    pub core_series_id: i32,
    pub bangumi_id: i32,
    pub title_cache: Option<String>,
    pub source: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::bangumi_mapping)]
pub struct NewBangumiMapping {
    pub core_series_id: i32,
    pub bangumi_id: i32,
    pub title_cache: Option<String>,
    pub source: String,
}

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
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::sync_tasks)]
pub struct NewSyncTask {
    pub download_id: i32,
    pub core_series_id: i32,
    pub episode_no: i32,
    pub source_path: String,
    pub status: String,
}
```

**Step 3: Verify compilation**

Run: `cargo check -p viewer-jellyfin`
Expected: PASS

**Step 4: Commit**

```bash
git add viewers/jellyfin/src/db.rs viewers/jellyfin/src/models.rs
git commit -m "feat(viewer): add database models and connection pool"
```

---

### Task 10: Viewer — bangumi.tv API client

**Files:**
- Create: `viewers/jellyfin/src/bangumi_client.rs`

**Step 1: Create bangumi_client.rs**

```rust
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::time::Duration;

const BANGUMI_API_BASE: &str = "https://api.bgm.tv";
const USER_AGENT: &str = "bangumi-viewer/1.0";

pub struct BangumiClient {
    http_client: reqwest::Client,
}

// ============ API Response Types ============

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub results: i32,
    pub list: Option<Vec<SearchItem>>,
}

#[derive(Debug, Deserialize)]
pub struct SearchItem {
    pub id: i32,
    pub name: String,
    pub name_cn: Option<String>,
    pub air_date: Option<String>,
    pub images: Option<SearchImages>,
}

#[derive(Debug, Deserialize)]
pub struct SearchImages {
    pub large: Option<String>,
    pub common: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectDetail {
    pub id: i32,
    pub name: String,
    pub name_cn: Option<String>,
    pub summary: Option<String>,
    pub date: Option<String>,
    pub images: Option<SubjectImages>,
    pub rating: Option<SubjectRating>,
    pub total_episodes: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectImages {
    pub large: Option<String>,
    pub common: Option<String>,
    pub medium: Option<String>,
    pub small: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectRating {
    pub score: Option<f32>,
    pub total: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct EpisodesResponse {
    pub data: Vec<EpisodeItem>,
    pub total: i32,
}

#[derive(Debug, Deserialize)]
pub struct EpisodeItem {
    pub id: i32,
    pub ep: Option<i32>,
    pub sort: i32,
    pub name: Option<String>,
    pub name_cn: Option<String>,
    pub airdate: Option<String>,
    pub desc: Option<String>,
}

// ============ Client Implementation ============

impl BangumiClient {
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to create HTTP client");
        Self { http_client }
    }

    /// Search for an anime by title. Returns the first match's bangumi_id.
    pub async fn search_anime(&self, title: &str) -> Result<Option<i32>> {
        let url = format!(
            "{}/search/subject/{}?type=2&responseGroup=small",
            BANGUMI_API_BASE,
            urlencoding::encode(title)
        );

        let resp = self.http_client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!("bangumi.tv search returned {}", resp.status()));
        }

        let result: SearchResult = resp.json().await?;

        if result.results > 0 {
            if let Some(list) = &result.list {
                if let Some(first) = list.first() {
                    return Ok(Some(first.id));
                }
            }
        }

        Ok(None)
    }

    /// Get detailed subject info by bangumi_id.
    pub async fn get_subject(&self, bangumi_id: i32) -> Result<SubjectDetail> {
        let url = format!("{}/v0/subjects/{}", BANGUMI_API_BASE, bangumi_id);
        let resp = self.http_client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "bangumi.tv subject {} returned {}",
                bangumi_id,
                resp.status()
            ));
        }

        Ok(resp.json().await?)
    }

    /// Get episode list for a subject.
    pub async fn get_episodes(&self, bangumi_id: i32) -> Result<Vec<EpisodeItem>> {
        let mut all_episodes = Vec::new();
        let mut offset = 0;
        let limit = 100;

        loop {
            let url = format!(
                "{}/v0/episodes?subject_id={}&type=0&limit={}&offset={}",
                BANGUMI_API_BASE, bangumi_id, limit, offset
            );

            let resp = self.http_client.get(&url).send().await?;

            if !resp.status().is_success() {
                return Err(anyhow!(
                    "bangumi.tv episodes for {} returned {}",
                    bangumi_id,
                    resp.status()
                ));
            }

            let result: EpisodesResponse = resp.json().await?;
            let count = result.data.len();
            all_episodes.extend(result.data);

            if all_episodes.len() >= result.total as usize || count == 0 {
                break;
            }
            offset += limit;

            // Rate limiting
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Ok(all_episodes)
    }

    /// Download an image from URL to a local file path.
    pub async fn download_image(&self, url: &str, target_path: &std::path::Path) -> Result<()> {
        let resp = self.http_client.get(url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to download image: {}", resp.status()));
        }

        let bytes = resp.bytes().await?;
        tokio::fs::write(target_path, &bytes).await?;
        Ok(())
    }
}
```

**Step 2: Add urlencoding dependency to Cargo.toml**

Append to `[dependencies]` in `viewers/jellyfin/Cargo.toml`:

```toml
urlencoding = "2.1"
```

**Step 3: Verify compilation**

Run: `cargo check -p viewer-jellyfin`
Expected: PASS

**Step 4: Commit**

```bash
git add viewers/jellyfin/src/bangumi_client.rs viewers/jellyfin/Cargo.toml
git commit -m "feat(viewer): add bangumi.tv API client"
```

---

### Task 11: Viewer — NFO generator

**Files:**
- Create: `viewers/jellyfin/src/nfo_generator.rs`

**Step 1: Create nfo_generator.rs**

```rust
use crate::bangumi_client::{EpisodeItem, SubjectDetail};
use std::path::Path;
use tokio::fs;

/// Generate tvshow.nfo in the anime root directory
pub async fn generate_tvshow_nfo(
    anime_dir: &Path,
    subject: &SubjectDetail,
) -> anyhow::Result<()> {
    let nfo_path = anime_dir.join("tvshow.nfo");

    // Skip if already exists
    if nfo_path.exists() {
        return Ok(());
    }

    let title = xml_escape(&subject.name);
    let title_cn = subject
        .name_cn
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();
    let plot = subject
        .summary
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();
    let rating = subject
        .rating
        .as_ref()
        .and_then(|r| r.score)
        .map(|s| format!("{:.1}", s))
        .unwrap_or_default();
    let year = subject
        .date
        .as_deref()
        .and_then(|d| d.split('-').next())
        .unwrap_or("");

    let nfo_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<tvshow>
    <title>{title_cn}</title>
    <originaltitle>{title}</originaltitle>
    <plot>{plot}</plot>
    <rating>{rating}</rating>
    <year>{year}</year>
    <uniqueid type="bangumi">{bangumi_id}</uniqueid>
</tvshow>
"#,
        title_cn = if title_cn.is_empty() { &title } else { &title_cn },
        title = title,
        plot = plot,
        rating = rating,
        year = year,
        bangumi_id = subject.id,
    );

    fs::write(&nfo_path, nfo_content).await?;
    tracing::info!("Generated tvshow.nfo at {}", nfo_path.display());
    Ok(())
}

/// Generate episode NFO file next to the video file
pub async fn generate_episode_nfo(
    video_path: &Path,
    episode: &EpisodeItem,
    season: i32,
) -> anyhow::Result<()> {
    let nfo_path = video_path.with_extension("nfo");

    let title = episode
        .name
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();
    let title_cn = episode
        .name_cn
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();
    let aired = episode.airdate.as_deref().unwrap_or("");
    let plot = episode
        .desc
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();
    let ep_no = episode.ep.unwrap_or(episode.sort);

    let display_title = if !title_cn.is_empty() {
        &title_cn
    } else if !title.is_empty() {
        &title
    } else {
        ""
    };

    let nfo_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<episodedetails>
    <title>{title}</title>
    <season>{season}</season>
    <episode>{episode}</episode>
    <aired>{aired}</aired>
    <plot>{plot}</plot>
</episodedetails>
"#,
        title = display_title,
        season = season,
        episode = ep_no,
        aired = aired,
        plot = plot,
    );

    fs::write(&nfo_path, nfo_content).await?;
    tracing::info!("Generated episode NFO at {}", nfo_path.display());
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("foo & bar"), "foo &amp; bar");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape(r#"a"b'c"#), "a&quot;b&apos;c");
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check -p viewer-jellyfin`
Expected: PASS

**Step 3: Commit**

```bash
git add viewers/jellyfin/src/nfo_generator.rs
git commit -m "feat(viewer): add NFO generator for tvshow and episode metadata"
```

---

### Task 12: Viewer — revamp sync handler for async processing

**Files:**
- Modify: `viewers/jellyfin/src/handlers.rs`
- Modify: `viewers/jellyfin/src/file_organizer.rs`
- Modify: `viewers/jellyfin/src/main.rs`

**Step 1: Update AppState to include DB pool and bangumi client**

In `viewers/jellyfin/src/main.rs`, update the state:

```rust
mod bangumi_client;
mod db;
mod file_organizer;
mod handlers;
mod models;
mod nfo_generator;
mod schema;

use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub organizer: Arc<FileOrganizer>,
    pub db: db::DbPool,
    pub bangumi: Arc<bangumi_client::BangumiClient>,
}
```

Update the main function to create DB pool and AppState, and pass it to the router. The router state changes from `Arc<FileOrganizer>` to `AppState`.

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let downloads_dir = std::env::var("DOWNLOADS_DIR").unwrap_or_else(|_| "/downloads".to_string());
    let library_dir = std::env::var("JELLYFIN_LIBRARY_DIR").unwrap_or_else(|_| "/media/jellyfin".to_string());
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://bangumi:bangumi_dev_password@localhost:5432/viewer_jellyfin".to_string());

    let organizer = Arc::new(file_organizer::FileOrganizer::new(
        std::path::PathBuf::from(&downloads_dir),
        std::path::PathBuf::from(&library_dir),
    ));

    let db_pool = db::create_pool(&database_url);
    let bangumi = Arc::new(bangumi_client::BangumiClient::new());

    let state = AppState {
        organizer,
        db: db_pool,
        bangumi,
    };

    let app = axum::Router::new()
        .route("/sync", axum::routing::post(handlers::sync))
        .route("/health", axum::routing::get(handlers::health_check))
        .with_state(state);

    // ... port binding and registration (keep existing pattern) ...
}
```

**Step 2: Rewrite handlers.rs for async processing**

Replace `viewers/jellyfin/src/handlers.rs` with the new async handler:

```rust
use crate::bangumi_client::BangumiClient;
use crate::db::DbPool;
use crate::file_organizer::FileOrganizer;
use crate::models::*;
use crate::nfo_generator;
use crate::schema::{bangumi_episodes, bangumi_mapping, bangumi_subjects, sync_tasks};
use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use chrono::NaiveDate;
use diesel::prelude::*;
use serde::Serialize;
use shared::ViewerSyncRequest;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

pub async fn health_check() -> (StatusCode, Json<HealthResponse>) {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "healthy".to_string(),
            service: "jellyfin-viewer".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }),
    )
}

pub async fn sync(
    State(state): State<AppState>,
    Json(req): Json<ViewerSyncRequest>,
) -> StatusCode {
    tracing::info!(
        "Received sync request: download_id={}, anime={} S{:02}E{:02}",
        req.download_id,
        req.anime_title,
        req.series_no,
        req.episode_no
    );

    // Record sync task
    if let Ok(mut conn) = state.db.get() {
        let new_task = NewSyncTask {
            download_id: req.download_id,
            core_series_id: req.series_id,
            episode_no: req.episode_no,
            source_path: req.file_path.clone(),
            status: "processing".to_string(),
        };
        let _ = diesel::insert_into(sync_tasks::table)
            .values(&new_task)
            .execute(&mut conn);
    }

    // Spawn async processing
    let organizer = state.organizer.clone();
    let db = state.db.clone();
    let bangumi = state.bangumi.clone();
    tokio::spawn(async move {
        process_sync(organizer, db, bangumi, req).await;
    });

    StatusCode::ACCEPTED
}

async fn process_sync(
    organizer: Arc<FileOrganizer>,
    db: DbPool,
    bangumi: Arc<BangumiClient>,
    req: ViewerSyncRequest,
) {
    let result = do_sync(&organizer, &db, &bangumi, &req).await;

    // Update sync_task record
    if let Ok(mut conn) = db.get() {
        let now = chrono::Utc::now().naive_utc();
        match &result {
            Ok(target_path) => {
                let _ = diesel::update(
                    sync_tasks::table
                        .filter(sync_tasks::download_id.eq(req.download_id))
                        .filter(sync_tasks::status.eq("processing")),
                )
                .set((
                    sync_tasks::status.eq("completed"),
                    sync_tasks::target_path.eq(target_path),
                    sync_tasks::completed_at.eq(Some(now)),
                ))
                .execute(&mut conn);
            }
            Err(e) => {
                let _ = diesel::update(
                    sync_tasks::table
                        .filter(sync_tasks::download_id.eq(req.download_id))
                        .filter(sync_tasks::status.eq("processing")),
                )
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
            "Failed to callback Core for download {}: {}",
            req.download_id,
            e
        );
    }
}

async fn do_sync(
    organizer: &FileOrganizer,
    db: &DbPool,
    bangumi: &BangumiClient,
    req: &ViewerSyncRequest,
) -> anyhow::Result<String> {
    // 1. Move the file
    let source = std::path::Path::new(&req.file_path);
    let target_path = organizer
        .organize_episode(
            &req.anime_title,
            req.series_no as u32,
            req.episode_no as u32,
            source,
        )
        .await?;

    // 2. Fetch bangumi metadata (best-effort)
    if let Err(e) = fetch_and_generate_metadata(
        db,
        bangumi,
        organizer,
        req.series_id,
        &req.anime_title,
        req.series_no,
        req.episode_no,
        &target_path,
    )
    .await
    {
        tracing::warn!(
            "Metadata fetch failed for download {} (non-fatal): {}",
            req.download_id,
            e
        );
        // Non-fatal: file is already moved, that's the success criteria
    }

    Ok(target_path.display().to_string())
}

async fn fetch_and_generate_metadata(
    db: &DbPool,
    bangumi: &BangumiClient,
    organizer: &FileOrganizer,
    series_id: i32,
    anime_title: &str,
    series_no: i32,
    episode_no: i32,
    target_path: &std::path::Path,
) -> anyhow::Result<()> {
    let mut conn = db.get().map_err(|e| anyhow::anyhow!("{}", e))?;

    // Check if we already have a mapping
    let mapping = bangumi_mapping::table
        .filter(bangumi_mapping::core_series_id.eq(series_id))
        .first::<BangumiMapping>(&mut conn)
        .optional()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let bangumi_id = if let Some(m) = mapping {
        m.bangumi_id
    } else {
        // Search bangumi.tv
        let found_id = bangumi.search_anime(anime_title).await?;
        match found_id {
            Some(id) => {
                // Fetch and cache subject
                let subject = bangumi.get_subject(id).await?;
                cache_subject(&mut conn, &subject)?;

                // Fetch and cache episodes
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let episodes = bangumi.get_episodes(id).await?;
                cache_episodes(&mut conn, id, &episodes)?;

                // Create mapping
                let new_mapping = NewBangumiMapping {
                    core_series_id: series_id,
                    bangumi_id: id,
                    title_cache: Some(anime_title.to_string()),
                    source: "auto_search".to_string(),
                };
                diesel::insert_into(bangumi_mapping::table)
                    .values(&new_mapping)
                    .execute(&mut conn)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                id
            }
            None => {
                tracing::warn!("No bangumi.tv match found for '{}'", anime_title);
                return Ok(()); // Non-fatal
            }
        }
    };

    // Load cached subject
    let subject = bangumi_subjects::table
        .filter(bangumi_subjects::bangumi_id.eq(bangumi_id))
        .first::<BangumiSubject>(&mut conn)
        .optional()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if let Some(subject) = subject {
        // Generate tvshow.nfo + poster.jpg in anime root directory
        let anime_dir = organizer
            .get_library_dir()
            .join(FileOrganizer::sanitize_filename(anime_title));

        let subject_detail = to_subject_detail(&subject);
        nfo_generator::generate_tvshow_nfo(&anime_dir, &subject_detail).await?;

        // Download poster if not exists
        if let Some(cover_url) = &subject.cover_url {
            let poster_path = anime_dir.join("poster.jpg");
            if !poster_path.exists() {
                if let Err(e) = bangumi.download_image(cover_url, &poster_path).await {
                    tracing::warn!("Failed to download poster: {}", e);
                }
            }
        }
    }

    // Generate episode NFO
    let episode = bangumi_episodes::table
        .filter(bangumi_episodes::bangumi_id.eq(bangumi_id))
        .filter(bangumi_episodes::episode_no.eq(episode_no))
        .first::<BangumiEpisode>(&mut conn)
        .optional()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if let Some(ep) = episode {
        let ep_item = to_episode_item(&ep);
        nfo_generator::generate_episode_nfo(target_path, &ep_item, series_no).await?;
    }

    Ok(())
}

fn cache_subject(
    conn: &mut diesel::PgConnection,
    subject: &crate::bangumi_client::SubjectDetail,
) -> anyhow::Result<()> {
    let air_date = subject
        .date
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

    let new_subject = NewBangumiSubject {
        bangumi_id: subject.id,
        title: subject.name.clone(),
        title_cn: subject.name_cn.clone(),
        summary: subject.summary.clone(),
        rating: subject.rating.as_ref().and_then(|r| r.score),
        cover_url: subject
            .images
            .as_ref()
            .and_then(|i| i.large.clone()),
        air_date,
        episode_count: subject.total_episodes,
        raw_json: None,
    };

    diesel::insert_into(bangumi_subjects::table)
        .values(&new_subject)
        .on_conflict(bangumi_subjects::bangumi_id)
        .do_nothing()
        .execute(conn)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

fn cache_episodes(
    conn: &mut diesel::PgConnection,
    bangumi_id: i32,
    episodes: &[crate::bangumi_client::EpisodeItem],
) -> anyhow::Result<()> {
    for ep in episodes {
        let air_date = ep
            .airdate
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let new_ep = NewBangumiEpisode {
            bangumi_ep_id: ep.id,
            bangumi_id,
            episode_no: ep.ep.unwrap_or(ep.sort),
            title: ep.name.clone(),
            title_cn: ep.name_cn.clone(),
            air_date,
            summary: ep.desc.clone(),
        };

        diesel::insert_into(bangumi_episodes::table)
            .values(&new_ep)
            .on_conflict(bangumi_episodes::bangumi_ep_id)
            .do_nothing()
            .execute(conn)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    Ok(())
}

/// Convert DB model to bangumi_client type for NFO generation
fn to_subject_detail(s: &BangumiSubject) -> crate::bangumi_client::SubjectDetail {
    crate::bangumi_client::SubjectDetail {
        id: s.bangumi_id,
        name: s.title.clone(),
        name_cn: s.title_cn.clone(),
        summary: s.summary.clone(),
        date: s.air_date.map(|d| d.format("%Y-%m-%d").to_string()),
        images: s.cover_url.as_ref().map(|url| crate::bangumi_client::SubjectImages {
            large: Some(url.clone()),
            common: None,
            medium: None,
            small: None,
        }),
        rating: s.rating.map(|score| crate::bangumi_client::SubjectRating {
            score: Some(score),
            total: None,
        }),
        total_episodes: s.episode_count,
    }
}

fn to_episode_item(ep: &BangumiEpisode) -> crate::bangumi_client::EpisodeItem {
    crate::bangumi_client::EpisodeItem {
        id: ep.bangumi_ep_id,
        ep: Some(ep.episode_no),
        sort: ep.episode_no,
        name: ep.title.clone(),
        name_cn: ep.title_cn.clone(),
        airdate: ep.air_date.map(|d| d.format("%Y-%m-%d").to_string()),
        desc: ep.summary.clone(),
    }
}
```

**Step 3: Update file_organizer.rs — use rename instead of hard_link/copy**

In `file_organizer.rs` (lines 62-65), replace:

```rust
        // Move the file (not link/copy)
        fs::rename(source_file, &target_path).await?;
```

Remove the `#[allow(dead_code)]` from `source_dir` field since it may be used.

**Step 4: Verify compilation**

Run: `cargo check -p viewer-jellyfin`
Expected: PASS

**Step 5: Commit**

```bash
git add viewers/jellyfin/src/
git commit -m "feat(viewer): async sync handler with bangumi.tv metadata and NFO generation"
```

---

### Task 13: Viewer — update main.rs with new AppState and DB

**Files:**
- Modify: `viewers/jellyfin/src/main.rs`

**Step 1: Rewrite main.rs**

Update `viewers/jellyfin/src/main.rs` with:
- DB pool initialization
- New AppState with organizer + db + bangumi
- diesel_migrations embed and run
- Keep existing registration and graceful port binding pattern

Key changes:
- Add `mod bangumi_client; mod db; mod models; mod nfo_generator; mod schema;`
- Create `AppState` struct
- Initialize `db::create_pool()` with `DATABASE_URL`
- Create `BangumiClient::new()`
- Run diesel embedded migrations on startup
- Router state changes from `Arc<FileOrganizer>` to `AppState`

**Step 2: Verify full build**

Run: `cargo build -p viewer-jellyfin`
Expected: PASS

**Step 3: Commit**

```bash
git add viewers/jellyfin/src/main.rs
git commit -m "feat(viewer): update main with AppState, DB pool, and embedded migrations"
```

---

### Task 14: End-to-end verification

**Step 1: Format all code**

Run: `cargo fmt --all`

**Step 2: Check all packages**

Run: `cargo check --workspace`
Note: Ignore pre-existing core-service Jsonb errors.

**Step 3: Run all tests**

Run: `cargo test --workspace`

**Step 4: Run viewer-specific tests**

Run: `cargo test -p viewer-jellyfin`

**Step 5: Commit any formatting changes**

```bash
git add -A
git commit -m "style: cargo fmt --all"
```

---

### Task 15: Manual integration test

**Step 1: Verify database setup**

```bash
PGPASSWORD=bangumi_dev_password psql -h 172.20.0.4 -U bangumi -d viewer_jellyfin -c "\dt"
```

Expected: 4 tables (bangumi_subjects, bangumi_episodes, bangumi_mapping, sync_tasks)

**Step 2: Start all services**

Start core-service, downloader, fetcher, and viewer. Verify viewer registers with Core and Core shows the viewer in service list.

**Step 3: Test the full flow**

Create a subscription → fetcher runs → items parsed → download dispatched → download completes → DownloadScheduler triggers sync → Viewer moves file and generates NFO → callback updates Core.

**Step 4: Verify results**

- Check `downloads` table: status should be `synced`
- Check `sync_tasks` table in viewer_jellyfin DB: status should be `completed`
- Check file system: files should be in `/media/jellyfin/{title}/Season XX/`
- Check NFO files generated
