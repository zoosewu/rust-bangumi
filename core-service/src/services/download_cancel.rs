use crate::db::DbPool;
use crate::schema::{downloads, service_modules};
use diesel::prelude::*;
use shared::BatchCancelRequest;

pub struct DownloadCancelService {
    db_pool: DbPool,
    http_client: reqwest::Client,
}

impl DownloadCancelService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            http_client: reqwest::Client::new(),
        }
    }

    /// Cancel in-progress downloads for the given link IDs.
    /// Calls the downloader cancel API and marks DB records as 'cancelled'.
    /// Silently skips links with no active downloads.
    pub async fn cancel_downloads_for_links(&self, link_ids: &[i32]) -> Result<usize, String> {
        if link_ids.is_empty() {
            return Ok(0);
        }

        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        // Find active (downloading) download records for these links
        let active: Vec<(i32, Option<i32>, Option<String>)> = downloads::table
            .filter(downloads::link_id.eq_any(link_ids))
            .filter(downloads::status.eq("downloading"))
            .select((
                downloads::download_id,
                downloads::module_id,
                downloads::torrent_hash,
            ))
            .load::<(i32, Option<i32>, Option<String>)>(&mut conn)
            .map_err(|e| format!("Failed to query active downloads: {}", e))?;

        if active.is_empty() {
            return Ok(0);
        }

        // Group by module_id to batch cancel per downloader
        let mut by_module: std::collections::HashMap<i32, Vec<(i32, String)>> =
            std::collections::HashMap::new();
        let mut no_module: Vec<i32> = Vec::new();

        for (download_id, module_id, torrent_hash) in &active {
            match (module_id, torrent_hash.as_deref()) {
                (Some(mid), Some(hash)) => {
                    by_module
                        .entry(*mid)
                        .or_default()
                        .push((*download_id, hash.to_string()));
                }
                _ => no_module.push(*download_id),
            }
        }

        let mut cancelled = 0;

        // Cancel per downloader
        for (module_id, items) in &by_module {
            let base_url: Option<String> = service_modules::table
                .filter(service_modules::module_id.eq(module_id))
                .select(service_modules::base_url)
                .first::<String>(&mut conn)
                .optional()
                .ok()
                .flatten();

            if let Some(url) = base_url {
                let hashes: Vec<String> = items.iter().map(|(_, h)| h.clone()).collect();
                let cancel_url = format!("{}/downloads/cancel", url);
                let req = BatchCancelRequest { hashes };
                match self
                    .http_client
                    .post(&cancel_url)
                    .json(&req)
                    .timeout(std::time::Duration::from_secs(10))
                    .send()
                    .await
                {
                    Ok(_) => {
                        tracing::info!(
                            "Cancelled {} torrents via module {}",
                            items.len(),
                            module_id
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Best-effort cancel failed for module {}: {}", module_id, e);
                    }
                }
            }

            // Mark as cancelled in DB regardless of API result
            let download_ids: Vec<i32> = items.iter().map(|(id, _)| *id).collect();
            diesel::update(downloads::table.filter(downloads::download_id.eq_any(&download_ids)))
                .set(downloads::status.eq("cancelled"))
                .execute(&mut conn)
                .map_err(|e| format!("Failed to update download status: {}", e))?;
            cancelled += items.len();
        }

        // Mark no-module records as cancelled too (no torrent hash = never sent to downloader)
        if !no_module.is_empty() {
            diesel::update(downloads::table.filter(downloads::download_id.eq_any(&no_module)))
                .set(downloads::status.eq("cancelled"))
                .execute(&mut conn)
                .map_err(|e| format!("Failed to update no-module download status: {}", e))?;
            cancelled += no_module.len();
        }

        Ok(cancelled)
    }
}
