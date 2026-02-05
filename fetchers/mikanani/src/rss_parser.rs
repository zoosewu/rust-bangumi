use feed_rs::parser;
use shared::models::RawAnimeItem;
use crate::retry::retry_with_backoff;
use std::time::Duration;
use chrono::{DateTime, Utc};

pub struct RssParser;

impl RssParser {
    pub fn new() -> Self {
        Self
    }

    /// 抓取 RSS 並回傳原始項目（不解析）
    pub async fn fetch_raw_items(&self, rss_url: &str) -> Result<Vec<RawAnimeItem>, String> {
        // Download RSS feed with retry logic
        let url = rss_url.to_string();
        let content = retry_with_backoff(
            3,
            Duration::from_secs(2),
            || {
                let url = url.clone();
                async move {
                    let resp = reqwest::get(&url).await?;
                    let resp = resp.error_for_status()?;
                    resp.bytes().await
                }
            },
        )
        .await
        .map_err(|e| format!("Failed to fetch RSS feed: {}", e))?;

        // Parse RSS
        let feed = parser::parse(&content[..])
            .map_err(|e| format!("Failed to parse RSS feed: {}", e))?;

        let mut items = Vec::new();

        for entry in feed.entries {
            let title = entry.title.map(|t| t.content).unwrap_or_default();
            if title.is_empty() {
                continue;
            }

            // Get download URL from enclosure or link
            let download_url = entry.media.first()
                .and_then(|m| m.content.first())
                .and_then(|c| c.url.as_ref())
                .map(|u| u.to_string())
                .or_else(|| entry.links.first().map(|l| l.href.clone()))
                .unwrap_or_default();

            if download_url.is_empty() {
                continue;
            }

            let description = entry.summary.map(|s| s.content);

            let pub_date = entry.published
                .or(entry.updated)
                .map(|dt| DateTime::<Utc>::from(dt));

            items.push(RawAnimeItem {
                title,
                description,
                download_url,
                pub_date,
            });
        }

        Ok(items)
    }
}

impl Default for RssParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 常用 BitTorrent tracker 列表
const TRACKERS: &[&str] = &[
    "http://open.acgtracker.com:1096/announce",
    "http://t.nyaatracker.com:80/announce",
    "udp://tracker.openbittorrent.com:80/announce",
];

/// 嘗試從 mikanani 的 .torrent URL 提取 hash 並構造 magnet link
///
/// URL 格式: `https://mikanani.me/Download/{date}/{hash}.torrent`
fn torrent_url_to_magnet(url: &str) -> Option<String> {
    if !url.contains("mikanani.me") || !url.ends_with(".torrent") {
        return None;
    }

    let filename = url.rsplit('/').next()?;
    let hash = filename.strip_suffix(".torrent")?;

    if hash.len() < 32 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let trackers: String = TRACKERS.iter().map(|t| format!("&tr={}", t)).collect();

    Some(format!(
        "magnet:?xt=urn:btih:{}{}",
        hash.to_lowercase(),
        trackers
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torrent_url_to_magnet_valid_mikanani_url() {
        let url = "https://mikanani.me/Download/20241222/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc.torrent";
        let result = torrent_url_to_magnet(url);
        assert!(result.is_some());
        let magnet = result.unwrap();
        assert!(magnet.starts_with("magnet:?xt=urn:btih:ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc"));
        assert!(magnet.contains("&tr="));
    }

    #[test]
    fn test_torrent_url_to_magnet_uppercase_hash_lowered() {
        let url = "https://mikanani.me/Download/20241222/ABCDEF1234567890ABCDEF1234567890ABCDEF12.torrent";
        let result = torrent_url_to_magnet(url);
        assert!(result.is_some());
        assert!(result
            .unwrap()
            .contains("abcdef1234567890abcdef1234567890abcdef12"));
    }

    #[test]
    fn test_torrent_url_to_magnet_non_mikanani_returns_none() {
        let url = "https://example.com/Download/20241222/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc.torrent";
        assert!(torrent_url_to_magnet(url).is_none());
    }

    #[test]
    fn test_torrent_url_to_magnet_non_torrent_returns_none() {
        let url = "https://mikanani.me/Home/Episode/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc";
        assert!(torrent_url_to_magnet(url).is_none());
    }

    #[test]
    fn test_torrent_url_to_magnet_short_hash_returns_none() {
        let url = "https://mikanani.me/Download/20241222/shorthash.torrent";
        assert!(torrent_url_to_magnet(url).is_none());
    }

    #[test]
    fn test_torrent_url_to_magnet_non_hex_hash_returns_none() {
        let url = "https://mikanani.me/Download/20241222/not_a_valid_hex_string_at_all_nope.torrent";
        assert!(torrent_url_to_magnet(url).is_none());
    }
}
