use crate::db::DbPool;
use crate::models::{AnimeLink, AnimeSeries, Download, ModuleTypeEnum, ServiceModule};
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
                tracing::warn!(
                    "No viewer module available for download {}",
                    download.download_id
                );
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

        if response.status() == reqwest::StatusCode::ACCEPTED || response.status().is_success() {
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
        conn: &mut diesel::PgConnection,
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

    /// Retry syncing completed downloads that have no viewer yet.
    /// Called when a new viewer registers to process backlogged downloads.
    pub async fn retry_completed_downloads(&self) -> Result<usize, String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        let completed: Vec<Download> = downloads::table
            .filter(downloads::status.eq("completed"))
            .filter(downloads::file_path.is_not_null())
            .filter(downloads::sync_retry_count.lt(3))
            .load::<Download>(&mut conn)
            .map_err(|e| format!("Failed to query completed downloads: {}", e))?;

        if completed.is_empty() {
            return Ok(0);
        }

        tracing::info!(
            "Found {} completed downloads pending viewer sync",
            completed.len()
        );

        let mut synced = 0;
        for download in &completed {
            match self.notify_viewer(download).await {
                Ok(true) => synced += 1,
                Ok(false) => {
                    tracing::warn!("No viewer available during retry, stopping");
                    break;
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to sync download {} during retry: {}",
                        download.download_id,
                        e
                    );
                }
            }
        }

        tracing::info!(
            "Viewer registration retry: synced {}/{} completed downloads",
            synced,
            completed.len()
        );

        Ok(synced)
    }

    /// Handle sync callback from viewer
    pub fn handle_callback(
        &self,
        conn: &mut diesel::PgConnection,
        download_id: i32,
        status: &str,
        target_path: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().naive_utc();

        match status {
            "synced" => {
                diesel::update(downloads::table.filter(downloads::download_id.eq(download_id)))
                    .set((
                        downloads::status.eq("synced"),
                        downloads::file_path.eq(target_path),
                        downloads::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .map_err(|e| format!("Failed to update download: {}", e))?;

                tracing::info!(
                    "Download {} synced to {}",
                    download_id,
                    target_path.unwrap_or("unknown")
                );
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

                diesel::update(downloads::table.filter(downloads::download_id.eq(download_id)))
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
