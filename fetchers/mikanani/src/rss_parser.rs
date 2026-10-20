use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use feed_rs::model::Entry;
use feed_rs::parser;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Decoder, Reader};
use shared::models::RawAnimeItem;
use shared::retry_with_backoff;
use std::collections::HashMap;
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

        parse_feed(&content)
    }
}

/// 解析 RSS 內容為原始項目。
///
/// 沒有標準 `<pubDate>` 的項目，改用 mikanani 自訂的 `<torrent><pubDate>`（feed-rs 不認得這個元素）。
fn parse_feed(content: &[u8]) -> Result<Vec<RawAnimeItem>, String> {
    let feed = parser::parse(content).map_err(|e| format!("Failed to parse RSS feed: {}", e))?;
    let torrent_dates = mikan_torrent_pub_dates(content);

    Ok(feed
        .entries
        .into_iter()
        .filter_map(entry_to_raw_item)
        .map(|mut item| {
            item.pub_date = item
                .pub_date
                .or_else(|| torrent_dates.get(&item.download_url).copied());
            item
        })
        .collect())
}

/// mikanani 的 `<torrent><pubDate>` 沒有時區，是 UTC+8
const MIKAN_UTC_OFFSET_SECS: i32 = 8 * 3600;

/// 每個 item 的 enclosure URL → mikanani `<torrent><pubDate>`。讀不到的項目略過。
fn mikan_torrent_pub_dates(content: &[u8]) -> HashMap<String, DateTime<Utc>> {
    let mut reader = Reader::from_reader(content);
    let mut buf = Vec::new();
    let mut path: Vec<Vec<u8>> = Vec::new();
    let mut url: Option<String> = None;
    let mut date: Option<DateTime<Utc>> = None;
    let mut dates = HashMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.local_name().as_ref() == b"item" {
                    url = None;
                    date = None;
                }
                url = url.or_else(|| enclosure_url(&path, &e, reader.decoder()));
                path.push(e.local_name().as_ref().to_vec());
            }
            Ok(Event::Empty(e)) => url = url.or_else(|| enclosure_url(&path, &e, reader.decoder())),
            Ok(Event::Text(t)) if path.ends_with(&[b"torrent".to_vec(), b"pubDate".to_vec()]) => {
                date = t.unescape().ok().and_then(|s| parse_mikan_date(&s));
            }
            Ok(Event::End(e)) => {
                path.pop();
                if e.local_name().as_ref() == b"item" {
                    if let (Some(u), Some(d)) = (url.take(), date.take()) {
                        dates.insert(u, d);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    dates
}

/// `<item>` 底下 `<enclosure>` 的 url 屬性
fn enclosure_url(path: &[Vec<u8>], e: &BytesStart, decoder: Decoder) -> Option<String> {
    (e.local_name().as_ref() == b"enclosure" && path.last().is_some_and(|p| p == b"item"))
        .then(|| e.try_get_attribute("url").ok().flatten())
        .flatten()
        .and_then(|a| {
            a.decode_and_unescape_value(decoder)
                .ok()
                .map(|v| v.into_owned())
        })
}

/// `2026-10-10T10:08:50.071939`（UTC+8）→ UTC
fn parse_mikan_date(s: &str) -> Option<DateTime<Utc>> {
    let naive = NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S%.f").ok()?;
    FixedOffset::east_opt(MIKAN_UTC_OFFSET_SECS)?
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
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
        external_ids: vec![],
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

    /// 擷取自 mikanani 的實際 RSS（2026-10-20）
    const MIKAN_SAMPLE: &str = r#"<?xml version="1.0" encoding="utf-8"?><rss version="2.0"><channel><title>Mikan Project - 药屋少女的呢喃 第三季</title><link>http://mikanani.me/RSS/Bangumi?bangumiId=4106&amp;subgroupid=1236</link><description>Mikan Project - 药屋少女的呢喃 第三季</description><item><guid isPermaLink="false">[绿茶字幕组] 药屋少女的呢喃 第三季 / Kusuriya no Hitorigoto [50][WebRip][1080p][繁日内嵌]</guid><link>https://mikanani.me/Home/Episode/58b5e7347ff0cb53155ef63fec0590e2df80b1cc</link><title>[绿茶字幕组] 药屋少女的呢喃 第三季 / Kusuriya no Hitorigoto [50][WebRip][1080p][繁日内嵌]</title><description>[绿茶字幕组] 药屋少女的呢喃 第三季 / Kusuriya no Hitorigoto [50][WebRip][1080p][繁日内嵌][433.7 MB]</description><torrent xmlns="https://mikanani.me/0.1/"><link>https://mikanani.me/Home/Episode/58b5e7347ff0cb53155ef63fec0590e2df80b1cc</link><contentLength>454767424</contentLength><pubDate>2026-10-10T10:08:50.071939</pubDate></torrent><enclosure type="application/x-bittorrent" length="454767424" url="https://mikanani.me/Download/20261010/58b5e7347ff0cb53155ef63fec0590e2df80b1cc.torrent" /></item><item><guid isPermaLink="false">[绿茶字幕组] 药屋少女的呢喃 第三季 / Kusuriya no Hitorigoto [49][WebRip][1080p][繁日内嵌]</guid><title>[绿茶字幕组] 药屋少女的呢喃 第三季 / Kusuriya no Hitorigoto [49][WebRip][1080p][繁日内嵌]</title><torrent xmlns="https://mikanani.me/0.1/"><pubDate>2026-10-03T09:30:00</pubDate></torrent><enclosure type="application/x-bittorrent" length="1" url="https://mikanani.me/Download/20261003/aaaa.torrent" /></item><item><title>[X] no date - 01</title><enclosure type="application/x-bittorrent" length="1" url="https://mikanani.me/Download/20261003/bbbb.torrent" /></item></channel></rss>"#;

    #[test]
    fn mikan_torrent_pub_date_is_read_as_utc_plus_8() {
        let items = parse_feed(MIKAN_SAMPLE.as_bytes()).expect("parse");
        let dates: Vec<Option<String>> = items
            .iter()
            .map(|i| {
                i.pub_date
                    .map(|d| d.format("%Y-%m-%d %H:%M:%S%.f").to_string())
            })
            .collect();
        // 與 torrent 的 Last-Modified（Sat, 10 Oct 2026 02:08:50 GMT）一致
        assert_eq!(
            dates,
            [
                Some("2026-10-10 02:08:50.071939".to_string()),
                Some("2026-10-03 01:30:00".to_string()),
                None,
            ]
        );
    }

    #[test]
    fn a_standard_pub_date_wins_over_the_mikan_one() {
        let xml = br#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0"><channel><title>t</title>
  <item>
    <title>[G] Show - 01</title>
    <pubDate>Sat, 10 Oct 2026 00:00:00 GMT</pubDate>
    <torrent xmlns="https://mikanani.me/0.1/"><pubDate>2026-10-10T10:08:50</pubDate></torrent>
    <enclosure type="application/x-bittorrent" length="1" url="https://mikanani.me/Download/20261010/abc.torrent" />
  </item>
</channel></rss>"#;
        let items = parse_feed(xml).expect("parse");
        assert_eq!(
            items[0].pub_date.unwrap().to_rfc3339(),
            "2026-10-10T00:00:00+00:00"
        );
    }
}
