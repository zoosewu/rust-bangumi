// src/traits.rs
use anyhow::Result;
use shared::{CancelResultItem, DownloadRequestItem, DownloadResultItem, DownloadStatusItem};

/// Trait defining the interface for torrent download clients.
/// This abstraction allows for mock implementations in tests.
pub trait DownloaderClient: Send + Sync {
    /// Authenticate with the torrent client
    fn login(
        &self,
        username: &str,
        password: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Add multiple torrents in batch, returning per-item results
    fn add_torrents(
        &self,
        items: Vec<DownloadRequestItem>,
    ) -> impl std::future::Future<Output = Result<Vec<DownloadResultItem>>> + Send;

    /// Cancel (delete without removing files) multiple torrents by hash
    fn cancel_torrents(
        &self,
        hashes: Vec<String>,
    ) -> impl std::future::Future<Output = Result<Vec<CancelResultItem>>> + Send;

    /// Query download status for multiple torrents by hash
    fn query_status(
        &self,
        hashes: Vec<String>,
    ) -> impl std::future::Future<Output = Result<Vec<DownloadStatusItem>>> + Send;

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
}
