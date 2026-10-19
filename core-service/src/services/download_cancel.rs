use crate::db::DbPool;
use crate::schema::{downloads, service_modules};
use diesel::prelude::*;
use shared::BatchCancelRequest;
use std::collections::{BTreeMap, HashSet};

/// Download statuses whose torrent is still in the downloader and can be cancelled.
/// `completed` is downloaded but not yet in the library: a conflict loser or a newly
/// filtered link in that state would otherwise keep its torrent and files forever.
const CANCELLABLE_STATUSES: [&str; 3] = ["downloading", "downloader_error", "completed"];

/// Download statuses that no longer need their torrent or its files: nothing will be
/// downloaded for them again, and a synced episode has already been moved to the library.
const TORRENT_UNNEEDED_STATUSES: [&str; 3] = ["cancelled", "failed", "synced"];

/// A download whose torrent is to be removed: (download_id, module_id, torrent_hash).
pub type TorrentRef = (i32, Option<i32>, Option<String>);

/// Torrents to remove, grouped by the downloader module holding them.
///
/// Cancelling removes the files too, so a torrent shared with a download outside
/// this removal that still needs it (another episode of a batch) is kept. Downloads
/// without a module or hash were never handed to a downloader and are skipped.
fn torrents_to_remove(
    targets: &[TorrentRef],
    still_needed: &HashSet<String>,
) -> BTreeMap<i32, Vec<String>> {
    targets
        .iter()
        .filter_map(|(_, module_id, hash)| Some(((*module_id)?, hash.clone()?)))
        .filter(|(_, hash)| !still_needed.contains(hash))
        .fold(BTreeMap::new(), |mut by_module, (module_id, hash)| {
            let hashes: &mut Vec<String> = by_module.entry(module_id).or_default();
            if !hashes.contains(&hash) {
                hashes.push(hash);
            }
            by_module
        })
}

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
    /// Removes their torrents and partial files from the downloader and marks the
    /// DB records as 'cancelled'. Silently skips links with no active downloads.
    pub async fn cancel_downloads_for_links(&self, link_ids: &[i32]) -> Result<usize, String> {
        if link_ids.is_empty() {
            return Ok(0);
        }

        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        let active: Vec<TorrentRef> = downloads::table
            .filter(downloads::link_id.eq_any(link_ids))
            .filter(downloads::status.eq_any(CANCELLABLE_STATUSES))
            .select((
                downloads::download_id,
                downloads::module_id,
                downloads::torrent_hash,
            ))
            .load::<TorrentRef>(&mut conn)
            .map_err(|e| format!("Failed to query active downloads: {}", e))?;

        if active.is_empty() {
            return Ok(0);
        }

        self.remove_torrents(&mut conn, &active).await?;

        // Mark as cancelled in DB regardless of the downloader's answer
        let download_ids: Vec<i32> = active.iter().map(|(id, _, _)| *id).collect();
        diesel::update(downloads::table.filter(downloads::download_id.eq_any(&download_ids)))
            .set(downloads::status.eq("cancelled"))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to update download status: {}", e))?;

        Ok(download_ids.len())
    }

    /// Best-effort removal of these downloads' torrents, with their files, from the
    /// downloader that holds each of them (`module_id`).
    pub async fn remove_torrents(
        &self,
        conn: &mut PgConnection,
        targets: &[TorrentRef],
    ) -> Result<(), String> {
        let download_ids: Vec<i32> = targets.iter().map(|(id, _, _)| *id).collect();
        let hashes: Vec<String> = targets.iter().filter_map(|(_, _, h)| h.clone()).collect();
        if hashes.is_empty() {
            return Ok(());
        }

        let still_needed: HashSet<String> = downloads::table
            .filter(downloads::torrent_hash.eq_any(&hashes))
            .filter(downloads::download_id.ne_all(&download_ids))
            .filter(downloads::status.ne_all(TORRENT_UNNEEDED_STATUSES))
            .select(downloads::torrent_hash.assume_not_null())
            .distinct()
            .load::<String>(conn)
            .map_err(|e| format!("Failed to query torrents still in use: {}", e))?
            .into_iter()
            .collect();
        if !still_needed.is_empty() {
            tracing::info!(
                "Keeping {} torrents still needed by other downloads",
                still_needed.len()
            );
        }

        for (module_id, hashes) in torrents_to_remove(targets, &still_needed) {
            let base_url: Option<String> = service_modules::table
                .filter(service_modules::module_id.eq(module_id))
                .select(service_modules::base_url)
                .first::<String>(conn)
                .optional()
                .ok()
                .flatten();
            let Some(url) = base_url else {
                tracing::warn!(
                    "Downloader module {} not found, cannot remove {} torrents",
                    module_id,
                    hashes.len()
                );
                continue;
            };

            let count = hashes.len();
            match self
                .http_client
                .post(format!("{}/downloads/cancel", url))
                .json(&BatchCancelRequest { hashes })
                .timeout(std::time::Duration::from_secs(10))
                .send()
                .await
            {
                Ok(_) => tracing::info!("Removed {} torrents via module {}", count, module_id),
                Err(e) => {
                    tracing::warn!("Best-effort cancel failed for module {}: {}", module_id, e)
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(id: i32, module: Option<i32>, hash: Option<&str>) -> TorrentRef {
        (id, module, hash.map(str::to_string))
    }

    #[test]
    fn a_downloaded_but_unsynced_torrent_can_be_cancelled() {
        assert!(CANCELLABLE_STATUSES.contains(&"completed"));
        // 已經在媒體庫的集數不在這裡處理（刪檔走 viewer）
        assert!(!CANCELLABLE_STATUSES.contains(&"synced"));
    }

    #[test]
    fn torrents_go_to_the_downloader_that_holds_them() {
        let targets = [
            target(1, Some(2), Some("aaa")),
            target(2, Some(5), Some("bbb")),
            target(3, Some(2), Some("ccc")),
        ];
        let by_module = torrents_to_remove(&targets, &HashSet::new());
        assert_eq!(by_module[&2], vec!["aaa", "ccc"]);
        assert_eq!(by_module[&5], vec!["bbb"]);
    }

    #[test]
    fn batch_torrent_is_removed_once() {
        let targets = [
            target(1, Some(2), Some("batch")),
            target(2, Some(2), Some("batch")),
        ];
        assert_eq!(
            torrents_to_remove(&targets, &HashSet::new())[&2],
            vec!["batch"]
        );
    }

    #[test]
    fn torrent_still_needed_by_another_episode_is_kept() {
        let targets = [
            target(1, Some(2), Some("batch")),
            target(2, Some(2), Some("single")),
        ];
        let still_needed = HashSet::from(["batch".to_string()]);
        assert_eq!(
            torrents_to_remove(&targets, &still_needed)[&2],
            vec!["single"]
        );
    }

    #[test]
    fn downloads_never_sent_to_a_downloader_are_skipped() {
        let targets = [target(1, None, Some("aaa")), target(2, Some(2), None)];
        assert!(torrents_to_remove(&targets, &HashSet::new()).is_empty());
    }
}
