use feed_rs::model::Entry;
use feed_rs::parser;
use shared::models::RawAnimeItem;
use shared::retry_with_backoff;
use std::time::Duration;

pub struct RssParser;

impl RssParser {
    pub fn new() -> Self {
        Self
    }

    /// 抓取 RSS 並回傳原始項目（不解析）
    pub async fn fetch_raw_items(&self, rss_url: &str) -> Result<Vec<RawAnimeItem>, String> {
        // Download RSS feed with retry logic
        let url = rss_url.to_string();
        let content = retry_with_backoff(3, Duration::from_secs(2), || {
            let url = url.clone();
            async move {
                let resp = reqwest::get(&url).await?;
                let resp = resp.error_for_status()?;
                resp.bytes().await
            }
        })
        .await
        .map_err(|e| format!("Failed to fetch RSS feed: {}", e))?;

        // Parse RSS
        let feed =
            parser::parse(&content[..]).map_err(|e| format!("Failed to parse RSS feed: {}", e))?;

        Ok(feed
            .entries
            .into_iter()
            .filter_map(entry_to_raw_item)
            .collect())
    }
}

impl Default for RssParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 將 RSS entry 轉為 RawAnimeItem。
///
/// download_url 保留原始 .torrent URL：下載器抓取 torrent 檔可取得完整的
/// 現役 tracker 清單；轉成僅含 hash 的 magnet 會丟失 tracker 並依賴 DHT。
fn entry_to_raw_item(entry: Entry) -> Option<RawAnimeItem> {
    let title = entry.title.map(|t| t.content).unwrap_or_default();
    if title.is_empty() {
        return None;
    }

    // Get download URL from enclosure or link
    let download_url = entry
        .media
        .first()
        .and_then(|m| m.content.first())
        .and_then(|c| c.url.as_ref())
        .map(|u| u.to_string())
        .or_else(|| entry.links.first().map(|l| l.href.clone()))
        .filter(|u| !u.is_empty())?;

    let description = entry.summary.map(|s| s.content);

    let pub_date = entry.published.or(entry.updated);

    Some(RawAnimeItem {
        title,
        description,
        download_url,
        pub_date,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS_SAMPLE: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0">
  <channel>
    <title>Mikan Project - Test</title>
    <link>https://mikanani.me/RSS/Bangumi?bangumiId=1</link>
    <item>
      <guid isPermaLink="false">[Group] Test Anime - 01</guid>
      <link>https://mikanani.me/Home/Episode/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc</link>
      <title>[Group] Test Anime - 01</title>
      <enclosure type="application/x-bittorrent" length="123" url="https://mikanani.me/Download/20241222/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc.torrent" />
    </item>
  </channel>
</rss>"#;

    fn first_entry(xml: &[u8]) -> Entry {
        parser::parse(xml)
            .expect("parse RSS")
            .entries
            .into_iter()
            .next()
            .expect("one entry")
    }

    #[test]
    fn test_entry_keeps_original_torrent_url() {
        let item = entry_to_raw_item(first_entry(RSS_SAMPLE)).expect("item");

        assert_eq!(
            item.download_url,
            "https://mikanani.me/Download/20241222/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc.torrent",
            "enclosure 的 .torrent URL 應原樣保留，不轉換為 magnet"
        );
        assert_eq!(item.title, "[Group] Test Anime - 01");
    }

    #[test]
    fn test_entry_without_title_is_skipped() {
        let xml = br#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0"><channel><title>t</title>
  <item>
    <enclosure type="application/x-bittorrent" length="1" url="https://mikanani.me/Download/20241222/abc.torrent" />
  </item>
</channel></rss>"#;
        assert!(entry_to_raw_item(first_entry(xml)).is_none());
    }
}
