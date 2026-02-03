pub mod qbittorrent_client;
pub mod retry;
pub mod traits;

pub use qbittorrent_client::{QBittorrentClient, TorrentInfo};
pub use retry::retry_with_backoff;
pub use traits::DownloaderClient;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lib_exports() {
        // Verify that public exports are available
        let _ = QBittorrentClient::new("http://localhost:8080".to_string());
    }
}
