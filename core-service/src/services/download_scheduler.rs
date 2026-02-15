use diesel::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

use crate::db::DbPool;
use crate::models::{Download, ModuleTypeEnum, ServiceModule};
use crate::schema::{downloads, service_modules};
use shared::StatusQueryResponse;

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
            http_client: reqwest::Client::new(),
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
                        // Store content_path when completed
                        diesel::update(
                            downloads::table
                                .filter(downloads::torrent_hash.eq(&status_item.hash))
                                .filter(downloads::module_id.eq(downloader.module_id)),
                        )
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
                        diesel::update(
                            downloads::table
                                .filter(downloads::torrent_hash.eq(&status_item.hash))
                                .filter(downloads::module_id.eq(downloader.module_id)),
                        )
                        .set((
                            downloads::status.eq(new_status),
                            downloads::progress.eq(status_item.progress as f32),
                            downloads::total_bytes.eq(status_item.size as i64),
                            downloads::file_path.eq(&status_item.content_path),
                            downloads::error_message.eq::<Option<String>>(None),
                            downloads::updated_at.eq(now),
                        ))
                        .execute(conn)
                        .ok();
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
