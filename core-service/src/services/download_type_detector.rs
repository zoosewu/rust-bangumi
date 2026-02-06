use shared::DownloadType;

trait DownloadTypeDetector {
    fn detect(&self, url: &str) -> Option<DownloadType>;
}

struct MagnetDetector;
impl DownloadTypeDetector for MagnetDetector {
    fn detect(&self, url: &str) -> Option<DownloadType> {
        url.starts_with("magnet:").then_some(DownloadType::Magnet)
    }
}

struct TorrentDetector;
impl DownloadTypeDetector for TorrentDetector {
    fn detect(&self, url: &str) -> Option<DownloadType> {
        (url.starts_with("http") && url.contains(".torrent")).then_some(DownloadType::Torrent)
    }
}

struct HttpDetector;
impl DownloadTypeDetector for HttpDetector {
    fn detect(&self, url: &str) -> Option<DownloadType> {
        url.starts_with("http").then_some(DownloadType::Http)
    }
}

pub fn detect_download_type(url: &str) -> Option<DownloadType> {
    let chain: Vec<Box<dyn DownloadTypeDetector>> = vec![
        Box::new(MagnetDetector),
        Box::new(TorrentDetector),
        Box::new(HttpDetector),
    ];
    chain.iter().find_map(|d| d.detect(url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_magnet() {
        assert_eq!(
            detect_download_type("magnet:?xt=urn:btih:abc123"),
            Some(DownloadType::Magnet)
        );
    }

    #[test]
    fn test_detect_torrent_url() {
        assert_eq!(
            detect_download_type("https://example.com/file.torrent"),
            Some(DownloadType::Torrent)
        );
    }

    #[test]
    fn test_detect_http() {
        assert_eq!(
            detect_download_type("https://example.com/download/123"),
            Some(DownloadType::Http)
        );
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_download_type("ftp://example.com/file"), None);
    }

    #[test]
    fn test_magnet_takes_priority_over_http() {
        assert_eq!(
            detect_download_type("magnet:?xt=urn:btih:abc"),
            Some(DownloadType::Magnet)
        );
    }

    #[test]
    fn test_torrent_takes_priority_over_http() {
        assert_eq!(
            detect_download_type("http://example.com/test.torrent"),
            Some(DownloadType::Torrent)
        );
    }

    #[test]
    fn test_mikanani_torrent_url() {
        assert_eq!(
            detect_download_type("https://mikanani.me/Download/20250101/abc123def456.torrent"),
            Some(DownloadType::Torrent)
        );
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(detect_download_type(""), None);
    }
}
