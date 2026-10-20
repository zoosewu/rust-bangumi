use crate::db::DbPool;
use crate::models::{Anime, AnimeLink, AnimeWork, Download, ModuleTypeEnum, ServiceModule};
use crate::schema::{
    anime_cover_images, anime_links, anime_works, animes, downloads, service_modules,
    subtitle_groups,
};
use crate::services::webhook_service::{WebhookContext, WebhookService};
use diesel::prelude::*;
use shared::{ExternalId, ViewerSeason, ViewerSeries, ViewerSyncRequest};
use std::sync::Arc;

/// Download statuses in which the torrent must stay in the downloader:
/// it is still being fetched, or is waiting to be polled again after an outage.
const TORRENT_IN_USE_STATUSES: [&str; 3] = ["pending", "downloading", "downloader_error"];

/// A torrent can leave the downloader once no download sharing it still needs it.
/// Batch torrents are shared by one download per episode.
fn torrent_releasable(sibling_statuses: &[String]) -> bool {
    !sibling_statuses
        .iter()
        .any(|s| TORRENT_IN_USE_STATUSES.contains(&s.as_str()))
}

/// What `notify_viewer` did with a download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncNotice {
    /// The viewer accepted the sync request.
    Sent,
    /// The download must not be synced (its link is in conflict or not active).
    Skipped,
    /// No viewer is registered, so nothing can be synced right now.
    NoViewer,
}

/// Statuses in which a viewer is working on the download and its callback is expected.
const SYNC_IN_FLIGHT_STATUSES: [&str; 2] = ["syncing", "resyncing"];

/// A `failed` callback only counts while a sync is in flight. One that arrives later
/// comes from a duplicate request whose source file the successful one already moved,
/// and must not turn a `synced` download back into `completed`.
fn accepts_failed_callback(current_status: &str) -> bool {
    SYNC_IN_FLIGHT_STATUSES.contains(&current_status)
}

/// 失敗回報之後的狀態與重試次數：重新同步失敗時檔案仍在原處，回到 synced 且不計入重試；
/// 第一次同步失敗回到 completed 讓排程重試，三次後停在 sync_failed。
fn status_after_failed_callback(current_status: &str, retry_count: i32) -> (&'static str, i32) {
    if current_status == "resyncing" {
        return ("synced", retry_count);
    }
    let retries = retry_count + 1;
    (
        if retries >= 3 {
            "sync_failed"
        } else {
            "completed"
        },
        retries,
    )
}

/// 撤回 `notify_viewer_resync` 的搶占（確定 viewer 沒收到請求時）。
fn release_resync_claim(conn: &mut diesel::PgConnection, download_id: i32) {
    if let Err(e) = diesel::update(
        downloads::table
            .filter(downloads::download_id.eq(download_id))
            .filter(downloads::status.eq("resyncing")),
    )
    .set((
        downloads::status.eq("synced"),
        downloads::updated_at.eq(chrono::Utc::now().naive_utc()),
    ))
    .execute(conn)
    {
        tracing::warn!(
            "Failed to release resync claim for download {}: {}",
            download_id,
            e
        );
    }
}

/// Undo `notify_viewer`'s claim when the viewer is known not to have taken the request.
fn release_sync_claim(conn: &mut diesel::PgConnection, download_id: i32) {
    let released = diesel::update(
        downloads::table
            .filter(downloads::download_id.eq(download_id))
            .filter(downloads::status.eq("syncing")),
    )
    .set((
        downloads::status.eq("completed"),
        downloads::updated_at.eq(chrono::Utc::now().naive_utc()),
    ))
    .execute(conn);
    if let Err(e) = released {
        tracing::warn!(
            "Failed to release sync claim for download {}: {}",
            download_id,
            e
        );
    }
}

/// 送給 viewer 的季身分、系列與季內容（spec 第 9 節）
struct ViewerContent {
    external_id: Option<ExternalId>,
    series: ViewerSeries,
    season: ViewerSeason,
}

/// 由查好的資料組出 viewer 的內容；系列名稱同時是媒體庫資料夾名
fn viewer_content(
    work: &AnimeWork,
    season: &Anime,
    bgm_id: Option<String>,
    series_poster: Option<String>,
    season_poster: Option<String>,
) -> ViewerContent {
    ViewerContent {
        external_id: bgm_id.map(|id| ExternalId::new("bgm", id)),
        series: ViewerSeries {
            title: work.title.clone(),
            summary: work.description.clone(),
            premiered: work.premiered.map(|d| d.to_string()),
            poster_url: series_poster,
        },
        season: ViewerSeason {
            title: season.title.clone(),
            original_title: season.original_title.clone(),
            summary: season.description.clone(),
            premiered: season.aired_date.map(|d| d.to_string()),
            poster_url: season_poster,
        },
    }
}

/// 查出一季要送給 viewer 的內容。系列海報優先用系列層級（`anime_id IS NULL`）的封面，
/// 沒有時用作品的預設封面；季海報只用該季自己的封面。
fn load_viewer_content(
    conn: &mut diesel::PgConnection,
    season: &Anime,
) -> Result<(AnimeWork, ViewerContent), String> {
    let work: AnimeWork = anime_works::table
        .find(season.work_id)
        .first(conn)
        .map_err(|e| format!("Failed to find anime work {}: {}", season.work_id, e))?;
    let bgm_id = crate::services::identity::find_external_id(conn, season.anime_id, "bgm")
        .map_err(|e| {
            format!(
                "Failed to find identity of anime {}: {}",
                season.anime_id, e
            )
        })?;
    let series_poster: Option<String> = anime_cover_images::table
        .filter(anime_cover_images::work_id.eq(season.work_id))
        .filter(
            anime_cover_images::anime_id
                .is_null()
                .or(anime_cover_images::is_default.eq(true)),
        )
        .order((
            anime_cover_images::anime_id.is_null().desc(),
            anime_cover_images::is_default.desc(),
            anime_cover_images::cover_id.asc(),
        ))
        .select(anime_cover_images::image_url)
        .first(conn)
        .optional()
        .map_err(|e| e.to_string())?;
    let season_poster: Option<String> = anime_cover_images::table
        .filter(anime_cover_images::anime_id.eq(season.anime_id))
        .order(anime_cover_images::cover_id.asc())
        .select(anime_cover_images::image_url)
        .first(conn)
        .optional()
        .map_err(|e| e.to_string())?;
    let content = viewer_content(&work, season, bgm_id, series_poster, season_poster);
    Ok((work, content))
}

pub struct SyncService {
    db_pool: DbPool,
    http_client: reqwest::Client,
    core_service_url: String,
    webhook_service: Arc<WebhookService>,
}

impl SyncService {
    pub fn new(db_pool: DbPool, webhook_service: Arc<WebhookService>) -> Self {
        let core_service_url = std::env::var("CORE_SERVICE_URL")
            .unwrap_or_else(|_| "http://core-service:8000".to_string());
        Self {
            db_pool,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
            core_service_url,
            webhook_service,
        }
    }

    /// Notify viewer of a completed download.
    pub async fn notify_viewer(&self, download: &Download) -> Result<SyncNotice, String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        // Check if the associated anime_link is in conflict or resolved
        let link: AnimeLink = anime_links::table
            .filter(anime_links::link_id.eq(download.link_id))
            .first::<AnimeLink>(&mut conn)
            .map_err(|e| format!("Failed to find anime link {}: {}", download.link_id, e))?;

        if link.conflict_flag {
            tracing::info!(
                "Skipping sync for download {} - link {} has conflict_flag=true",
                download.download_id,
                link.link_id
            );
            return Ok(SyncNotice::Skipped);
        }

        if link.link_status != "active" {
            tracing::info!(
                "Skipping sync for download {} - link {} has link_status={}",
                download.download_id,
                link.link_id,
                link.link_status
            );
            return Ok(SyncNotice::Skipped);
        }

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
                return Ok(SyncNotice::NoViewer);
            }
        };

        // Build the sync request by joining anime metadata
        let sync_request = self.build_sync_request(&mut conn, download)?;

        // Claim the download before sending. The caller's list may be stale (a callback
        // can land between reading it and getting here) and a slow viewer used to get the
        // same sync several times; only the request that moves it out of `completed` is sent.
        let claimed = diesel::update(
            downloads::table
                .filter(downloads::download_id.eq(download.download_id))
                .filter(downloads::status.eq("completed")),
        )
        .set((
            downloads::status.eq("syncing"),
            downloads::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to claim download for sync: {}", e))?;
        if claimed == 0 {
            tracing::info!(
                "Skipping sync for download {} - no longer completed",
                download.download_id
            );
            return Ok(SyncNotice::Skipped);
        }

        let sync_url = format!("{}/sync", viewer.base_url);
        tracing::info!(
            "Notifying viewer {} for download {} at {}",
            viewer.name,
            download.download_id,
            sync_url
        );

        // Send the notification
        let response = match self
            .http_client
            .post(&sync_url)
            .json(&sync_request)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                // A timeout does not mean the viewer missed it (a busy viewer answers late),
                // so the claim stays and `recover_stale_syncing` re-queues it if no callback
                // ever comes. Only a refused connection is known not to have arrived.
                if e.is_connect() {
                    release_sync_claim(&mut conn, download.download_id);
                }
                return Err(format!("Failed to notify viewer: {}", e));
            }
        };

        if response.status() == reqwest::StatusCode::ACCEPTED || response.status().is_success() {
            // Fire webhooks fire-and-forget
            let webhook_ctx = WebhookContext {
                download_id: sync_request.download_id,
                anime_id: sync_request.series_id,
                anime_title: sync_request.anime_title.clone(),
                episode_no: sync_request.episode_no,
                series_no: sync_request.series_no,
                subtitle_group: sync_request.subtitle_group.clone(),
                video_path: sync_request.video_path.clone(),
            };
            let wh_service = self.webhook_service.clone();
            tokio::spawn(async move {
                wh_service.fire(webhook_ctx).await;
            });

            Ok(SyncNotice::Sent)
        } else {
            release_sync_claim(&mut conn, download.download_id);
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

        // Get anime (formerly anime_series)
        let series: Anime = animes::table
            .filter(animes::anime_id.eq(link.anime_id))
            .first::<Anime>(conn)
            .map_err(|e| format!("Failed to find anime {}: {}", link.anime_id, e))?;

        let (work, content) = load_viewer_content(conn, &series)?;

        // Get subtitle group name
        let subtitle_group: String = subtitle_groups::table
            .filter(subtitle_groups::group_id.eq(link.group_id))
            .select(subtitle_groups::group_name)
            .first::<String>(conn)
            .map_err(|e| format!("Failed to find subtitle group {}: {}", link.group_id, e))?;

        let callback_url = format!("{}/sync-callback", self.core_service_url);

        Ok(ViewerSyncRequest {
            download_id: download.download_id,
            series_id: link.anime_id,
            anime_title: work.title,
            series_no: series.series_no,
            episode_no: link.episode_no,
            subtitle_group,
            video_path: download
                .video_file
                .clone()
                .unwrap_or_else(|| download.file_path.clone().unwrap_or_default()),
            subtitle_paths: download
                .subtitle_files
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default(),
            callback_url,
            external_id: content.external_id,
            series: Some(content.series),
            season: Some(content.season),
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
                Ok(SyncNotice::Sent) => synced += 1,
                // A conflicted download must not stop the others from syncing
                Ok(SyncNotice::Skipped) => {}
                Ok(SyncNotice::NoViewer) => {
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

    /// Reset `sync_failed` downloads back to `completed` so they can be retried.
    /// Called when a viewer registers/recovers, before `retry_completed_downloads()`.
    pub fn retry_sync_failed_downloads(&self) -> Result<usize, String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        let now = chrono::Utc::now().naive_utc();
        let updated = diesel::update(downloads::table.filter(downloads::status.eq("sync_failed")))
            .set((
                downloads::status.eq("completed"),
                downloads::sync_retry_count.eq(0),
                downloads::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to reset sync_failed downloads: {}", e))?;

        if updated > 0 {
            tracing::info!(
                "Service Viewer recovered: retrying {} sync_failed tasks",
                updated
            );
        }

        Ok(updated)
    }

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

        // 先搶占再送出：viewer 回報得再快，也不會被之後的狀態更新蓋掉
        let claimed = diesel::update(
            downloads::table
                .filter(downloads::download_id.eq(download.download_id))
                .filter(downloads::status.eq("synced")),
        )
        .set((
            downloads::status.eq("resyncing"),
            downloads::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to claim download for resync: {}", e))?;
        if claimed == 0 {
            return Err(format!(
                "Download {} is not synced, not resyncing it",
                download.download_id
            ));
        }

        let resync_url = format!("{}/resync", viewer.base_url);
        tracing::info!(
            "Notifying viewer {} for resync download {} at {}",
            viewer.name,
            download.download_id,
            resync_url
        );

        let response = match self
            .http_client
            .post(&resync_url)
            .json(&resync_request)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                // 連不上就確定 viewer 沒收到；逾時則可能已在處理，交給回報或逾時回收
                if e.is_connect() {
                    release_resync_claim(&mut conn, download.download_id);
                }
                return Err(format!("Failed to notify viewer for resync: {}", e));
            }
        };

        if response.status() == reqwest::StatusCode::ACCEPTED || response.status().is_success() {
            Ok(true)
        } else {
            release_resync_claim(&mut conn, download.download_id);
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

        let series: Anime = animes::table
            .filter(animes::anime_id.eq(link.anime_id))
            .first::<Anime>(conn)
            .map_err(|e| format!("Failed to find anime {}: {}", link.anime_id, e))?;

        let (work, content) = load_viewer_content(conn, &series)?;

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
            series_id: link.anime_id,
            anime_title: work.title,
            series_no: series.series_no,
            episode_no: link.episode_no,
            subtitle_group,
            old_target_path,
            callback_url,
            external_id: content.external_id,
            series: Some(content.series),
            season: Some(content.season),
        })
    }

    /// Notify viewer to delete synced files (best-effort).
    pub async fn notify_viewer_delete(
        &self,
        download_ids: Vec<i32>,
    ) -> Result<shared::ViewerDeleteResponse, String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

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
                tracing::warn!("No viewer module available for delete notification");
                return Ok(shared::ViewerDeleteResponse { deleted: vec![] });
            }
        };

        // 帶上 core 記錄的媒體庫路徑，viewer 的記錄和它不一致時不刪
        let expected_paths: std::collections::HashMap<i32, String> = downloads::table
            .filter(downloads::download_id.eq_any(&download_ids))
            .filter(downloads::file_path.is_not_null())
            .select((
                downloads::download_id,
                downloads::file_path.assume_not_null(),
            ))
            .load::<(i32, String)>(&mut conn)
            .map_err(|e| format!("Failed to load download paths: {}", e))?
            .into_iter()
            .collect();

        let delete_url = format!("{}/delete", viewer.base_url);
        let request = shared::ViewerDeleteRequest {
            download_ids,
            expected_paths,
        };

        let response = self
            .http_client
            .post(&delete_url)
            .json(&request)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("Failed to notify viewer for delete: {}", e))?;

        if response.status().is_success() {
            response
                .json::<shared::ViewerDeleteResponse>()
                .await
                .map_err(|e| format!("Failed to parse viewer delete response: {}", e))
        } else {
            Err(format!(
                "Viewer returned status {} for delete",
                response.status()
            ))
        }
    }

    /// Remove a synced download's torrent from its downloader, keeping the files.
    ///
    /// The viewer moves media out of the download directory, so a torrent left
    /// behind turns into "missingFiles" and can re-download from scratch.
    /// Skipped while another download sharing the torrent still needs it.
    /// Returns Ok(true) if the downloader removed the torrent.
    pub async fn release_torrent_if_settled(&self, download_id: i32) -> Result<bool, String> {
        let (hash, base_url) = {
            let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

            let download: Download = downloads::table
                .find(download_id)
                .first::<Download>(&mut conn)
                .map_err(|e| format!("Download {} not found: {}", download_id, e))?;

            let (hash, module_id) = match (
                download.torrent_hash.filter(|h| !h.is_empty()),
                download.module_id,
            ) {
                (Some(h), Some(m)) => (h, m),
                _ => return Ok(false),
            };

            let sibling_statuses: Vec<String> = downloads::table
                .filter(downloads::torrent_hash.eq(&hash))
                .filter(downloads::module_id.eq(module_id))
                .select(downloads::status)
                .load::<String>(&mut conn)
                .map_err(|e| format!("Failed to query downloads for torrent {}: {}", hash, e))?;

            if !torrent_releasable(&sibling_statuses) {
                tracing::debug!(
                    "Keeping torrent {} — other downloads still depend on it",
                    hash
                );
                return Ok(false);
            }

            let base_url = service_modules::table
                .filter(service_modules::module_id.eq(module_id))
                .select(service_modules::base_url)
                .first::<String>(&mut conn)
                .optional()
                .map_err(|e| format!("Failed to query downloader {}: {}", module_id, e))?;

            match base_url {
                Some(url) => (hash, url),
                None => return Ok(false),
            }
        };

        let url = format!("{}/downloads/{}?delete_files=false", base_url, hash);
        let response = self
            .http_client
            .delete(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to remove torrent {}: {}", hash, e))?;

        if response.status().is_success() {
            tracing::info!("Removed torrent {} from downloader after sync", hash);
            Ok(true)
        } else {
            Err(format!(
                "Downloader returned status {} removing torrent {}",
                response.status(),
                hash
            ))
        }
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

                if !accepts_failed_callback(&download.status) {
                    tracing::info!(
                        "Ignoring failed sync callback for download {} in status {}: {}",
                        download_id,
                        download.status,
                        error_message.unwrap_or("unknown")
                    );
                    return Ok(());
                }

                let (new_status, new_retry_count) =
                    status_after_failed_callback(&download.status, download.sync_retry_count);

                // Re-check the status: a `synced` callback may have landed since the read
                diesel::update(
                    downloads::table
                        .filter(downloads::download_id.eq(download_id))
                        .filter(downloads::status.eq_any(SYNC_IN_FLIGHT_STATUSES)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_resync_leaves_the_episode_synced() {
        assert_eq!(status_after_failed_callback("resyncing", 1), ("synced", 1));
    }

    #[test]
    fn a_failed_first_sync_is_retried_then_given_up() {
        assert_eq!(status_after_failed_callback("syncing", 0), ("completed", 1));
        assert_eq!(
            status_after_failed_callback("syncing", 2),
            ("sync_failed", 3)
        );
    }

    fn work_and_season() -> (AnimeWork, Anime) {
        let now = chrono::Utc::now().naive_utc();
        let work = AnimeWork {
            work_id: 16,
            title: "金牌得主".to_string(),
            created_at: now,
            updated_at: now,
            series_namespace: Some("bgm".to_string()),
            series_external_id: Some("430699".to_string()),
            title_source: "bgm".to_string(),
            description: Some("系列簡介".to_string()),
            premiered: chrono::NaiveDate::from_ymd_opt(2025, 1, 4),
            is_active: true,
            soft_deleted_at: None,
        };
        let season = Anime {
            anime_id: 16,
            work_id: 16,
            series_no: 2,
            season_id: 1,
            description: Some("第二季簡介".to_string()),
            aired_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 24),
            end_date: None,
            created_at: now,
            updated_at: now,
            title: Some("金牌得主 第二季".to_string()),
            original_title: Some("メダリスト 第2期".to_string()),
        };
        (work, season)
    }

    #[test]
    fn viewer_content_carries_the_identity_series_and_season() {
        let (work, season) = work_and_season();
        let content = viewer_content(
            &work,
            &season,
            Some("548818".to_string()),
            Some("https://img/s1.jpg".to_string()),
            Some("https://img/s2.jpg".to_string()),
        );
        assert_eq!(content.external_id, Some(ExternalId::new("bgm", "548818")));
        assert_eq!(
            content.series,
            ViewerSeries {
                title: "金牌得主".to_string(),
                summary: Some("系列簡介".to_string()),
                premiered: Some("2025-01-04".to_string()),
                poster_url: Some("https://img/s1.jpg".to_string()),
            }
        );
        assert_eq!(content.season.title.as_deref(), Some("金牌得主 第二季"));
        assert_eq!(content.season.premiered.as_deref(), Some("2026-01-24"));
        assert_eq!(
            content.season.poster_url.as_deref(),
            Some("https://img/s2.jpg")
        );
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn viewer_content_is_loaded_from_the_bound_season() {
        use crate::services::identity::{
            resolve_season, save_metadata_namespaces, test_db, IdentityContext, SeasonResolution,
        };
        let mut conn = test_db::conn();
        test_db::insert_module(&mut conn, "meta-bgm", true);
        save_metadata_namespaces(&mut conn, "meta-bgm", &["bgm".to_string()]).unwrap();
        let ids = vec![ExternalId::new("bgm", "548818")];
        let context =
            IdentityContext::prepare(&mut conn, &test_db::lookup(), std::slice::from_ref(&ids))
                .await
                .unwrap();
        let SeasonResolution::Resolved(season) =
            resolve_season(&mut conn, &context, &ids, 2).unwrap()
        else {
            panic!("expected a season");
        };

        let (work, content) = load_viewer_content(&mut conn, &season).unwrap();
        assert_eq!(work.title, "金牌得主");
        assert_eq!(content.external_id, Some(ExternalId::new("bgm", "548818")));
        assert_eq!(content.series.summary.as_deref(), Some("金牌得主 簡介"));
        assert_eq!(
            content.series.poster_url.as_deref(),
            Some("https://img/430699.jpg")
        );
        assert_eq!(
            content.season.poster_url.as_deref(),
            Some("https://img/548818.jpg")
        );
        assert_eq!(content.season.title.as_deref(), Some("金牌得主 第二季"));
    }

    #[test]
    fn a_season_without_identity_still_names_the_series() {
        let (work, season) = work_and_season();
        let content = viewer_content(&work, &season, None, None, None);
        assert!(content.external_id.is_none());
        assert_eq!(content.series.title, "金牌得主");
    }

    fn statuses(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn single_synced_download_releases_torrent() {
        assert!(torrent_releasable(&statuses(&["synced"])));
    }

    #[test]
    fn batch_waits_for_episodes_still_downloading() {
        assert!(!torrent_releasable(&statuses(&["synced", "downloading"])));
        assert!(!torrent_releasable(&statuses(&["synced", "pending"])));
        assert!(!torrent_releasable(&statuses(&[
            "synced",
            "downloader_error"
        ])));
    }

    #[test]
    fn batch_releases_once_no_episode_needs_downloader() {
        // completed / syncing / batch_unmatched work from files on disk, which are kept
        assert!(torrent_releasable(&statuses(&[
            "synced",
            "completed",
            "syncing",
            "batch_unmatched",
            "batch_failed",
            "sync_failed",
        ])));
    }

    #[test]
    fn failed_callback_counts_only_while_sync_in_flight() {
        assert!(accepts_failed_callback("syncing"));
        assert!(accepts_failed_callback("resyncing"));
    }

    #[test]
    fn late_failed_callback_cannot_undo_a_finished_sync() {
        // E6: a duplicate request fails after the first one already synced the file
        for status in ["synced", "completed", "sync_failed", "cancelled"] {
            assert!(!accepts_failed_callback(status), "{status}");
        }
    }
}
