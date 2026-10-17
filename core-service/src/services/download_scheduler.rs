use diesel::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::interval;

use crate::db::DbPool;
use crate::models::{Download, ModuleTypeEnum, ServiceModule};
use crate::schema::{anime_links, downloads, service_modules};
use crate::services::SyncNotice;
use shared::{
    build_default_chain, classify_files, collect_files_recursive, match_batch_files,
    pick_main_video, FileType, StatusQueryResponse,
};

/// Statuses whose torrent is still tracked by the downloader and polled every cycle.
/// `downloader_error` is included so a download recovers as soon as its downloader answers again.
const POLLED_STATUSES: [&str; 2] = ["downloading", "downloader_error"];

/// Consecutive failed polls before a downloader's downloads are marked `downloader_error`.
/// A single failure (restart, deploy, a busy moment) must not flag everything.
const OFFLINE_AFTER_FAILED_POLLS: u32 = 3;

/// How long a torrent may stay missing from its downloader before the download is failed.
/// qBittorrent adds URL torrents asynchronously, so a short absence right after dispatch is normal.
const MISSING_GRACE: Duration = Duration::from_secs(10 * 60);

/// A download whose progress has not changed for this long is flagged as stalled.
/// Its status stays `downloading`; only the error message tells the user to look at it.
const STALL_AFTER_HOURS: i64 = 24;
const STALL_NOTE: &str = "No download progress for 24 hours (no reachable peers?)";

/// What a polled download should become, given the downloader's normalized status.
#[derive(Debug, PartialEq, Eq)]
enum PollOutcome {
    Completed,
    Failed,
    Downloading,
    /// The downloader does not know the torrent.
    Missing,
    /// No information to act on (e.g. `unknown`); leave the record as is.
    Unchanged,
}

fn poll_outcome(downloader_status: &str) -> PollOutcome {
    match downloader_status {
        "completed" => PollOutcome::Completed,
        "error" => PollOutcome::Failed,
        "downloading" | "paused" | "checking" => PollOutcome::Downloading,
        "not_found" => PollOutcome::Missing,
        _ => PollOutcome::Unchanged,
    }
}

pub struct DownloadScheduler {
    db_pool: DbPool,
    poll_interval_secs: u64,
    http_client: reqwest::Client,
    sync_service: Arc<super::SyncService>,
    /// Consecutive failed polls per downloader module_id.
    failed_polls: Mutex<HashMap<i32, u32>>,
    /// When each (module_id, torrent hash) was first reported `not_found` in a row of polls.
    missing_since: Mutex<HashMap<(i32, String), Instant>>,
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
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap(),
            sync_service,
            failed_polls: Mutex::new(HashMap::new()),
            missing_since: Mutex::new(HashMap::new()),
        }
    }

    pub async fn start(self: Arc<Self>) {
        let mut ticker = interval(Duration::from_secs(self.poll_interval_secs));

        tracing::info!(
            "DownloadScheduler started, polling every {} seconds",
            self.poll_interval_secs
        );

        loop {
            ticker.tick().await;

            if let Err(e) = self.poll_all_downloaders().await {
                tracing::error!("Download poll error: {}", e);
            }
        }
    }

    async fn poll_all_downloaders(&self) -> Result<(), String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        // Reset stale "syncing" downloads back to "completed" (stuck > 5 min)
        self.recover_stale_syncing(&mut conn);

        // Get all enabled downloader modules
        let downloaders: Vec<ServiceModule> = service_modules::table
            .filter(service_modules::is_enabled.eq(true))
            .filter(service_modules::module_type.eq(ModuleTypeEnum::Downloader))
            .select(ServiceModule::as_select())
            .load::<ServiceModule>(&mut conn)
            .map_err(|e| format!("Failed to query downloaders: {}", e))?;

        for downloader in &downloaders {
            if let Err(e) = self.poll_downloader(&mut conn, downloader).await {
                tracing::error!(
                    "Failed to poll downloader {} ({}): {}",
                    downloader.name,
                    downloader.base_url,
                    e
                );
            }
        }

        // Retry any batch_unmatched records
        if let Ok(mut conn) = self.db_pool.get() {
            self.retry_batch_unmatched(&mut conn);
        }

        Ok(())
    }

    async fn poll_downloader(
        &self,
        conn: &mut PgConnection,
        downloader: &ServiceModule,
    ) -> Result<(), String> {
        let polled_downloads: Vec<Download> = downloads::table
            .filter(downloads::module_id.eq(downloader.module_id))
            .filter(downloads::status.eq_any(POLLED_STATUSES))
            .load::<Download>(conn)
            .map_err(|e| format!("Failed to query active downloads: {}", e))?;

        let hashes: Vec<String> = polled_downloads
            .iter()
            .filter_map(|d| d.torrent_hash.clone())
            .collect();

        if hashes.is_empty() {
            return Ok(());
        }

        let hashes_param = hashes.join(",");
        let status_url = format!("{}/downloads?hashes={}", downloader.base_url, hashes_param);

        match self.query_downloader_status(&status_url).await {
            Ok(response) => {
                self.record_poll_success(downloader);
                self.forget_missing_except(downloader.module_id, &hashes);

                for status_item in &response.statuses {
                    let now = chrono::Utc::now().naive_utc();
                    let polled_rows = downloads::table
                        .filter(downloads::torrent_hash.eq(&status_item.hash))
                        .filter(downloads::module_id.eq(downloader.module_id))
                        .filter(downloads::status.eq_any(POLLED_STATUSES));

                    let outcome = poll_outcome(&status_item.status);
                    if outcome != PollOutcome::Missing {
                        self.forget_missing(downloader.module_id, &status_item.hash);
                    }

                    match outcome {
                        PollOutcome::Completed => Self::apply_completed_files(
                            conn,
                            &status_item.hash,
                            downloader.module_id,
                            status_item.content_path.as_deref(),
                            &status_item.files,
                            status_item.progress,
                            status_item.size,
                        ),
                        PollOutcome::Failed => {
                            diesel::update(polled_rows)
                                .set((
                                    downloads::status.eq("failed"),
                                    downloads::error_message
                                        .eq(Some("Downloader reported an error".to_string())),
                                    downloads::updated_at.eq(now),
                                ))
                                .execute(conn)
                                .ok();
                        }
                        PollOutcome::Downloading => {
                            // Only rows that made progress (or are recovering) are touched, so
                            // updated_at of a `downloading` row means "last progress".
                            let progress = status_item.progress as f32;
                            diesel::update(
                                polled_rows.filter(
                                    downloads::progress
                                        .is_distinct_from(progress)
                                        .or(downloads::status.eq("downloader_error")),
                                ),
                            )
                            .set((
                                downloads::status.eq("downloading"),
                                downloads::progress.eq(progress),
                                downloads::total_bytes.eq(status_item.size as i64),
                                downloads::error_message.eq::<Option<String>>(None),
                                downloads::updated_at.eq(now),
                            ))
                            .execute(conn)
                            .ok();
                            Self::flag_stalled(conn, &status_item.hash, downloader.module_id, now);
                        }
                        PollOutcome::Missing => {
                            let missing_for =
                                self.missing_for(downloader.module_id, &status_item.hash);
                            if missing_for >= MISSING_GRACE {
                                let failed = diesel::update(polled_rows)
                                    .set((
                                        downloads::status.eq("failed"),
                                        downloads::error_message.eq(Some(format!(
                                            "Torrent not found in downloader for {} minutes",
                                            missing_for.as_secs() / 60
                                        ))),
                                        downloads::updated_at.eq(now),
                                    ))
                                    .execute(conn)
                                    .unwrap_or(0);
                                tracing::warn!(
                                    "Torrent {} missing from downloader {} for {}s, marked {} downloads as failed",
                                    status_item.hash,
                                    downloader.name,
                                    missing_for.as_secs(),
                                    failed
                                );
                                self.forget_missing(downloader.module_id, &status_item.hash);
                            }
                        }
                        PollOutcome::Unchanged => {}
                    }
                }

                // Trigger sync for newly completed downloads
                self.trigger_sync_for_completed(conn).await;

                Ok(())
            }
            Err(e) => {
                let failed_polls = self.record_poll_failure(downloader);
                if failed_polls < OFFLINE_AFTER_FAILED_POLLS {
                    tracing::warn!(
                        "Polling downloader {} failed ({}/{}), will retry: {}",
                        downloader.name,
                        failed_polls,
                        OFFLINE_AFTER_FAILED_POLLS,
                        e
                    );
                    return Ok(());
                }

                let now = chrono::Utc::now().naive_utc();
                let marked = diesel::update(
                    downloads::table
                        .filter(downloads::module_id.eq(downloader.module_id))
                        .filter(downloads::status.eq("downloading")),
                )
                .set((
                    downloads::status.eq("downloader_error"),
                    downloads::error_message.eq(Some(format!("Downloader offline: {}", e))),
                    downloads::updated_at.eq(now),
                ))
                .execute(conn)
                .map_err(|e| format!("Failed to mark as downloader_error: {}", e))?;

                if marked > 0 {
                    tracing::warn!(
                        "Downloader {} offline after {} failed polls, marked {} downloads as downloader_error: {}",
                        downloader.name,
                        failed_polls,
                        marked,
                        e
                    );
                }

                Ok(())
            }
        }
    }

    fn record_poll_success(&self, downloader: &ServiceModule) {
        let previous = self
            .failed_polls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&downloader.module_id);
        if let Some(count) = previous {
            tracing::info!(
                "Downloader {} reachable again after {} failed polls",
                downloader.name,
                count
            );
        }
    }

    /// How long the torrent has been reported missing in consecutive polls (zero on first report).
    fn missing_for(&self, module_id: i32, hash: &str) -> Duration {
        self.missing_since
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry((module_id, hash.to_string()))
            .or_insert_with(Instant::now)
            .elapsed()
    }

    fn forget_missing(&self, module_id: i32, hash: &str) {
        self.missing_since
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&(module_id, hash.to_string()));
    }

    /// Drops entries of torrents this downloader no longer polls (synced, cancelled, ...).
    fn forget_missing_except(&self, module_id: i32, polled_hashes: &[String]) {
        self.missing_since
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|(m, h), _| *m != module_id || polled_hashes.contains(h));
    }

    /// Flags `downloading` rows of a torrent whose progress has not moved for STALL_AFTER_HOURS.
    fn flag_stalled(
        conn: &mut PgConnection,
        torrent_hash: &str,
        module_id: i32,
        now: chrono::NaiveDateTime,
    ) {
        let cutoff = now - chrono::Duration::hours(STALL_AFTER_HOURS);
        match diesel::update(
            downloads::table
                .filter(downloads::torrent_hash.eq(torrent_hash))
                .filter(downloads::module_id.eq(module_id))
                .filter(downloads::status.eq("downloading"))
                .filter(downloads::updated_at.lt(cutoff))
                .filter(downloads::error_message.is_null()),
        )
        .set(downloads::error_message.eq(STALL_NOTE))
        .execute(conn)
        {
            Ok(count) if count > 0 => tracing::warn!(
                "Torrent {} has made no progress for {} hours ({} downloads flagged)",
                torrent_hash,
                STALL_AFTER_HOURS,
                count
            ),
            Err(e) => tracing::error!("Failed to flag stalled torrent {}: {}", torrent_hash, e),
            _ => {}
        }
    }

    /// Returns the number of consecutive failed polls, including this one.
    fn record_poll_failure(&self, downloader: &ServiceModule) -> u32 {
        let mut failed_polls = self.failed_polls.lock().unwrap_or_else(|e| e.into_inner());
        let count = failed_polls.entry(downloader.module_id).or_insert(0);
        *count += 1;
        *count
    }

    async fn trigger_sync_for_completed(&self, conn: &mut PgConnection) {
        use crate::schema::anime_links;

        // Find downloads that are "completed" with file_path set, retry count < 3,
        // and NOT in conflict (conflict_flag=false, link_status='active')
        let completed: Vec<Download> = match downloads::table
            .inner_join(anime_links::table.on(anime_links::link_id.eq(downloads::link_id)))
            .filter(downloads::status.eq("completed"))
            .filter(downloads::file_path.is_not_null())
            .filter(downloads::sync_retry_count.lt(3))
            .filter(anime_links::conflict_flag.eq(false))
            .filter(anime_links::link_status.eq("active"))
            .select(Download::as_select())
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
                Ok(SyncNotice::Sent) => {
                    tracing::info!("Triggered sync for download {}", download.download_id);
                }
                Ok(SyncNotice::Skipped | SyncNotice::NoViewer) => {}
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

    /// Reset downloads stuck in "syncing" for more than 5 minutes back to "completed"
    /// so that the next poll cycle will re-trigger sync.
    fn recover_stale_syncing(&self, conn: &mut PgConnection) {
        let cutoff = chrono::Utc::now().naive_utc() - chrono::Duration::minutes(5);
        let now = chrono::Utc::now().naive_utc();

        match diesel::update(
            downloads::table
                .filter(downloads::status.eq("syncing"))
                .filter(downloads::updated_at.lt(cutoff)),
        )
        .set((
            downloads::status.eq("completed"),
            downloads::updated_at.eq(now),
        ))
        .execute(conn)
        {
            Ok(count) if count > 0 => {
                tracing::warn!("Reset {} stale syncing downloads back to completed", count);
            }
            Err(e) => {
                tracing::error!("Failed to recover stale syncing downloads: {}", e);
            }
            _ => {}
        }
    }

    /// Retry file matching for downloads in "batch_unmatched" status.
    /// The torrent is already downloaded; re-scan the folder and attempt
    /// to match again. On success, the record transitions to "completed"
    /// and will be picked up by trigger_sync_for_completed.
    ///
    /// Uses `sync_retry_count` to track retry attempts. After
    /// `BATCH_MATCH_MAX_RETRIES` failed attempts, transitions to "batch_failed".
    fn retry_batch_unmatched(&self, conn: &mut PgConnection) {
        use std::collections::HashMap;

        const BATCH_MATCH_MAX_RETRIES: i32 = 10;

        let unmatched: Vec<Download> = match downloads::table
            .filter(downloads::status.eq("batch_unmatched"))
            .filter(downloads::file_path.is_not_null())
            .load::<Download>(conn)
        {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Failed to query batch_unmatched: {}", e);
                return;
            }
        };

        if unmatched.is_empty() {
            return;
        }

        // Group by (torrent_hash, module_id) → (folder_path, records)
        let mut groups: HashMap<(String, i32), (String, Vec<&Download>)> = HashMap::new();
        for dl in &unmatched {
            if let (Some(hash), Some(mid), Some(fp)) =
                (&dl.torrent_hash, dl.module_id, &dl.file_path)
            {
                groups
                    .entry((hash.clone(), mid))
                    .or_insert_with(|| (fp.clone(), Vec::new()))
                    .1
                    .push(dl);
            }
        }

        for ((hash, _module_id), (folder, group)) in &groups {
            // Re-scan filesystem for current file list
            let files = collect_files_recursive(std::path::Path::new(folder));
            if files.is_empty() {
                tracing::warn!("retry_batch_unmatched: no files found in {}", folder);
                // Increment retry count even when folder is empty
                let now = chrono::Utc::now().naive_utc();
                for dl in group {
                    Self::increment_batch_retry(conn, dl, BATCH_MATCH_MAX_RETRIES, now);
                }
                continue;
            }

            // Resolve episode_no for each link_id
            let link_ids: Vec<i32> = group.iter().map(|d| d.link_id).collect();
            let ep_map: HashMap<i32, i32> = anime_links::table
                .filter(anime_links::link_id.eq_any(&link_ids))
                .select((anime_links::link_id, anime_links::episode_no))
                .load::<(i32, i32)>(conn)
                .unwrap_or_default()
                .into_iter()
                .collect();

            let episode_nos: Vec<i32> = ep_map.values().copied().collect();
            let chain = build_default_chain();
            let matches = match_batch_files(&files, &episode_nos, &chain);

            let now = chrono::Utc::now().naive_utc();
            for dl in group {
                let ep = match ep_map.get(&dl.link_id) {
                    Some(e) => *e,
                    None => continue,
                };
                if let Some((video, subs)) = matches.get(&ep) {
                    let subtitle_json = if subs.is_empty() {
                        None
                    } else {
                        serde_json::to_string(subs).ok()
                    };
                    let updated = diesel::update(
                        downloads::table.filter(downloads::download_id.eq(dl.download_id)),
                    )
                    .set((
                        downloads::status.eq("completed"),
                        downloads::video_file.eq(video.as_deref()),
                        downloads::subtitle_files.eq(subtitle_json.as_deref()),
                        downloads::error_message.eq(None::<String>),
                        downloads::updated_at.eq(now),
                    ))
                    .execute(conn);

                    match updated {
                        Ok(_) => tracing::info!(
                            "retry_batch_unmatched: recovered download_id={} ep={} hash={}",
                            dl.download_id,
                            ep,
                            hash
                        ),
                        Err(e) => tracing::error!(
                            "retry_batch_unmatched: failed to update download_id={}: {}",
                            dl.download_id,
                            e
                        ),
                    }
                } else {
                    // Still unmatched — increment retry count or transition to batch_failed
                    Self::increment_batch_retry(conn, dl, BATCH_MATCH_MAX_RETRIES, now);
                }
            }
        }
    }

    /// Increment retry count for a batch_unmatched download.
    /// If retries exceed the max, transition to "batch_failed" terminal state.
    fn increment_batch_retry(
        conn: &mut PgConnection,
        dl: &Download,
        max_retries: i32,
        now: chrono::NaiveDateTime,
    ) {
        let new_count = dl.sync_retry_count + 1;
        if new_count >= max_retries {
            diesel::update(downloads::table.filter(downloads::download_id.eq(dl.download_id)))
                .set((
                    downloads::status.eq("batch_failed"),
                    downloads::sync_retry_count.eq(new_count),
                    downloads::error_message.eq(Some(format!(
                        "Batch file matching failed after {} retries",
                        new_count
                    ))),
                    downloads::updated_at.eq(now),
                ))
                .execute(conn)
                .ok();
            tracing::warn!(
                "retry_batch_unmatched: download_id={} exceeded max retries ({}), marking as batch_failed",
                dl.download_id, max_retries
            );
        } else {
            diesel::update(downloads::table.filter(downloads::download_id.eq(dl.download_id)))
                .set((
                    downloads::sync_retry_count.eq(new_count),
                    downloads::updated_at.eq(now),
                ))
                .execute(conn)
                .ok();
        }
    }

    /// Update download records for a completed torrent (`downloading` or `downloader_error`).
    ///
    /// - Single record: set video_file to the main (largest) video.
    /// - Multiple records (batch torrent): use match_batch_files to assign
    ///   each record its specific video_file and subtitle_files.
    ///   Records that cannot be matched are set to status "batch_unmatched".
    fn apply_completed_files(
        conn: &mut PgConnection,
        torrent_hash: &str,
        module_id: i32,
        content_path: Option<&str>,
        files: &[String],
        progress: f64,
        size: u64,
    ) {
        let now = chrono::Utc::now().naive_utc();

        // Load all Download records + their episode_no for this torrent
        let records: Vec<(i32, i32)> = downloads::table
            .inner_join(anime_links::table.on(anime_links::link_id.eq(downloads::link_id)))
            .filter(downloads::torrent_hash.eq(torrent_hash))
            .filter(downloads::module_id.eq(module_id))
            .filter(downloads::status.eq_any(POLLED_STATUSES))
            .select((downloads::download_id, anime_links::episode_no))
            .load::<(i32, i32)>(conn)
            .unwrap_or_default();

        if records.is_empty() {
            return;
        }

        if records.len() == 1 {
            // Single episode: pick the main video
            let (video_file, subtitle_files_json) = Self::extract_media_files(files);
            diesel::update(downloads::table.filter(downloads::download_id.eq(records[0].0)))
                .set((
                    downloads::status.eq("completed"),
                    downloads::progress.eq(progress as f32),
                    downloads::total_bytes.eq(size as i64),
                    downloads::file_path.eq(content_path),
                    downloads::video_file.eq(video_file.as_deref()),
                    downloads::subtitle_files.eq(subtitle_files_json.as_deref()),
                    downloads::error_message.eq::<Option<String>>(None),
                    downloads::updated_at.eq(now),
                ))
                .execute(conn)
                .ok();
            return;
        }

        // Batch mode: match each episode to its specific file
        let episode_nos: Vec<i32> = records.iter().map(|(_, ep)| *ep).collect();
        let chain = build_default_chain();
        let matches = match_batch_files(files, &episode_nos, &chain);

        for (download_id, episode_no) in &records {
            if let Some((video, subs)) = matches.get(episode_no) {
                let subtitle_json = if subs.is_empty() {
                    None
                } else {
                    serde_json::to_string(subs).ok()
                };
                diesel::update(downloads::table.filter(downloads::download_id.eq(download_id)))
                    .set((
                        downloads::status.eq("completed"),
                        downloads::progress.eq(progress as f32),
                        downloads::total_bytes.eq(size as i64),
                        downloads::file_path.eq(content_path),
                        downloads::video_file.eq(video.as_deref()),
                        downloads::subtitle_files.eq(subtitle_json.as_deref()),
                        downloads::error_message.eq::<Option<String>>(None),
                        downloads::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .ok();
            } else {
                tracing::warn!(
                    "batch_unmatched: download_id={} episode_no={} torrent_hash={}",
                    download_id,
                    episode_no,
                    torrent_hash
                );
                diesel::update(downloads::table.filter(downloads::download_id.eq(download_id)))
                    .set((
                        downloads::status.eq("batch_unmatched"),
                        downloads::progress.eq(progress as f32),
                        downloads::total_bytes.eq(size as i64),
                        downloads::file_path.eq(content_path),
                        downloads::error_message.eq(Some(format!(
                            "Unable to match episode {} in {}",
                            episode_no,
                            content_path.unwrap_or("(unknown)")
                        ))),
                        downloads::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .ok();
            }
        }
    }

    /// 從 downloader 回報的檔案列表中提取影片路徑和字幕路徑（JSON 字串）。
    fn extract_media_files(files: &[String]) -> (Option<String>, Option<String>) {
        if files.is_empty() {
            return (None, None);
        }
        let classified = classify_files(files.to_vec());
        let video = pick_main_video(&classified, |p| std::fs::metadata(p).ok().map(|m| m.len()));
        let subtitles: Vec<String> = classified
            .iter()
            .filter(|f| f.file_type == FileType::Subtitle)
            .map(|f| f.path.clone())
            .collect();
        let subtitle_json = if subtitles.is_empty() {
            None
        } else {
            serde_json::to_string(&subtitles).ok()
        };
        (video, subtitle_json)
    }

    async fn query_downloader_status(&self, url: &str) -> Result<StatusQueryResponse, String> {
        let response = self
            .http_client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Downloader returned status: {}", response.status()));
        }

        response
            .json::<StatusQueryResponse>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_and_error_are_terminal_outcomes() {
        assert_eq!(poll_outcome("completed"), PollOutcome::Completed);
        assert_eq!(poll_outcome("error"), PollOutcome::Failed);
    }

    #[test]
    fn in_progress_statuses_keep_downloading() {
        for status in ["downloading", "paused", "checking"] {
            assert_eq!(poll_outcome(status), PollOutcome::Downloading, "{status}");
        }
    }

    #[test]
    fn not_found_is_reported_as_missing() {
        assert_eq!(poll_outcome("not_found"), PollOutcome::Missing);
    }

    #[test]
    fn statuses_without_information_leave_the_record_alone() {
        for status in ["unknown", ""] {
            assert_eq!(poll_outcome(status), PollOutcome::Unchanged, "{status}");
        }
    }

    #[test]
    fn downloader_error_is_polled_so_it_can_recover() {
        assert!(POLLED_STATUSES.contains(&"downloading"));
        assert!(POLLED_STATUSES.contains(&"downloader_error"));
    }
}
