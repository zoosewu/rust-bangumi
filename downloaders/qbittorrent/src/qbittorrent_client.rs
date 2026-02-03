use reqwest::{cookie::Jar, Client};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use anyhow::{anyhow, Result};

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

    pub async fn login(&self, username: &str, password: &str) -> Result<()> {
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

    pub async fn add_magnet(
        &self,
        magnet_url: &str,
        save_path: Option<&str>,
    ) -> Result<String> {
        let mut params = vec![("urls", magnet_url)];

        if let Some(path) = save_path {
            params.push(("savepath", path));
        }

        let resp = self
            .client
            .post(format!("{}/api/v2/torrents/add", self.base_url))
            .form(&params)
            .send()
            .await?;

        if resp.status().is_success() {
            tracing::info!("Magnet link added successfully");
            let hash = self.extract_hash_from_magnet(magnet_url)?;
            Ok(hash)
        } else {
            Err(anyhow!("Failed to add magnet: {}", resp.status()))
        }
    }

    pub async fn get_torrent_info(&self, hash: &str) -> Result<Option<TorrentInfo>> {
        let resp = self
            .client
            .get(format!("{}/api/v2/torrents/info", self.base_url))
            .query(&[("hashes", hash)])
            .send()
            .await?;

        if resp.status().is_success() {
            let torrents: Vec<TorrentInfo> = resp.json().await?;
            Ok(torrents.into_iter().next())
        } else {
            Err(anyhow!("Failed to get torrent info: {}", resp.status()))
        }
    }

    pub async fn get_all_torrents(&self) -> Result<Vec<TorrentInfo>> {
        let resp = self
            .client
            .get(format!("{}/api/v2/torrents/info", self.base_url))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow!("Failed to get torrents: {}", resp.status()))
        }
    }

    pub async fn pause_torrent(&self, hash: &str) -> Result<()> {
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

    pub async fn resume_torrent(&self, hash: &str) -> Result<()> {
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

    pub async fn delete_torrent(&self, hash: &str, delete_files: bool) -> Result<()> {
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

    pub fn extract_hash_from_magnet(&self, magnet_url: &str) -> Result<String> {
        // magnet:?xt=urn:btih:HASH&dn=...
        if let Some(start) = magnet_url.find("btih:") {
            let hash_start = start + 5;
            let hash_part = &magnet_url[hash_start..];
            let hash = hash_part
                .split('&')
                .next()
                .unwrap_or("")
                .to_lowercase();

            if !hash.is_empty() && hash.len() >= 32 {
                Ok(hash)
            } else {
                Err(anyhow!("Invalid hash extracted from magnet URL"))
            }
        } else {
            Err(anyhow!("Invalid magnet URL format"))
        }
    }
}

use crate::traits::DownloaderClient;

impl DownloaderClient for QBittorrentClient {
    async fn login(&self, username: &str, password: &str) -> Result<()> {
        QBittorrentClient::login(self, username, password).await
    }

    async fn add_magnet(&self, magnet_url: &str, save_path: Option<&str>) -> Result<String> {
        QBittorrentClient::add_magnet(self, magnet_url, save_path).await
    }

    async fn get_torrent_info(&self, hash: &str) -> Result<Option<TorrentInfo>> {
        QBittorrentClient::get_torrent_info(self, hash).await
    }

    async fn get_all_torrents(&self) -> Result<Vec<TorrentInfo>> {
        QBittorrentClient::get_all_torrents(self).await
    }

    async fn pause_torrent(&self, hash: &str) -> Result<()> {
        QBittorrentClient::pause_torrent(self, hash).await
    }

    async fn resume_torrent(&self, hash: &str) -> Result<()> {
        QBittorrentClient::resume_torrent(self, hash).await
    }

    async fn delete_torrent(&self, hash: &str, delete_files: bool) -> Result<()> {
        QBittorrentClient::delete_torrent(self, hash, delete_files).await
    }

    fn extract_hash_from_magnet(&self, magnet_url: &str) -> Result<String> {
        QBittorrentClient::extract_hash_from_magnet(self, magnet_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_hash_from_magnet() {
        let client = QBittorrentClient::new("http://localhost:8080".to_string());
        let magnet = "magnet:?xt=urn:btih:1234567890abcdef1234567890abcdef&dn=test&tr=http://tracker.example.com";
        let hash = client.extract_hash_from_magnet(magnet).unwrap();
        assert_eq!(hash, "1234567890abcdef1234567890abcdef");
    }

    #[test]
    fn test_extract_hash_invalid_magnet() {
        let client = QBittorrentClient::new("http://localhost:8080".to_string());
        let result = client.extract_hash_from_magnet("invalid_url");
        assert!(result.is_err());
    }

    #[test]
    fn test_client_creation() {
        let client = QBittorrentClient::new("http://localhost:8080".to_string());
        assert_eq!(client.base_url, "http://localhost:8080");
    }

    #[test]
    fn test_torrent_info_structure() {
        let info = TorrentInfo {
            hash: "abc123".to_string(),
            name: "test".to_string(),
            state: "downloading".to_string(),
            progress: 0.5,
            dlspeed: 1024000,
            size: 1000000000,
            downloaded: 500000000,
        };
        assert_eq!(info.progress, 0.5);
    }
}
