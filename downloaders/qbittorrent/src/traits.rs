// src/traits.rs
use crate::TorrentInfo;
use anyhow::Result;

/// Trait defining the interface for torrent download clients.
/// This abstraction allows for mock implementations in tests.
pub trait DownloaderClient: Send + Sync {
    /// Authenticate with the torrent client
    fn login(
        &self,
        username: &str,
        password: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Add a magnet link and return the torrent hash
    fn add_magnet(
        &self,
        magnet_url: &str,
        save_path: Option<&str>,
    ) -> impl std::future::Future<Output = Result<String>> + Send;

    /// Get information about a specific torrent by hash
    fn get_torrent_info(
        &self,
        hash: &str,
    ) -> impl std::future::Future<Output = Result<Option<TorrentInfo>>> + Send;

    /// Get information about all torrents
    fn get_all_torrents(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<TorrentInfo>>> + Send;

    /// Pause a torrent
    fn pause_torrent(&self, hash: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Resume a paused torrent
    fn resume_torrent(&self, hash: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Delete a torrent, optionally deleting downloaded files
    fn delete_torrent(
        &self,
        hash: &str,
        delete_files: bool,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Extract the info hash from a magnet URL
    fn extract_hash_from_magnet(&self, magnet_url: &str) -> Result<String>;

    /// Add a torrent by URL (magnet link or .torrent HTTP URL) and return the torrent hash
    fn add_torrent(
        &self,
        url: &str,
        save_path: Option<&str>,
    ) -> impl std::future::Future<Output = Result<String>> + Send;

    /// Extract the info hash from a URL (magnet link or .torrent URL)
    fn extract_hash_from_url(&self, url: &str) -> Result<String>;
}
