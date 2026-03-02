// shared/src/downloader_trait.rs
use anyhow::Result;
use crate::{CancelResultItem, DownloadRequestItem, DownloadResultItem, DownloadStatusItem};

pub trait DownloaderClient: Send + Sync {
    fn login(
        &self,
        username: &str,
        password: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn add_torrents(
        &self,
        items: Vec<DownloadRequestItem>,
    ) -> impl std::future::Future<Output = Result<Vec<DownloadResultItem>>> + Send;

    fn cancel_torrents(
        &self,
        hashes: Vec<String>,
    ) -> impl std::future::Future<Output = Result<Vec<CancelResultItem>>> + Send;

    fn query_status(
        &self,
        hashes: Vec<String>,
    ) -> impl std::future::Future<Output = Result<Vec<DownloadStatusItem>>> + Send;

    fn pause_torrent(&self, hash: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    fn resume_torrent(&self, hash: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    fn delete_torrent(
        &self,
        hash: &str,
        delete_files: bool,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}
