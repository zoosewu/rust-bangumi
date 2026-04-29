use crate::db::DbPool;
use crate::models::{AnimeLink, Download, DownloaderCapability, NewDownload, ServiceModule};
use crate::schema::{anime_links, downloader_capabilities, downloads, raw_anime_items, service_modules, subscriptions};
use chrono::Utc;
use diesel::prelude::*;
use shared::{BatchDownloadRequest, BatchDownloadResponse, DownloadRequestItem};
use std::collections::{HashMap, HashSet};

/// Download statuses that allow re-dispatching the link.
/// `dispatch_new_links` treats other statuses as "active" and skips the link.
/// Manual retry uses the same set as the input gate.
pub const RETRYABLE_STATUSES: &[&str] = &[
    "cancelled",
    "failed",
    "no_downloader",
    "downloader_error",
];

/// Group a slice of links by URL, preserving insertion order.
/// Each unique URL maps to all links sharing that URL.
fn group_links_by_url<'a>(links: &[&'a AnimeLink]) -> Vec<(String, Vec<&'a AnimeLink>)> {
    let mut groups: Vec<(String, Vec<&'a AnimeLink>)> = Vec::new();
    for link in links {
        if let Some(g) = groups.iter_mut().find(|(url, _)| url == &link.url) {
            g.1.push(link);
        } else {
            groups.push((link.url.clone(), vec![link]));
        }
    }
    groups
}

/// Split a slice of downloads into (retryable references, count of non-retryable).
/// Pure function — no DB / IO.
pub fn partition_retryable(downloads: &[Download]) -> (Vec<&Download>, usize) {
    let retryable: Vec<&Download> = downloads
        .iter()
        .filter(|d| RETRYABLE_STATUSES.contains(&d.status.as_str()))
        .collect();
    let not_retryable = downloads.len() - retryable.len();
    (retryable, not_retryable)
}

pub struct DownloadDispatchService {
    db_pool: DbPool,
    http_client: reqwest::Client,
}

pub struct DispatchResult {
    pub dispatched: usize,
    pub no_downloader: usize,
    pub failed: usize,
}

#[derive(Debug, Clone)]
pub struct RetryResult {
    pub downloads_matched: usize,
    pub not_retryable: usize,
    pub unique_links: usize,
    pub dispatched: usize,
    pub no_downloader: usize,
    pub conflict_or_filtered: usize,
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

        // Batch-level conflict propagation:
        // 同一個 raw_item 通常對應一個 torrent (e.g. [01-02] batch). 任何兄弟 link
        // 仍在 conflict_flag=true 時，整批暫不派送，避免下載被衝突的 episode 檔案。
        let raw_ids_with_conflicts: HashSet<i32> = anime_links::table
            .filter(anime_links::conflict_flag.eq(true))
            .filter(anime_links::link_status.eq("active"))
            .filter(anime_links::raw_item_id.is_not_null())
            .select(anime_links::raw_item_id.assume_not_null())
            .distinct()
            .load::<i32>(&mut conn)
            .map_err(|e| format!("Failed to query conflicting raw_item_ids: {}", e))?
            .into_iter()
            .collect();

        let links: Vec<AnimeLink> = if raw_ids_with_conflicts.is_empty() {
            links
        } else {
            let before = links.len();
            let filtered: Vec<AnimeLink> = links
                .into_iter()
                .filter(|l| {
                    l.raw_item_id
                        .map(|r| !raw_ids_with_conflicts.contains(&r))
                        .unwrap_or(true)
                })
                .collect();
            let skipped = before - filtered.len();
            if skipped > 0 {
                tracing::info!(
                    "dispatch: skipping {} links from raw_items with sibling conflicts",
                    skipped
                );
            }
            filtered
        };

        if links.is_empty() {
            return Ok(DispatchResult {
                dispatched: 0,
                no_downloader: 0,
                failed: 0,
            });
        }

        // Skip links that already have a non-terminal download.
        // Only these terminal-failure statuses allow re-dispatch:
        let candidate_link_ids: Vec<i32> = links.iter().map(|l| l.link_id).collect();
        let links_with_active_downloads: Vec<i32> = downloads::table
            .filter(downloads::link_id.eq_any(&candidate_link_ids))
            .filter(downloads::status.ne_all(RETRYABLE_STATUSES))
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

        // Build map: link_id → preferred_downloader_id (from subscription via raw_anime_items)
        let surviving_link_ids: Vec<i32> = links.iter().map(|l| l.link_id).collect();
        let link_preferred_map: HashMap<i32, i32> = {
            let result: Vec<(i32, i32)> = anime_links::table
                .inner_join(
                    raw_anime_items::table.on(
                        anime_links::raw_item_id.eq(raw_anime_items::item_id.nullable())
                    )
                )
                .inner_join(
                    subscriptions::table.on(
                        subscriptions::subscription_id.eq(raw_anime_items::subscription_id)
                    )
                )
                .filter(anime_links::link_id.eq_any(&surviving_link_ids))
                .filter(subscriptions::preferred_downloader_id.is_not_null())
                .select((
                    anime_links::link_id,
                    subscriptions::preferred_downloader_id.assume_not_null(),
                ))
                .load::<(i32, i32)>(&mut conn)
                .unwrap_or_default();
            result.into_iter().collect()
        };

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

            // Build capable downloader ID set for fast lookup
            let capable_ids: std::collections::HashSet<i32> =
                downloaders.iter().map(|d| d.module_id).collect();

            // Phase 1: Split into preferred groups and cascade_pending
            let mut cascade_pending: Vec<&AnimeLink> = Vec::new();
            let mut by_preferred: HashMap<i32, Vec<&AnimeLink>> = HashMap::new();

            for link in &type_links {
                match link_preferred_map.get(&link.link_id) {
                    Some(&pref_id) if capable_ids.contains(&pref_id) => {
                        by_preferred.entry(pref_id).or_default().push(link);
                    }
                    _ => cascade_pending.push(link),
                }
            }

            // Dispatch each preferred downloader group
            for (pref_id, pref_links) in &by_preferred {
                let pref_dl = downloaders.iter().find(|d| d.module_id == *pref_id).unwrap();
                let url_groups = group_links_by_url(pref_links);
                let items: Vec<DownloadRequestItem> = url_groups
                    .iter()
                    .map(|(url, _)| DownloadRequestItem {
                        url: url.clone(),
                        save_path: "/downloads".to_string(),
                    })
                    .collect();

                let download_url = format!("{}/downloads", pref_dl.base_url);
                match self.send_batch_to_downloader(&download_url, items).await {
                    Ok(response) => {
                        for (i, result) in response.results.iter().enumerate() {
                            if i >= url_groups.len() {
                                break;
                            }
                            let (_, links_for_url) = &url_groups[i];
                            for link in links_for_url {
                                if result.status == "accepted" {
                                    self.create_download_record(
                                        &mut conn,
                                        link.link_id,
                                        &download_type,
                                        "downloading",
                                        Some(pref_dl.module_id),
                                        result.hash.as_deref(),
                                    )?;
                                    total_dispatched += 1;
                                } else {
                                    // rejected by preferred — fallback to cascade
                                    cascade_pending.push(link);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Preferred downloader {} failed for {} links: {}",
                            pref_dl.name,
                            pref_links.len(),
                            e
                        );
                        cascade_pending.extend(pref_links.iter().copied());
                    }
                }
            }

            // Phase 2: Regular cascade for remaining links
            let mut pending_links: Vec<&AnimeLink> = cascade_pending;

            for downloader in &downloaders {
                if pending_links.is_empty() {
                    break;
                }

                let url_groups = group_links_by_url(&pending_links);
                let items: Vec<DownloadRequestItem> = url_groups
                    .iter()
                    .map(|(url, _)| DownloadRequestItem {
                        url: url.clone(),
                        save_path: "/downloads".to_string(),
                    })
                    .collect();

                let download_url = format!("{}/downloads", downloader.base_url);
                match self.send_batch_to_downloader(&download_url, items).await {
                    Ok(response) => {
                        let mut rejected = Vec::new();

                        for (i, result) in response.results.iter().enumerate() {
                            if i >= url_groups.len() {
                                break;
                            }
                            let (_, links_for_url) = &url_groups[i];
                            for link in links_for_url {
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
                                    rejected.push(*link);
                                }
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

    /// Manually retry the given downloads.
    ///
    /// Loads each download, keeps only those in `RETRYABLE_STATUSES`, dedups
    /// link_ids, and calls `dispatch_new_links`. The existing dispatch logic
    /// inserts a NEW download row per accepted link, preserving history.
    ///
    /// Counts in `RetryResult`:
    /// - `downloads_matched`: downloads found by id (input length minus missing)
    /// - `not_retryable`: matched downloads whose status is not retryable
    /// - `unique_links`: deduplicated link_ids fed into dispatch
    /// - `dispatched / no_downloader / failed`: forwarded from `DispatchResult`
    /// - `conflict_or_filtered`: `unique_links - dispatched - no_downloader - failed`
    ///   (links dropped by dispatch's conflict / filter / link_status / batch-conflict gates)
    pub async fn manual_retry(
        &self,
        download_ids: Vec<i32>,
    ) -> Result<RetryResult, String> {
        if download_ids.is_empty() {
            return Ok(RetryResult {
                downloads_matched: 0,
                not_retryable: 0,
                unique_links: 0,
                dispatched: 0,
                no_downloader: 0,
                conflict_or_filtered: 0,
                failed: 0,
            });
        }

        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        let matched: Vec<Download> = downloads::table
            .filter(downloads::download_id.eq_any(&download_ids))
            .load::<Download>(&mut conn)
            .map_err(|e| format!("Failed to load downloads: {}", e))?;

        let downloads_matched = matched.len();
        let (retryable, not_retryable) = partition_retryable(&matched);

        let mut seen: HashSet<i32> = HashSet::new();
        let unique_link_ids: Vec<i32> = retryable
            .iter()
            .filter_map(|d| if seen.insert(d.link_id) { Some(d.link_id) } else { None })
            .collect();
        let unique_links = unique_link_ids.len();

        if unique_link_ids.is_empty() {
            return Ok(RetryResult {
                downloads_matched,
                not_retryable,
                unique_links: 0,
                dispatched: 0,
                no_downloader: 0,
                conflict_or_filtered: 0,
                failed: 0,
            });
        }

        // Drop the connection before awaiting an async call that may need its own conn.
        drop(conn);

        let dispatch_result = self.dispatch_new_links(unique_link_ids).await?;

        let DispatchResult {
            dispatched,
            no_downloader,
            failed,
        } = dispatch_result;

        let accounted = dispatched + no_downloader + failed;
        let conflict_or_filtered = unique_links.saturating_sub(accounted);

        tracing::info!(
            "manual_retry: matched={}, not_retryable={}, unique_links={}, dispatched={}, no_downloader={}, conflict_or_filtered={}, failed={}",
            downloads_matched,
            not_retryable,
            unique_links,
            dispatched,
            no_downloader,
            conflict_or_filtered,
            failed
        );

        Ok(RetryResult {
            downloads_matched,
            not_retryable,
            unique_links,
            dispatched,
            no_downloader,
            conflict_or_filtered,
            failed,
        })
    }

    /// Retry dispatching links whose downloads ended in `failed` or `downloader_error`.
    /// Called when a downloader registers/recovers.
    pub async fn retry_failed_downloads(&self) -> Result<usize, String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        let retryable_statuses = vec!["failed", "downloader_error"];
        let failed_records: Vec<Download> = downloads::table
            .filter(downloads::status.eq_any(&retryable_statuses))
            .load::<Download>(&mut conn)
            .map_err(|e| format!("Failed to query failed downloads: {}", e))?;

        if failed_records.is_empty() {
            return Ok(0);
        }

        let link_ids: Vec<i32> = failed_records.iter().map(|d| d.link_id).collect();
        let download_ids: Vec<i32> = failed_records.iter().map(|d| d.download_id).collect();
        let count = failed_records.len();

        tracing::info!(
            "Service Downloader recovered: retrying {} failed/downloader_error tasks",
            count
        );

        // Delete old failed records
        diesel::delete(downloads::table.filter(downloads::download_id.eq_any(&download_ids)))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to delete failed download records: {}", e))?;

        // Re-dispatch
        let result = self.dispatch_new_links(link_ids).await?;

        tracing::info!(
            "Retried failed downloads: {} dispatched, {} no_downloader, {} failed again",
            result.dispatched,
            result.no_downloader,
            result.failed
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

#[cfg(test)]
mod tests {
    use super::group_links_by_url;
    use crate::models::{AnimeLink, Download};
    use chrono::Utc;

    fn make_link(id: i32, url: &str) -> AnimeLink {
        let now = Utc::now().naive_utc();
        AnimeLink {
            link_id: id,
            anime_id: 1,
            group_id: 1,
            episode_no: id,
            title: None,
            url: url.to_string(),
            source_hash: format!("hash{}", id),
            filtered_flag: false,
            created_at: now,
            raw_item_id: None,
            download_type: Some("magnet".to_string()),
            conflict_flag: false,
            link_status: "active".to_string(),
        }
    }

    fn make_download(download_id: i32, link_id: i32, status: &str) -> Download {
        let now = Utc::now().naive_utc();
        Download {
            download_id,
            link_id,
            downloader_type: "magnet".to_string(),
            status: status.to_string(),
            progress: None,
            downloaded_bytes: None,
            total_bytes: None,
            error_message: None,
            created_at: now,
            updated_at: now,
            module_id: None,
            torrent_hash: None,
            file_path: None,
            sync_retry_count: 0,
            video_file: None,
            subtitle_files: None,
        }
    }

    #[test]
    fn test_group_links_by_url_deduplicates() {
        let l1 = make_link(1, "magnet:?xt=urn:btih:ABC");
        let l2 = make_link(2, "magnet:?xt=urn:btih:ABC");
        let l3 = make_link(3, "magnet:?xt=urn:btih:DEF");
        let refs = vec![&l1, &l2, &l3];

        let groups = group_links_by_url(&refs);

        assert_eq!(groups.len(), 2, "should have 2 unique URLs");
        let abc_group = groups.iter().find(|(url, _)| url == "magnet:?xt=urn:btih:ABC").unwrap();
        assert_eq!(abc_group.1.len(), 2);
        let def_group = groups.iter().find(|(url, _)| url == "magnet:?xt=urn:btih:DEF").unwrap();
        assert_eq!(def_group.1.len(), 1);
    }

    #[test]
    fn test_group_links_by_url_preserves_order() {
        let l1 = make_link(1, "magnet:?xt=urn:btih:AAA");
        let l2 = make_link(2, "magnet:?xt=urn:btih:BBB");
        let refs = vec![&l1, &l2];

        let groups = group_links_by_url(&refs);
        assert_eq!(groups[0].0, "magnet:?xt=urn:btih:AAA");
        assert_eq!(groups[1].0, "magnet:?xt=urn:btih:BBB");
    }

    #[test]
    fn partition_retryable_keeps_retryable_statuses() {
        let downloads = vec![
            make_download(1, 10, "failed"),
            make_download(2, 20, "cancelled"),
            make_download(3, 30, "no_downloader"),
            make_download(4, 40, "downloader_error"),
        ];
        let (retryable, not_retryable) = super::partition_retryable(&downloads);
        assert_eq!(retryable.len(), 4);
        assert_eq!(not_retryable, 0);
    }

    #[test]
    fn partition_retryable_excludes_active_statuses() {
        let downloads = vec![
            make_download(1, 10, "downloading"),
            make_download(2, 20, "completed"),
            make_download(3, 30, "syncing"),
            make_download(4, 40, "failed"),
        ];
        let (retryable, not_retryable) = super::partition_retryable(&downloads);
        assert_eq!(retryable.len(), 1);
        assert_eq!(retryable[0].download_id, 4);
        assert_eq!(not_retryable, 3);
    }

    #[test]
    fn partition_retryable_handles_empty_input() {
        let (retryable, not_retryable) = super::partition_retryable(&[]);
        assert!(retryable.is_empty());
        assert_eq!(not_retryable, 0);
    }
}
