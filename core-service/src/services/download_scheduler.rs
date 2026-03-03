use diesel::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

use crate::db::DbPool;
use crate::models::{Download, ModuleTypeEnum, ServiceModule};
use crate::schema::{anime_links, downloads, service_modules};
use shared::{build_default_chain, classify_files, collect_files_recursive, match_batch_files,
             FileType, StatusQueryResponse};

pub struct DownloadScheduler {
    db_pool: DbPool,
    poll_interval_secs: u64,
    http_client: reqwest::Client,
    sync_service: Arc<super::SyncService>,
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
        // Get all downloading records for this downloader
        let active_downloads: Vec<Download> = downloads::table
            .filter(downloads::module_id.eq(downloader.module_id))
            .filter(downloads::status.eq("downloading"))
            .load::<Download>(conn)
            .map_err(|e| format!("Failed to query active downloads: {}", e))?;

        if active_downloads.is_empty() {
            // Also check for downloader_error records to recover
            self.check_recovery(conn, downloader).await?;
            return Ok(());
        }

        // Collect hashes to query
        let hashes: Vec<String> = active_downloads
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
                // Update download records based on status
                for status_item in &response.statuses {
                    let new_status = match status_item.status.as_str() {
                        "completed" => "completed",
                        "error" => "failed",
                        "downloading" | "stalledDL" | "metaDL" | "queuedDL" | "checkingDL"
                        | "forcedDL" | "allocating" | "moving" => "downloading",
                        _ => continue,
                    };

                    let now = chrono::Utc::now().naive_utc();

                    if new_status == "completed" {
                        Self::apply_completed_files(
                            conn,
                            &status_item.hash,
                            downloader.module_id,
                            status_item.content_path.as_deref(),
                            &status_item.files,
                            status_item.progress,
                            status_item.size,
                        );
                    } else {
                        diesel::update(
                            downloads::table
                                .filter(downloads::torrent_hash.eq(&status_item.hash))
                                .filter(downloads::module_id.eq(downloader.module_id)),
                        )
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

                // Trigger sync for newly completed downloads
                self.trigger_sync_for_completed(conn).await;

                Ok(())
            }
            Err(e) => {
                // Downloader offline — mark all downloading records as downloader_error
                tracing::warn!(
                    "Downloader {} offline, marking {} downloads as downloader_error: {}",
                    downloader.name,
                    active_downloads.len(),
                    e
                );

                let now = chrono::Utc::now().naive_utc();
                diesel::update(
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

                Ok(())
            }
        }
    }

    /// Check for downloader_error records and attempt recovery
    async fn check_recovery(
        &self,
        conn: &mut PgConnection,
        downloader: &ServiceModule,
    ) -> Result<(), String> {
        let error_downloads: Vec<Download> = downloads::table
            .filter(downloads::module_id.eq(downloader.module_id))
            .filter(downloads::status.eq("downloader_error"))
            .load::<Download>(conn)
            .map_err(|e| format!("Failed to query error downloads: {}", e))?;

        if error_downloads.is_empty() {
            return Ok(());
        }

        let hashes: Vec<String> = error_downloads
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
                tracing::info!(
                    "Downloader {} recovered, updating {} error records",
                    downloader.name,
                    response.statuses.len()
                );

                for status_item in &response.statuses {
                    let new_status = match status_item.status.as_str() {
                        "completed" => "completed",
                        "error" => "failed",
                        _ => "downloading",
                    };

                    let now = chrono::Utc::now().naive_utc();

                    if new_status == "completed" {
                        Self::apply_completed_files(
                            conn,
                            &status_item.hash,
                            downloader.module_id,
                            status_item.content_path.as_deref(),
                            &status_item.files,
                            status_item.progress,
                            status_item.size,
                        );
                    } else {
                        diesel::update(
                            downloads::table
                                .filter(downloads::torrent_hash.eq(&status_item.hash))
                                .filter(downloads::module_id.eq(downloader.module_id)),
                        )
                        .set((
                            downloads::status.eq(new_status),
                            downloads::progress.eq(status_item.progress as f32),
                            downloads::total_bytes.eq(status_item.size as i64),
                            downloads::error_message.eq::<Option<String>>(None),
                            downloads::updated_at.eq(now),
                        ))
                        .execute(conn)
                        .ok();
                    }
                }

                // Trigger sync for newly completed downloads
                self.trigger_sync_for_completed(conn).await;

                Ok(())
            }
            Err(_) => {
                // Still offline, nothing to do
                Ok(())
            }
        }
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
                tracing::warn!(
                    "Reset {} stale syncing downloads back to completed",
                    count
                );
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
    fn retry_batch_unmatched(&self, conn: &mut PgConnection) {
        use std::collections::HashMap;

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
                            dl.download_id, ep, hash
                        ),
                        Err(e) => tracing::error!(
                            "retry_batch_unmatched: failed to update download_id={}: {}",
                            dl.download_id, e
                        ),
                    }
                }
            }
        }
    }

    /// Update download records for a completed torrent.
    ///
    /// - Single record: bulk-set video_file to first video found (existing behaviour).
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
            .filter(downloads::status.eq("downloading"))
            .select((downloads::download_id, anime_links::episode_no))
            .load::<(i32, i32)>(conn)
            .unwrap_or_default();

        if records.is_empty() {
            return;
        }

        if records.len() == 1 {
            // Single episode: original behaviour — pick first video
            let (video_file, subtitle_files_json) = Self::extract_media_files(files);
            diesel::update(
                downloads::table.filter(downloads::download_id.eq(records[0].0)),
            )
            .set((
                downloads::status.eq("completed"),
                downloads::progress.eq(progress as f32),
                downloads::total_bytes.eq(size as i64),
                downloads::file_path.eq(content_path),
                downloads::video_file.eq(video_file.as_deref()),
                downloads::subtitle_files.eq(subtitle_files_json.as_deref()),
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
        let video = classified.iter()
            .find(|f| f.file_type == FileType::Video)
            .map(|f| f.path.clone());
        let subtitles: Vec<String> = classified.iter()
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
