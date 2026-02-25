pub mod mock;
pub mod qbittorrent_client;
pub mod traits;

pub use mock::MockDownloaderClient;
pub use qbittorrent_client::{QBittorrentClient, TorrentInfo};
pub use traits::DownloaderClient;
