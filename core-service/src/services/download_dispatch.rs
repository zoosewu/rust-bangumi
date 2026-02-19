use crate::db::DbPool;
use crate::models::{AnimeLink, Download, DownloaderCapability, NewDownload, ServiceModule};
use crate::schema::{anime_links, downloader_capabilities, downloads, service_modules};
use chrono::Utc;
use diesel::prelude::*;
use shared::{BatchDownloadRequest, BatchDownloadResponse, DownloadRequestItem};
use std::collections::HashMap;

pub struct DownloadDispatchService {
    db_pool: DbPool,
    http_client: reqwest::Client,
}

pub struct DispatchResult {
    pub dispatched: usize,
    pub no_downloader: usize,
    pub failed: usize,
}

impl DownloadDispatchService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            http_client: reqwest::Client::new(),
        }
    }

    /// Dispatch a batch of new anime_links to appropriate downloaders.
    /// Called after fetcher results are fully processed.
    pub async fn dispatch_new_links(&self, link_ids: Vec<i32>) -> Result<DispatchResult, String> {
        if link_ids.is_empty() {
            return Ok(DispatchResult {
                dispatched: 0,
                no_downloader: 0,
                failed: 0,
            });
        }

        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        // Load the anime_links for these IDs (only active, unfiltered and non-conflicted)
        let links: Vec<AnimeLink> = anime_links::table
            .filter(anime_links::link_id.eq_any(&link_ids))
            .filter(anime_links::link_status.eq("active"))
            .filter(anime_links::filtered_flag.eq(false))
            .filter(anime_links::conflict_flag.eq(false))
            .load::<AnimeLink>(&mut conn)
            .map_err(|e| format!("Failed to load anime links: {}", e))?;

        if links.is_empty() {
            return Ok(DispatchResult {
                dispatched: 0,
                no_downloader: 0,
                failed: 0,
            });
        }

        // Skip links that already have an active download
        let candidate_link_ids: Vec<i32> = links.iter().map(|l| l.link_id).collect();
        let links_with_active_downloads: Vec<i32> = downloads::table
            .filter(downloads::link_id.eq_any(&candidate_link_ids))
            .filter(downloads::status.eq_any(&["downloading", "completed", "syncing", "synced"]))
            .select(downloads::link_id)
            .distinct()
            .load::<i32>(&mut conn)
            .map_err(|e| format!("Failed to check active downloads: {}", e))?;

        let links: Vec<AnimeLink> = if links_with_active_downloads.is_empty() {
            links
        } else {
            tracing::info!(
                "dispatch: skipping {} links with active downloads",
                links_with_active_downloads.len()
            );
            links
                .into_iter()
                .filter(|l| !links_with_active_downloads.contains(&l.link_id))
                .collect()
        };

        if links.is_empty() {
            return Ok(DispatchResult {
                dispatched: 0,
                no_downloader: 0,
                failed: 0,
            });
        }

        // Group links by download_type
        let mut groups: HashMap<String, Vec<&AnimeLink>> = HashMap::new();
        for link in &links {
            let dt = link
                .download_type
                .clone()
                .unwrap_or_else(|| "http".to_string());
            groups.entry(dt).or_default().push(link);
        }

        let mut total_dispatched = 0;
        let mut total_no_downloader = 0;
        let mut total_failed = 0;

        for (download_type, type_links) in groups {
            // Find capable downloaders sorted by priority DESC
            let downloaders = self.find_capable_downloaders(&mut conn, &download_type)?;

            if downloaders.is_empty() {
                // No downloader supports this type — mark all as no_downloader
                for link in &type_links {
                    self.create_download_record(
                        &mut conn,
                        link.link_id,
                        &download_type,
                        "no_downloader",
                        None,
                        None,
                    )?;
                }
                total_no_downloader += type_links.len();
                tracing::warn!(
                    "No downloader available for type '{}': {} links marked as no_downloader",
                    download_type,
                    type_links.len()
                );
                continue;
            }

            // Cascade through downloaders
            let mut pending_links: Vec<&AnimeLink> = type_links;

            for downloader in &downloaders {
                if pending_links.is_empty() {
                    break;
                }

                let items: Vec<DownloadRequestItem> = pending_links
                    .iter()
                    .map(|link| DownloadRequestItem {
                        url: link.url.clone(),
                        save_path: "/downloads".to_string(),
                    })
                    .collect();

                let download_url = format!("{}/downloads", downloader.base_url);
                match self.send_batch_to_downloader(&download_url, items).await {
                    Ok(response) => {
                        let mut rejected = Vec::new();

                        for (i, result) in response.results.iter().enumerate() {
                            if i >= pending_links.len() {
                                break;
                            }
                            let link = pending_links[i];

                            if result.status == "accepted" {
                                self.create_download_record(
                                    &mut conn,
                                    link.link_id,
                                    &download_type,
                                    "downloading",
                                    Some(downloader.module_id),
                                    result.hash.as_deref(),
                                )?;
                                total_dispatched += 1;
                            } else {
                                rejected.push(link);
                            }
                        }

                        pending_links = rejected;
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to send batch to downloader {} ({}): {}",
                            downloader.name,
                            downloader.base_url,
                            e
                        );
                        // Network error: try next downloader
                        continue;
                    }
                }
            }

            // Remaining links after all downloaders tried
            for link in &pending_links {
                self.create_download_record(
                    &mut conn,
                    link.link_id,
                    &download_type,
                    "failed",
                    None,
                    None,
                )?;
                total_failed += 1;
            }
        }

        Ok(DispatchResult {
            dispatched: total_dispatched,
            no_downloader: total_no_downloader,
            failed: total_failed,
        })
    }

    /// Retry dispatching links that have no_downloader status for given download types.
    /// Called when a new downloader registers.
    pub async fn retry_no_downloader_links(
        &self,
        download_types: &[String],
    ) -> Result<usize, String> {
        if download_types.is_empty() {
            return Ok(0);
        }

        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        // Find downloads with no_downloader status that match the new types
        let no_dl_records: Vec<Download> = downloads::table
            .filter(downloads::status.eq("no_downloader"))
            .filter(downloads::downloader_type.eq_any(download_types))
            .load::<Download>(&mut conn)
            .map_err(|e| format!("Failed to query no_downloader records: {}", e))?;

        if no_dl_records.is_empty() {
            return Ok(0);
        }

        let link_ids: Vec<i32> = no_dl_records.iter().map(|d| d.link_id).collect();
        let download_ids: Vec<i32> = no_dl_records.iter().map(|d| d.download_id).collect();

        // Delete old no_downloader records
        diesel::delete(downloads::table.filter(downloads::download_id.eq_any(&download_ids)))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to delete no_downloader records: {}", e))?;

        // Re-dispatch
        let result = self.dispatch_new_links(link_ids).await?;

        tracing::info!(
            "Retried no_downloader links: {} dispatched, {} still no_downloader",
            result.dispatched,
            result.no_downloader
        );

        Ok(result.dispatched)
    }

    fn find_capable_downloaders(
        &self,
        conn: &mut PgConnection,
        download_type: &str,
    ) -> Result<Vec<ServiceModule>, String> {
        service_modules::table
            .inner_join(
                downloader_capabilities::table
                    .on(downloader_capabilities::module_id.eq(service_modules::module_id)),
            )
            .filter(downloader_capabilities::download_type.eq(download_type))
            .filter(service_modules::is_enabled.eq(true))
            .order(service_modules::priority.desc())
            .select(ServiceModule::as_select())
            .load::<ServiceModule>(conn)
            .map_err(|e| format!("Failed to find capable downloaders: {}", e))
    }

    async fn send_batch_to_downloader(
        &self,
        url: &str,
        items: Vec<DownloadRequestItem>,
    ) -> Result<BatchDownloadResponse, String> {
        let request = BatchDownloadRequest { items };

        let response = self
            .http_client
            .post(url)
            .json(&request)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Downloader returned status: {}", response.status()));
        }

        response
            .json::<BatchDownloadResponse>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    fn create_download_record(
        &self,
        conn: &mut PgConnection,
        link_id: i32,
        downloader_type: &str,
        status: &str,
        module_id: Option<i32>,
        torrent_hash: Option<&str>,
    ) -> Result<(), String> {
        let now = Utc::now().naive_utc();
        let new_download = NewDownload {
            link_id,
            downloader_type: downloader_type.to_string(),
            status: status.to_string(),
            created_at: now,
            updated_at: now,
            module_id,
            torrent_hash: torrent_hash.map(|h| h.to_string()),
        };

        diesel::insert_into(downloads::table)
            .values(&new_download)
            .execute(conn)
            .map_err(|e| format!("Failed to create download record: {}", e))?;

        Ok(())
    }
}
