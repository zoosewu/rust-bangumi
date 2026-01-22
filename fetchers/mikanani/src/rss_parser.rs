use feed_rs::parser;
use sha2::{Sha256, Digest};
use regex::Regex;
use shared::models::{FetchedAnime, FetchedLink};
use std::collections::HashMap;
use crate::retry::retry_with_backoff;
use std::time::Duration;

pub struct RssParser {
    episode_regex: Regex,
}

impl RssParser {
    pub fn new() -> Self {
        Self {
            // Match episode numbers: [01], 第01話, EP01, etc.
            episode_regex: Regex::new(r"(?:\[|第|EP)(\d+)(?:\]|話|集)?").unwrap(),
        }
    }

    pub async fn parse_feed(&self, rss_url: &str) -> Result<Vec<FetchedAnime>, String> {
        // Download RSS feed with retry logic
        let url = rss_url.to_string();
        let content = retry_with_backoff(
            3,  // Max 3 attempts
            Duration::from_secs(2),  // Initial 2s delay
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

        let mut animes_map: HashMap<String, FetchedAnime> = HashMap::new();

        for entry in feed.entries {
            // Extract title and link
            let title = entry.title.map(|t| t.content).unwrap_or_default();
            let link = entry.links.first()
                .map(|l| l.href.clone())
                .unwrap_or_default();

            // Parse anime information
            if let Some((anime_title, subtitle_group, episode_no)) = self.parse_title(&title) {
                // Generate source_hash
                let source_hash = self.generate_hash(&link);

                // Build FetchedLink
                let fetched_link = FetchedLink {
                    episode_no,
                    subtitle_group: subtitle_group.clone(),
                    title: title.clone(),
                    url: link,
                    source_hash,
                    source_rss_url: rss_url.to_string(),
                };

                // Group by anime title
                animes_map.entry(anime_title.clone())
                    .or_insert_with(|| FetchedAnime {
                        title: anime_title.clone(),
                        description: String::new(),
                        season: "unknown".to_string(),
                        year: 2025,
                        series_no: 1,
                        links: Vec::new(),
                    })
                    .links.push(fetched_link);
            }
        }

        Ok(animes_map.into_values().collect())
    }

    pub fn parse_title_public(&self, title: &str) -> Option<(String, String, i32)> {
        self.parse_title(title)
    }

    fn parse_title(&self, title: &str) -> Option<(String, String, i32)> {
        // Example formats:
        // - "[Subtitle Group] Anime Title [01][1080p]"
        // - "[字幕組] 動畫標題 第05話 [1080p]"
        // - "[Group] Title EP12 [720p]"

        // Extract subtitle group (first bracketed part)
        let subtitle_group = title
            .split('[')
            .nth(1)?
            .split(']')
            .next()?
            .to_string();

        // Extract episode number and find its position
        let episode_match = self.episode_regex.captures(title)?;
        let episode_no = episode_match
            .get(1)
            .and_then(|m| m.as_str().parse::<i32>().ok())?;
        let episode_match_start = episode_match.get(0)?.start();

        // Extract anime title - between first ] and the episode marker
        let first_close = title.find(']')?;
        let anime_title_end = episode_match_start;

        let between = &title[first_close + 1..anime_title_end];
        let anime_title = between.trim().to_string();

        if anime_title.is_empty() {
            return None;
        }

        Some((anime_title, subtitle_group, episode_no))
    }

    pub fn generate_hash_public(&self, url: &str) -> String {
        self.generate_hash(url)
    }

    fn generate_hash(&self, url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_title_standard_format() {
        let parser = RssParser::new();
        let title = "[SubGroup] Anime Title [01][1080p]";
        let result = parser.parse_title(title);

        assert!(result.is_some());
        let (anime, group, episode) = result.unwrap();
        assert_eq!(group, "SubGroup");
        assert_eq!(episode, 1);
        assert_eq!(anime, "Anime Title");
    }

    #[test]
    fn test_parse_title_chinese_episode_format() {
        let parser = RssParser::new();
        let title = "[字幕組] 動畫標題 第05話 [1080p]";
        let result = parser.parse_title(title);

        assert!(result.is_some());
        let (_anime, group, episode) = result.unwrap();
        assert_eq!(group, "字幕組");
        assert_eq!(episode, 5);
    }

    #[test]
    fn test_parse_title_ep_format() {
        let parser = RssParser::new();
        let title = "[Group] Title EP12 [720p]";
        let result = parser.parse_title(title);

        assert!(result.is_some());
        let (_anime, _group, episode) = result.unwrap();
        assert_eq!(episode, 12);
    }

    #[test]
    fn test_generate_hash_deterministic() {
        let parser = RssParser::new();
        let url = "magnet:?xt=urn:btih:abc123";
        let hash1 = parser.generate_hash(url);
        let hash2 = parser.generate_hash(url);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_generate_hash_unique() {
        let parser = RssParser::new();
        let hash1 = parser.generate_hash("url1");
        let hash2 = parser.generate_hash("url2");

        assert_ne!(hash1, hash2);
    }
}
