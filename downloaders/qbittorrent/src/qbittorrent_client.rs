use anyhow::{anyhow, Result};
use reqwest::{cookie::Jar, Client};
use serde::{Deserialize, Serialize};
use shared::{CancelResultItem, DownloadRequestItem, DownloadResultItem, DownloadStatusItem};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct QBittorrentClient {
    pub client: Client,
    pub base_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TorrentInfo {
    pub hash: String,
    pub name: String,
    pub state: String,
    pub progress: f64,
    pub dlspeed: i64,
    pub size: i64,
    pub downloaded: i64,
}

impl QBittorrentClient {
    pub fn new(base_url: String) -> Self {
        let jar = Arc::new(Jar::default());
        let client = Client::builder()
            .cookie_provider(jar)
            .build()
            .expect("Failed to build HTTP client");

        Self { client, base_url }
    }

    /// Extract the info hash from a magnet URL or .torrent URL (private helper)
    fn extract_hash_from_url(url: &str) -> Result<String> {
        if url.starts_with("magnet:") {
            return Self::extract_hash_from_magnet(url);
        }

        // Try to extract hash from .torrent URL filename
        // Format: https://example.com/path/{hash}.torrent
        if url.ends_with(".torrent") {
            if let Some(filename) = url.rsplit('/').next() {
                if let Some(hash) = filename.strip_suffix(".torrent") {
                    let hash = hash.to_lowercase();
                    if hash.len() >= 32 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Ok(hash);
                    }
                }
            }
        }

        Err(anyhow!("Cannot extract hash from URL: {}", url))
    }

    /// Extract the info hash from a magnet URL (private helper)
    fn extract_hash_from_magnet(magnet_url: &str) -> Result<String> {
        // magnet:?xt=urn:btih:HASH&dn=...
        if let Some(start) = magnet_url.find("btih:") {
            let hash_start = start + 5;
            let hash_part = &magnet_url[hash_start..];
            let hash = hash_part.split('&').next().unwrap_or("").to_lowercase();

            if !hash.is_empty() && hash.len() >= 32 {
                Ok(hash)
            } else {
                Err(anyhow!("Invalid hash extracted from magnet URL"))
            }
        } else {
            Err(anyhow!("Invalid magnet URL format"))
        }
    }

    /// Map qBittorrent torrent state string to a normalized status string
    fn map_torrent_state(state: &str) -> String {
        match state {
            "error" | "missingFiles" => "error".to_string(),
            "uploading" | "stalledUP" | "forcedUP" | "queuedUP" | "checkingUP" => {
                "completed".to_string()
            }
            "pausedDL" | "pausedUP" => "paused".to_string(),
            "downloading" | "stalledDL" | "forcedDL" | "queuedDL" | "checkingDL"
            | "metaDL" | "allocating" | "moving" => "downloading".to_string(),
            "checkingResumeData" => "checking".to_string(),
            "unknown" | _ => "unknown".to_string(),
        }
    }
}

use crate::traits::DownloaderClient;

impl DownloaderClient for QBittorrentClient {
    async fn login(&self, username: &str, password: &str) -> Result<()> {
        let params = [("username", username), ("password", password)];

        let resp = self
            .client
            .post(format!("{}/api/v2/auth/login", self.base_url))
            .form(&params)
            .send()
            .await?;

        if resp.status().is_success() {
            let body = resp.text().await?;
            if body == "Ok." {
                tracing::info!("Successfully logged in to qBittorrent");
                Ok(())
            } else {
                Err(anyhow!("Login failed: {}", body))
            }
        } else {
            Err(anyhow!("Login request failed: {}", resp.status()))
        }
    }

    async fn add_torrents(
        &self,
        items: Vec<DownloadRequestItem>,
    ) -> Result<Vec<DownloadResultItem>> {
        let mut results = Vec::with_capacity(items.len());

        for item in &items {
            if !item.url.starts_with("magnet:") && !item.url.starts_with("http") {
                results.push(DownloadResultItem {
                    url: item.url.clone(),
                    hash: None,
                    status: "failed".to_string(),
                    reason: Some("Unsupported URL format".to_string()),
                });
                continue;
            }

            let mut params = vec![("urls", item.url.as_str())];
            let save_path = &item.save_path;
            if !save_path.is_empty() {
                params.push(("savepath", save_path.as_str()));
            }

            let resp = self
                .client
                .post(format!("{}/api/v2/torrents/add", self.base_url))
                .form(&params)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    match Self::extract_hash_from_url(&item.url) {
                        Ok(hash) => {
                            tracing::info!("Torrent added successfully: {}", hash);
                            results.push(DownloadResultItem {
                                url: item.url.clone(),
                                hash: Some(hash),
                                status: "accepted".to_string(),
                                reason: None,
                            });
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Torrent added but could not extract hash from URL: {}",
                                e
                            );
                            results.push(DownloadResultItem {
                                url: item.url.clone(),
                                hash: None,
                                status: "accepted".to_string(),
                                reason: Some(format!("Added but hash extraction failed: {}", e)),
                            });
                        }
                    }
                }
                Ok(r) => {
                    let status = r.status();
                    tracing::error!("Failed to add torrent {}: {}", item.url, status);
                    results.push(DownloadResultItem {
                        url: item.url.clone(),
                        hash: None,
                        status: "failed".to_string(),
                        reason: Some(format!("qBittorrent returned {}", status)),
                    });
                }
                Err(e) => {
                    tracing::error!("Request error adding torrent {}: {}", item.url, e);
                    results.push(DownloadResultItem {
                        url: item.url.clone(),
                        hash: None,
                        status: "failed".to_string(),
                        reason: Some(format!("Request error: {}", e)),
                    });
                }
            }
        }

        Ok(results)
    }

    async fn cancel_torrents(&self, hashes: Vec<String>) -> Result<Vec<CancelResultItem>> {
        let mut results = Vec::with_capacity(hashes.len());

        for hash in &hashes {
            let resp = self
                .client
                .post(format!("{}/api/v2/torrents/delete", self.base_url))
                .form(&[("hashes", hash.as_str()), ("deleteFiles", "false")])
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    tracing::info!("Torrent {} cancelled", hash);
                    results.push(CancelResultItem {
                        hash: hash.clone(),
                        status: "cancelled".to_string(),
                    });
                }
                Ok(r) => {
                    let status = r.status();
                    tracing::error!("Failed to cancel torrent {}: {}", hash, status);
                    results.push(CancelResultItem {
                        hash: hash.clone(),
                        status: format!("failed: {}", status),
                    });
                }
                Err(e) => {
                    tracing::error!("Request error cancelling torrent {}: {}", hash, e);
                    results.push(CancelResultItem {
                        hash: hash.clone(),
                        status: format!("failed: {}", e),
                    });
                }
            }
        }

        Ok(results)
    }

    async fn query_status(&self, hashes: Vec<String>) -> Result<Vec<DownloadStatusItem>> {
        if hashes.is_empty() {
            return Ok(vec![]);
        }

        let hashes_param = hashes.join("|");
        let resp = self
            .client
            .get(format!("{}/api/v2/torrents/info", self.base_url))
            .query(&[("hashes", &hashes_param)])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "Failed to query torrent status: {}",
                resp.status()
            ));
        }

        let torrents: Vec<TorrentInfo> = resp.json().await?;

        // Build a set of returned hashes for lookup
        let returned_hashes: std::collections::HashSet<String> =
            torrents.iter().map(|t| t.hash.clone()).collect();

        let mut results: Vec<DownloadStatusItem> = torrents
            .iter()
            .map(|t| DownloadStatusItem {
                hash: t.hash.clone(),
                status: Self::map_torrent_state(&t.state),
                progress: t.progress,
                size: t.size as u64,
            })
            .collect();

        // For hashes not found in qBittorrent, report as "not_found"
        for hash in &hashes {
            if !returned_hashes.contains(hash) {
                results.push(DownloadStatusItem {
                    hash: hash.clone(),
                    status: "not_found".to_string(),
                    progress: 0.0,
                    size: 0,
                });
            }
        }

        Ok(results)
    }

    async fn pause_torrent(&self, hash: &str) -> Result<()> {
        let resp = self
            .client
            .post(format!("{}/api/v2/torrents/pause", self.base_url))
            .form(&[("hashes", hash)])
            .send()
            .await?;

        if resp.status().is_success() {
            tracing::info!("Torrent {} paused", hash);
            Ok(())
        } else {
            Err(anyhow!("Failed to pause torrent: {}", resp.status()))
        }
    }

    async fn resume_torrent(&self, hash: &str) -> Result<()> {
        let resp = self
            .client
            .post(format!("{}/api/v2/torrents/resume", self.base_url))
            .form(&[("hashes", hash)])
            .send()
            .await?;

        if resp.status().is_success() {
            tracing::info!("Torrent {} resumed", hash);
            Ok(())
        } else {
            Err(anyhow!("Failed to resume torrent: {}", resp.status()))
        }
    }

    async fn delete_torrent(&self, hash: &str, delete_files: bool) -> Result<()> {
        let resp = self
            .client
            .post(format!("{}/api/v2/torrents/delete", self.base_url))
            .form(&[
                ("hashes", hash),
                ("deleteFiles", if delete_files { "true" } else { "false" }),
            ])
            .send()
            .await?;

        if resp.status().is_success() {
            tracing::info!("Torrent {} deleted", hash);
            Ok(())
        } else {
            Err(anyhow!("Failed to delete torrent: {}", resp.status()))
        }
    }
}
