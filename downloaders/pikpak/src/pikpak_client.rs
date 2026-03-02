// downloaders/pikpak/src/pikpak_client.rs
use crate::{
    db::{Db, DownloadRecord},
    pikpak_api::PikPakApi,
};
use anyhow::Result;
use shared::{
    CancelResultItem, DownloadRequestItem, DownloadResultItem, DownloadStatusItem, DownloaderClient,
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

// ── Hash extraction ────────────────────────────────────────────────────────

fn extract_magnet_hash(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    let prefix = "urn:btih:";
    let start = lower.find(prefix)? + prefix.len();
    let end = lower[start..].find('&').map(|i| start + i).unwrap_or(lower.len());
    let hash = &url[start..end];
    if hash.len() >= 32 {
        Some(hash.to_uppercase())
    } else {
        None
    }
}

fn synthetic_hash(url: &str) -> String {
    use sha2::{Digest, Sha256};
    let result = Sha256::digest(url.as_bytes());
    hex::encode(&result[..20])
}

pub fn extract_hash(url: &str) -> String {
    if url.starts_with("magnet:") {
        extract_magnet_hash(url).unwrap_or_else(|| synthetic_hash(url))
    } else {
        synthetic_hash(url)
    }
}

// ── PikPakClient ───────────────────────────────────────────────────────────

pub struct PikPakClient {
    api: Arc<PikPakApi>,
    db: Db,
    polling_started: Arc<AtomicBool>,
}

impl PikPakClient {
    pub fn new(db_path: &str) -> Result<Self> {
        Ok(Self {
            api: Arc::new(PikPakApi::new()),
            db: Db::open(db_path)?,
            polling_started: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Start the background polling loop. Idempotent — safe to call multiple times.
    pub fn start_polling(&self) {
        // Use compare_exchange to ensure only one polling loop is started.
        if self
            .polling_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            tracing::debug!("Polling already started, skipping.");
            return;
        }
        let api = self.api.clone();
        let db = self.db.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                if let Err(e) = poll_once(&api, &db).await {
                    tracing::warn!("PikPak polling error: {e}");
                }
            }
        });
        tracing::info!("PikPak polling loop started.");
    }
}

// ── Background polling ─────────────────────────────────────────────────────

async fn poll_once(api: &PikPakApi, db: &Db) -> Result<()> {
    if !api.is_logged_in() {
        return Ok(());
    }

    let downloading = db.list_by_status("downloading")?;
    if downloading.is_empty() {
        return Ok(());
    }

    // Fetch completed tasks from PikPak
    let completed = api.list_completed_tasks().await?;
    // Map: task_id → file_id
    let completed_map: std::collections::HashMap<String, String> = completed
        .into_iter()
        .filter_map(|t| t.file_id.map(|fid| (t.id, fid)))
        .collect();

    for rec in &downloading {
        let task_id = match &rec.task_id {
            Some(id) => id.clone(),
            None => continue,
        };

        if let Some(file_id) = completed_map.get(&task_id) {
            tracing::info!(
                "PikPak task {} complete, starting local download for hash={}",
                task_id,
                rec.hash
            );
            match download_to_local(api, db, rec, file_id).await {
                Ok(()) => tracing::info!("Local download complete for hash={}", rec.hash),
                Err(e) => {
                    tracing::error!("Local download failed for hash={}: {e}", rec.hash);
                    let _ = db.update_error(&rec.hash, &e.to_string());
                }
            }
        }
    }

    Ok(())
}

async fn download_to_local(
    api: &PikPakApi,
    db: &Db,
    rec: &DownloadRecord,
    file_id: &str,
) -> Result<()> {
    let (download_url, size) = api.get_file_download_url(file_id).await?;

    // Extract filename from URL (before query params)
    let filename = download_url
        .split('/')
        .last()
        .and_then(|s| s.split('?').next())
        .filter(|s| !s.is_empty())
        .unwrap_or("download");

    let dest_path = Path::new(&rec.save_path).join(filename);

    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let resp = reqwest::get(&download_url).await?.error_for_status()?;
    let mut stream = resp.bytes_stream();

    let mut file = tokio::fs::File::create(&dest_path).await?;
    let mut downloaded: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
    }
    file.flush().await?;

    let dest_str = dest_path.to_string_lossy().to_string();
    let files_json = serde_json::json!([dest_str]).to_string();
    let actual_size = if size > 0 { size } else { downloaded };

    db.update_completed(&rec.hash, file_id, &dest_str, &files_json, actual_size)?;
    Ok(())
}

// ── DownloaderClient impl ──────────────────────────────────────────────────

impl DownloaderClient for PikPakClient {
    async fn login(&self, username: &str, password: &str) -> Result<()> {
        self.api.login(username, password).await
    }

    async fn add_torrents(
        &self,
        items: Vec<DownloadRequestItem>,
    ) -> Result<Vec<DownloadResultItem>> {
        let mut results = Vec::new();
        for item in items {
            let hash = extract_hash(&item.url);

            // Skip if already tracked
            if let Ok(Some(existing)) = self.db.get(&hash) {
                results.push(DownloadResultItem {
                    url: item.url,
                    hash: Some(hash),
                    status: existing.status,
                    reason: None,
                });
                continue;
            }

            match self.api.offline_download(&item.url).await {
                Ok(task) => {
                    let rec = DownloadRecord {
                        hash: hash.clone(),
                        task_id: Some(task.id.clone()),
                        file_id: None,
                        url: item.url.clone(),
                        save_path: item.save_path.clone(),
                        status: "downloading".to_string(),
                        progress: 0.0,
                        size: 0,
                        content_path: None,
                        files_json: None,
                        error_msg: None,
                    };
                    if let Err(e) = self.db.insert(&rec) {
                        tracing::error!(
                            "Failed to persist download record for hash={hash}: {e}"
                        );
                    }
                    results.push(DownloadResultItem {
                        url: item.url,
                        hash: Some(hash),
                        status: "accepted".to_string(),
                        reason: None,
                    });
                }
                Err(e) => {
                    tracing::error!("PikPak offline_download failed for {}: {e}", item.url);
                    results.push(DownloadResultItem {
                        url: item.url,
                        hash: None,
                        status: "failed".to_string(),
                        reason: Some(e.to_string()),
                    });
                }
            }
        }
        Ok(results)
    }

    async fn cancel_torrents(&self, hashes: Vec<String>) -> Result<Vec<CancelResultItem>> {
        let mut results = Vec::new();
        for hash in hashes {
            match self.db.get(&hash) {
                Ok(Some(rec)) => {
                    if let Some(task_id) = &rec.task_id {
                        let _ = self.api.delete_tasks(&[task_id.as_str()], false).await;
                    }
                    if let Err(e) = self.db.update_status(&hash, "cancelled", rec.progress) {
                        tracing::error!("Failed to update cancel status for hash={hash}: {e}");
                    }
                    results.push(CancelResultItem {
                        hash,
                        status: "cancelled".to_string(),
                    });
                }
                _ => {
                    results.push(CancelResultItem {
                        hash,
                        status: "not_found".to_string(),
                    });
                }
            }
        }
        Ok(results)
    }

    async fn query_status(&self, hashes: Vec<String>) -> Result<Vec<DownloadStatusItem>> {
        let records = self.db.get_many(&hashes)?;
        Ok(records
            .into_iter()
            .map(|rec| {
                let files: Vec<String> = rec
                    .files_json
                    .as_deref()
                    .and_then(|j| serde_json::from_str(j).ok())
                    .unwrap_or_default();
                DownloadStatusItem {
                    hash: rec.hash,
                    status: rec.status,
                    progress: rec.progress,
                    size: rec.size,
                    content_path: rec.content_path,
                    files,
                }
            })
            .collect())
    }

    // PikPak doesn't support pause/resume for offline downloads.
    async fn pause_torrent(&self, _hash: &str) -> Result<()> {
        Ok(())
    }

    async fn resume_torrent(&self, _hash: &str) -> Result<()> {
        Ok(())
    }

    async fn delete_torrent(&self, hash: &str, delete_files: bool) -> Result<()> {
        if let Ok(Some(rec)) = self.db.get(hash) {
            if let Some(task_id) = &rec.task_id {
                let _ = self.api.delete_tasks(&[task_id.as_str()], delete_files).await;
            }
            if delete_files {
                if let Some(path) = &rec.content_path {
                    let _ = tokio::fs::remove_file(path).await;
                }
            }
        }
        self.db.delete(hash)?;
        Ok(())
    }
}

// ── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_magnet_hash() {
        let magnet = "magnet:?xt=urn:btih:AABBCC1234567890AABBCC1234567890AABBCC12&dn=test";
        let hash = extract_hash(magnet);
        assert_eq!(hash, "AABBCC1234567890AABBCC1234567890AABBCC12");
    }

    #[test]
    fn test_extract_magnet_hash_lowercase_normalized() {
        let magnet = "magnet:?xt=urn:btih:aabbcc1234567890aabbcc1234567890aabbcc12&dn=test";
        let hash = extract_hash(magnet);
        assert_eq!(hash, "AABBCC1234567890AABBCC1234567890AABBCC12");
    }

    #[test]
    fn test_extract_http_synthetic_hash() {
        let url = "https://example.com/file.torrent";
        let hash = extract_hash(url);
        assert_eq!(hash.len(), 40);
        assert_eq!(extract_hash(url), hash); // deterministic
    }

    #[test]
    fn test_different_urls_different_hashes() {
        let h1 = extract_hash("https://a.com/1.torrent");
        let h2 = extract_hash("https://a.com/2.torrent");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_magnet_without_amp_works() {
        // Magnet with no additional params
        let magnet = "magnet:?xt=urn:btih:AABBCC1234567890AABBCC1234567890AABBCC12";
        let hash = extract_hash(magnet);
        assert_eq!(hash, "AABBCC1234567890AABBCC1234567890AABBCC12");
    }
}
