use crate::db::DbPool;
use crate::file_organizer::FileOrganizer;
use crate::metadata_client::MetadataClient;
use crate::models::*;
use crate::nfo_generator;
use crate::schema::sync_tasks;
use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use diesel::prelude::*;
use serde::Serialize;
use shared::ViewerSyncRequest;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Download ids whose sync is being processed.
///
/// Core may send the same sync again while the first one is still running (it timed
/// out waiting for our 202, or recovered a long `syncing`). A second copy would find
/// the source already moved and report a failure, so it is acknowledged and dropped.
#[derive(Clone, Default)]
pub struct InFlightSyncs(Arc<Mutex<HashSet<i32>>>);

impl InFlightSyncs {
    /// Marks `download_id` as in progress, or returns `None` if it already is.
    /// The mark is cleared when the returned guard is dropped.
    pub fn try_start(&self, download_id: i32) -> Option<InFlightGuard> {
        let inserted = self
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(download_id);
        inserted.then(|| InFlightGuard {
            syncs: self.clone(),
            download_id,
        })
    }
}

pub struct InFlightGuard {
    syncs: InFlightSyncs,
    download_id: i32,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.syncs
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.download_id);
    }
}

/// Run a DB operation off the async worker: r2d2 `get` blocks while the pool is busy,
/// and the viewer has a single worker thread that also has to answer HTTP requests.
async fn with_conn<T, F>(db: &DbPool, f: F) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: FnOnce(&mut diesel::PgConnection) -> QueryResult<T> + Send + 'static,
{
    let db = db.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = db.get()?;
        Ok(f(&mut conn)?)
    })
    .await?
}

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

pub async fn sync(State(state): State<AppState>, Json(req): Json<ViewerSyncRequest>) -> StatusCode {
    tracing::info!(
        "Received sync request: download_id={}, anime={} S{:02}E{:02}",
        req.download_id,
        req.anime_title,
        req.series_no,
        req.episode_no
    );

    let Some(guard) = state.in_flight.try_start(req.download_id) else {
        tracing::info!(
            "Sync for download {} is already in progress, ignoring duplicate request",
            req.download_id
        );
        return StatusCode::ACCEPTED;
    };

    // Answer right away; everything that touches the DB or the disk runs in the task
    let organizer = state.organizer.clone();
    let db = state.db.clone();
    let metadata = state.metadata.clone();
    tokio::spawn(async move {
        process_sync(organizer, db, metadata, req).await;
        drop(guard);
    });

    StatusCode::ACCEPTED
}

async fn process_sync(
    organizer: Arc<FileOrganizer>,
    db: DbPool,
    metadata: Arc<MetadataClient>,
    req: ViewerSyncRequest,
) {
    // Record sync task and get task_id
    let new_task = NewSyncTask {
        download_id: req.download_id,
        status: "processing".to_string(),
    };
    let task_id = with_conn(&db, move |conn| {
        diesel::insert_into(sync_tasks::table)
            .values(&new_task)
            .returning(sync_tasks::task_id)
            .get_result::<i32>(conn)
    })
    .await
    .ok();

    let result = do_sync(&organizer, &metadata, &req).await;

    // Update sync_task record by task_id
    if let Some(tid) = task_id {
        let now = chrono::Utc::now().naive_utc();
        let (status, target_path) = match &result {
            Ok(target_path) => ("completed", Some(target_path.clone())),
            Err(_) => ("failed", None),
        };
        let _ = with_conn(&db, move |conn| {
            diesel::update(sync_tasks::table.filter(sync_tasks::task_id.eq(tid)))
                .set((
                    sync_tasks::status.eq(status),
                    sync_tasks::target_path.eq(target_path),
                    sync_tasks::completed_at.eq(Some(now)),
                ))
                .execute(conn)
        })
        .await;
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

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("valid reqwest client config");
    let callback_url = req.callback_url.clone();
    let download_id = req.download_id;
    let result = shared::retry_with_backoff(10, std::time::Duration::from_secs(2), || {
        let client = client.clone();
        let url = callback_url.clone();
        let cb = callback.clone();
        async move {
            let resp = client.post(&url).json(&cb).send().await?;
            resp.error_for_status().map(|_| ())
        }
    })
    .await;

    if let Err(e) = result {
        tracing::error!(
            "Failed to callback Core for download {} after retries: {}",
            download_id,
            e
        );
    }
}

async fn do_sync(
    organizer: &FileOrganizer,
    metadata: &MetadataClient,
    req: &ViewerSyncRequest,
) -> anyhow::Result<String> {
    // 1. Move the file (resolve container path → local path)
    let source = organizer.resolve_download_path(&req.video_path).await;
    let target_path = organizer
        .organize_episode(
            &req.anime_title,
            req.series_no as u32,
            req.episode_no as u32,
            &source,
        )
        .await?;

    // 1b. Move subtitle files (non-fatal)
    if !req.subtitle_paths.is_empty() {
        organizer
            .organize_subtitles(
                &req.subtitle_paths,
                &req.anime_title,
                req.series_no as u32,
                req.episode_no as u32,
            )
            .await;
    }

    // 2. Write NFO files and posters (best-effort: the move is what counts)
    fetch_and_generate_metadata(
        metadata,
        organizer,
        &EpisodeMetadata {
            external_id: req.external_id.as_ref(),
            series: req.series.as_ref(),
            season: req.season.as_ref(),
            anime_title: &req.anime_title,
            series_no: req.series_no,
            episode_no: req.episode_no,
        },
        &target_path,
    )
    .await;

    Ok(target_path.display().to_string())
}

/// 一集同步後要寫進媒體庫的 metadata
struct EpisodeMetadata<'a> {
    external_id: Option<&'a shared::ExternalId>,
    series: Option<&'a shared::ViewerSeries>,
    season: Option<&'a shared::ViewerSeason>,
    anime_title: &'a str,
    series_no: i32,
    episode_no: i32,
}

/// 海報不存在時才下載（同一季的每一集都會呼叫，不必每次重抓）
async fn download_poster(metadata: &MetadataClient, url: Option<&str>, dir: &std::path::Path) {
    let Some(url) = url else { return };
    let poster = dir.join("poster.jpg");
    if tokio::fs::try_exists(&poster).await.unwrap_or(false) {
        return;
    }
    if let Err(e) = tokio::fs::create_dir_all(dir).await {
        tracing::warn!("Failed to create {}: {}", dir.display(), e);
        return;
    }
    if let Err(e) = metadata.download_image(url, &poster).await {
        tracing::warn!("Failed to download poster {}: {}", poster.display(), e);
    }
}

/// 寫出系列的 `tvshow.nfo` 與海報、季的 `season.nfo` 與海報、這一集的 NFO（spec 第 9 節）。
/// 每一部分都是 best-effort：檔案已經搬好，metadata 失敗不影響同步結果。
async fn fetch_and_generate_metadata(
    metadata: &MetadataClient,
    organizer: &FileOrganizer,
    episode: &EpisodeMetadata<'_>,
    target_path: &std::path::Path,
) {
    let series_dir = organizer
        .get_library_dir()
        .join(FileOrganizer::sanitize_filename(episode.anime_title));
    let season_dir = target_path.parent().unwrap_or(&series_dir).to_path_buf();

    if let Some(series) = episode.series {
        if let Err(e) = nfo_generator::generate_tvshow_nfo(&series_dir, series).await {
            tracing::warn!("Failed to write tvshow.nfo for '{}': {}", series.title, e);
        }
        download_poster(metadata, series.poster_url.as_deref(), &series_dir).await;
    }
    if let Some(season) = episode.season {
        if let Err(e) =
            nfo_generator::generate_season_nfo(&season_dir, episode.series_no, season).await
        {
            tracing::warn!(
                "Failed to write season.nfo in {}: {}",
                season_dir.display(),
                e
            );
        }
        download_poster(metadata, season.poster_url.as_deref(), &season_dir).await;
    }

    let Some(external_id) = episode.external_id else {
        tracing::info!(
            "No identity for '{}', skipping episode NFO",
            episode.anime_title
        );
        return;
    };
    match metadata
        .enrich_episodes(external_id, episode.episode_no)
        .await
    {
        Ok(Some(ep_info)) => {
            let episode_data = crate::nfo_generator::EpisodeNfoData {
                title: ep_info.title,
                title_cn: ep_info.title_cn,
                air_date: ep_info.air_date,
                summary: ep_info.summary,
            };
            if let Err(e) = nfo_generator::generate_episode_nfo(
                target_path,
                &episode_data,
                episode.episode_no,
                episode.series_no,
            )
            .await
            {
                tracing::warn!(
                    "Failed to write episode NFO for {}: {}",
                    target_path.display(),
                    e
                );
            }
        }
        Ok(None) => tracing::warn!(
            "No episode info in Metadata Service for {} episode {}",
            external_id,
            episode.episode_no
        ),
        Err(e) => tracing::warn!("Failed to fetch episode info from Metadata Service: {}", e),
    }
}

pub async fn resync(
    State(state): State<AppState>,
    Json(req): Json<shared::ViewerResyncRequest>,
) -> StatusCode {
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
            status: "processing".to_string(),
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
    let metadata = state.metadata.clone();
    tokio::spawn(async move {
        process_resync(organizer, db, metadata, req, task_id).await;
    });

    StatusCode::ACCEPTED
}

async fn process_resync(
    organizer: Arc<FileOrganizer>,
    db: DbPool,
    metadata: Arc<MetadataClient>,
    req: shared::ViewerResyncRequest,
    task_id: Option<i32>,
) {
    let result = do_resync(&organizer, &db, &metadata, &req).await;

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
            Err(_) => {
                let _ = diesel::update(sync_tasks::table.filter(sync_tasks::task_id.eq(tid)))
                    .set((
                        sync_tasks::status.eq("failed"),
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

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("valid reqwest client config");
    let callback_url = req.callback_url.clone();
    let download_id = req.download_id;
    let result = shared::retry_with_backoff(10, std::time::Duration::from_secs(2), || {
        let client = client.clone();
        let url = callback_url.clone();
        let cb = callback.clone();
        async move {
            let resp = client.post(&url).json(&cb).send().await?;
            resp.error_for_status().map(|_| ())
        }
    })
    .await;

    if let Err(e) = result {
        tracing::error!(
            "Failed to callback Core for resync download {} after retries: {}",
            download_id,
            e
        );
    }
}

async fn do_resync(
    organizer: &FileOrganizer,
    db: &DbPool,
    metadata: &MetadataClient,
    req: &shared::ViewerResyncRequest,
) -> anyhow::Result<String> {
    // 1. Find the actual current file path from our DB
    let current_path = {
        let mut conn = db.get().map_err(|e| anyhow::anyhow!("{}", e))?;
        let latest_target: Option<Option<String>> = sync_tasks::table
            .filter(sync_tasks::download_id.eq(req.download_id))
            .filter(sync_tasks::status.eq("completed"))
            .order(sync_tasks::completed_at.desc())
            .select(sync_tasks::target_path)
            .first::<Option<String>>(&mut conn)
            .optional()
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        match latest_target.flatten() {
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
    let old_nfo = current_path.with_extension("nfo");
    if old_nfo.exists() && old_nfo != new_path.with_extension("nfo") {
        let _ = tokio::fs::remove_file(&old_nfo).await;
    }

    // 4. Regenerate NFO files and posters
    fetch_and_generate_metadata(
        metadata,
        organizer,
        &EpisodeMetadata {
            external_id: req.external_id.as_ref(),
            series: req.series.as_ref(),
            season: req.season.as_ref(),
            anime_title: &req.anime_title,
            series_no: req.series_no,
            episode_no: req.episode_no,
        },
        &new_path,
    )
    .await;

    Ok(new_path.display().to_string())
}

pub async fn delete_synced(
    State(state): State<AppState>,
    Json(req): Json<shared::ViewerDeleteRequest>,
) -> (StatusCode, Json<shared::ViewerDeleteResponse>) {
    tracing::info!(
        "Received delete request for {} downloads",
        req.download_ids.len()
    );

    let mut results = Vec::new();

    for download_id in &req.download_ids {
        let expected = req.expected_paths.get(download_id).map(String::as_str);
        let result = delete_single_download(&state, *download_id, expected).await;
        results.push(result);
    }

    let success_count = results.iter().filter(|r| r.success).count();
    tracing::info!(
        "Delete completed: {}/{} successful",
        success_count,
        results.len()
    );

    (
        StatusCode::OK,
        Json(shared::ViewerDeleteResponse { deleted: results }),
    )
}

/// POST /downloads/cleanup — 刪除下載目錄第一層的孤兒項目（檔案或整個資料夾）。
///
/// 哪些是孤兒由 core 判斷；這裡只確保刪的是下載目錄裡的直接子項目。
pub async fn cleanup_download_dir(
    State(state): State<AppState>,
    Json(req): Json<shared::DownloadDirCleanupRequest>,
) -> (StatusCode, Json<shared::DownloadDirCleanupResponse>) {
    let source_dir = state.organizer.get_source_dir().to_path_buf();
    let mut results = Vec::with_capacity(req.names.len());
    for name in req.names {
        let outcome = match download_dir_entry(&source_dir, &name) {
            Ok(path) => remove_entry(&path).await,
            Err(e) => Err(e),
        };
        match &outcome {
            Ok(()) => tracing::info!("Removed orphan {} from the download directory", name),
            Err(e) => tracing::warn!("Did not remove orphan {}: {}", name, e),
        }
        results.push(shared::DownloadDirCleanupResult {
            name,
            success: outcome.is_ok(),
            error_message: outcome.err(),
        });
    }
    (
        StatusCode::OK,
        Json(shared::DownloadDirCleanupResponse { results }),
    )
}

/// 下載目錄裡名為 `name` 的直接子項目；名稱含路徑分隔或 `.` / `..` 時拒絕。
fn download_dir_entry(source_dir: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    let mut components = std::path::Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) if !name.contains('\\') => {
            Ok(source_dir.join(name))
        }
        _ => Err(format!("{name} 不是下載目錄裡的項目名稱，未刪除")),
    }
}

/// 刪除檔案、符號連結（只刪連結本身）或整個資料夾
async fn remove_entry(path: &std::path::Path) -> Result<(), String> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(|e| format!("{}: {}", path.display(), e))?;
    let removed = if metadata.is_dir() {
        tokio::fs::remove_dir_all(path).await
    } else {
        tokio::fs::remove_file(path).await
    };
    removed.map_err(|e| format!("{}: {}", path.display(), e))
}

/// 決定要刪哪個檔案：viewer 的同步記錄與 core 帶來的路徑一致才刪；只有一邊有記錄時用那一邊。
/// 任何路徑都必須在媒體庫底下（不允許 `..`），避免刪到媒體庫以外的檔案。
fn resolve_delete_target(
    viewer_path: Option<&str>,
    core_path: Option<&str>,
    library_dir: &std::path::Path,
) -> Result<PathBuf, String> {
    let path = match (viewer_path, core_path) {
        (Some(v), Some(c)) if v != c => {
            return Err(format!(
                "路徑不一致，未刪除：viewer 記錄 {v}，core 記錄 {c}"
            ))
        }
        (Some(p), _) | (None, Some(p)) => PathBuf::from(p),
        (None, None) => return Err("No completed sync task found for this download".to_string()),
    };
    let inside_library = path.starts_with(library_dir)
        && !path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
    if inside_library {
        Ok(path)
    } else {
        Err(format!(
            "{} 不在媒體庫 {} 底下，未刪除",
            path.display(),
            library_dir.display()
        ))
    }
}

async fn delete_single_download(
    state: &AppState,
    download_id: i32,
    expected_path: Option<&str>,
) -> shared::ViewerDeleteResult {
    // Find the latest completed sync task for this download_id
    let target_path: Option<String> = match state.db.get() {
        Ok(mut conn) => sync_tasks::table
            .filter(sync_tasks::download_id.eq(download_id))
            .filter(sync_tasks::status.eq("completed"))
            .order(sync_tasks::completed_at.desc())
            .select(sync_tasks::target_path)
            .first::<Option<String>>(&mut conn)
            .ok()
            .flatten(),
        Err(_) => None,
    };

    let file_path = match resolve_delete_target(
        target_path.as_deref(),
        expected_path,
        state.organizer.get_library_dir(),
    ) {
        Ok(p) => p,
        Err(reason) => {
            tracing::warn!("Not deleting download {}: {}", download_id, reason);
            return shared::ViewerDeleteResult {
                download_id,
                success: false,
                deleted_path: None,
                error_message: Some(reason),
            };
        }
    };
    let target_path = file_path.display().to_string();

    // Delete the episode file
    if file_path.exists() {
        if let Err(e) = tokio::fs::remove_file(&file_path).await {
            return shared::ViewerDeleteResult {
                download_id,
                success: false,
                deleted_path: Some(target_path),
                error_message: Some(format!("Failed to delete file: {}", e)),
            };
        }
    }

    // Delete associated .nfo file
    let nfo_path = file_path.with_extension("nfo");
    if nfo_path.exists() {
        let _ = tokio::fs::remove_file(&nfo_path).await;
    }

    // Cleanup empty directories
    state.organizer.cleanup_empty_dirs(&file_path).await;

    shared::ViewerDeleteResult {
        download_id,
        success: true,
        deleted_path: Some(target_path),
        error_message: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, routing::post, Router};

    #[test]
    fn delete_targets_must_agree_and_stay_inside_the_library() {
        let lib = std::path::Path::new("/media/jellyfin");
        let a = "/media/jellyfin/X/Season 01/X - S01E01.mkv";
        let b = "/media/jellyfin/Y/Season 01/Y - S01E09.mkv";
        assert_eq!(
            resolve_delete_target(Some(a), Some(a), lib).unwrap(),
            PathBuf::from(a)
        );
        // 507 的情況：兩邊記錄不同就不刪
        assert!(resolve_delete_target(Some(a), Some(b), lib)
            .unwrap_err()
            .contains("不一致"));
        // 只有一邊有記錄時用那一邊
        assert_eq!(
            resolve_delete_target(Some(a), None, lib).unwrap(),
            PathBuf::from(a)
        );
        assert_eq!(
            resolve_delete_target(None, Some(b), lib).unwrap(),
            PathBuf::from(b)
        );
        assert!(resolve_delete_target(None, None, lib).is_err());
        // 媒體庫以外的路徑一律不刪
        assert!(resolve_delete_target(None, Some("/downloads/x.mkv"), lib).is_err());
        assert!(resolve_delete_target(None, Some("/media/jellyfin/../etc/passwd"), lib).is_err());
    }

    #[test]
    fn cleanup_accepts_only_direct_entries_of_the_download_dir() {
        let dir = std::path::Path::new("/downloads");
        assert_eq!(
            download_dir_entry(dir, "[Group] Show - 01 [1080p].mkv").unwrap(),
            PathBuf::from("/downloads/[Group] Show - 01 [1080p].mkv")
        );
        ["", ".", "..", "a/b", "../etc", "/etc", "a\\b"]
            .iter()
            .for_each(|name| assert!(download_dir_entry(dir, name).is_err(), "{name:?}"));
    }

    #[tokio::test]
    async fn cleanup_removes_files_and_whole_folders() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("leftover.mkv");
        let folder = dir.path().join("Batch");
        std::fs::write(&file, b"x").unwrap();
        std::fs::create_dir_all(folder.join("Fonts")).unwrap();
        std::fs::write(folder.join("Fonts/a.ttf"), b"x").unwrap();

        remove_entry(&file).await.unwrap();
        remove_entry(&folder).await.unwrap();
        assert!(!file.exists() && !folder.exists());
        assert!(remove_entry(&file).await.is_err());
    }

    /// 模擬 metadata service：第 1 集有資料，海報回傳固定位元組
    async fn mock_metadata() -> String {
        let app = Router::new()
            .route(
                "/enrich/episodes",
                post(|Json(body): Json<serde_json::Value>| async move {
                    if body["namespace"] == "bgm"
                        && body["external_id"] == "548818"
                        && body["episode_no"] == 1
                    {
                        Json(serde_json::json!({
                            "episode_no": 1, "title": "選手宣誓", "title_cn": "选手宣誓",
                            "air_date": "2026-01-24", "summary": "第一集"
                        }))
                        .into_response()
                    } else {
                        StatusCode::NOT_FOUND.into_response()
                    }
                }),
            )
            .route("/poster/:name", get(|| async { "jpg-bytes" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{}", addr)
    }

    use axum::response::IntoResponse;

    fn series(base: &str) -> shared::ViewerSeries {
        shared::ViewerSeries {
            title: "金牌得主".to_string(),
            summary: Some("系列簡介".to_string()),
            premiered: Some("2025-01-04".to_string()),
            poster_url: Some(format!("{base}/poster/series")),
        }
    }

    fn season(base: &str) -> shared::ViewerSeason {
        shared::ViewerSeason {
            title: Some("金牌得主 第二季".to_string()),
            poster_url: Some(format!("{base}/poster/season")),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn a_synced_episode_gets_series_season_and_episode_metadata() {
        let base = mock_metadata().await;
        let library = tempfile::tempdir().unwrap();
        let organizer = FileOrganizer::new(
            library.path().join("downloads"),
            library.path().to_path_buf(),
            Default::default(),
        );
        let metadata = MetadataClient::new(base.clone());
        let season_dir = library.path().join("金牌得主").join("Season 02");
        std::fs::create_dir_all(&season_dir).unwrap();
        let video = season_dir.join("金牌得主 - S02E01.mkv");
        std::fs::write(&video, b"video").unwrap();
        let id = shared::ExternalId::new("bgm", "548818");
        let (series, season) = (series(&base), season(&base));

        fetch_and_generate_metadata(
            &metadata,
            &organizer,
            &EpisodeMetadata {
                external_id: Some(&id),
                series: Some(&series),
                season: Some(&season),
                anime_title: "金牌得主",
                series_no: 2,
                episode_no: 1,
            },
            &video,
        )
        .await;

        let series_dir = library.path().join("金牌得主");
        let read = |p: std::path::PathBuf| std::fs::read_to_string(p).unwrap();
        assert!(read(series_dir.join("tvshow.nfo")).contains("<plot>系列簡介</plot>"));
        assert_eq!(read(series_dir.join("poster.jpg")), "jpg-bytes");
        assert!(read(season_dir.join("season.nfo")).contains("<title>金牌得主 第二季</title>"));
        assert_eq!(read(season_dir.join("poster.jpg")), "jpg-bytes");
        assert!(read(video.with_extension("nfo")).contains("<title>选手宣誓</title>"));
    }

    #[tokio::test]
    async fn without_identity_the_series_is_still_named_but_no_episode_nfo_is_written() {
        let base = mock_metadata().await;
        let library = tempfile::tempdir().unwrap();
        let organizer = FileOrganizer::new(
            library.path().join("downloads"),
            library.path().to_path_buf(),
            Default::default(),
        );
        let season_dir = library.path().join("金牌得主").join("Season 01");
        std::fs::create_dir_all(&season_dir).unwrap();
        let video = season_dir.join("金牌得主 - S01E01.mkv");
        let series = shared::ViewerSeries {
            poster_url: None,
            ..series(&base)
        };

        fetch_and_generate_metadata(
            &MetadataClient::new(base),
            &organizer,
            &EpisodeMetadata {
                external_id: None,
                series: Some(&series),
                season: None,
                anime_title: "金牌得主",
                series_no: 1,
                episode_no: 1,
            },
            &video,
        )
        .await;

        assert!(library.path().join("金牌得主/tvshow.nfo").exists());
        assert!(!video.with_extension("nfo").exists());
        assert!(!library.path().join("金牌得主/poster.jpg").exists());
    }

    #[tokio::test]
    async fn an_existing_poster_is_not_downloaded_again() {
        let base = mock_metadata().await;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("poster.jpg"), b"mine").unwrap();
        download_poster(
            &MetadataClient::new(base.clone()),
            Some(&format!("{base}/poster/x")),
            dir.path(),
        )
        .await;
        assert_eq!(
            std::fs::read(dir.path().join("poster.jpg")).unwrap(),
            b"mine"
        );
    }
}
