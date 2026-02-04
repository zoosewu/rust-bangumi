// tests/common/mod.rs
use downloader_qbittorrent::TorrentInfo;

pub fn sample_torrent_info() -> TorrentInfo {
    TorrentInfo {
        hash: "abc123def456789012345678901234ab".to_string(),
        name: "Test Torrent".to_string(),
        state: "downloading".to_string(),
        progress: 0.5,
        dlspeed: 1024000,
        size: 1000000000,
        downloaded: 500000000,
    }
}

pub fn valid_magnet() -> &'static str {
    "magnet:?xt=urn:btih:1234567890abcdef1234567890abcdef&dn=test"
}

pub fn valid_magnet_hash() -> &'static str {
    "1234567890abcdef1234567890abcdef"
}
