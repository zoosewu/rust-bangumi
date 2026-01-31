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
