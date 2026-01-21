use feed_rs::parser;
use sha2::{Sha256, Digest};
use regex::Regex;
use shared::models::{FetchedAnime, FetchedLink};
use chrono::{Utc, Datelike};
use std::collections::HashMap;

pub struct RssParser {
    episode_regex: Regex,
    resolution_regex: Regex,
}

impl RssParser {
    pub fn new() -> Self {
        Self {
            // Match episode numbers: [01], 第01話, EP01, etc.
            episode_regex: Regex::new(r"(?:\[|第|EP)(\d+)(?:\]|話|集)?").unwrap(),
            // Match resolution: 1080p, 720p, etc.
            resolution_regex: Regex::new(r"(\d{3,4}[pP])").unwrap(),
        }
    }

    pub async fn parse_feed(&self, rss_url: &str) -> Result<Vec<FetchedAnime>, Box<dyn std::error::Error>> {
        // Download RSS feed
        let content = reqwest::get(rss_url).await?.bytes().await?;

        // Parse RSS
        let feed = parser::parse(&content[..])?;

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
                };

                // Group by anime title
                animes_map.entry(anime_title.clone())
                    .or_insert_with(|| FetchedAnime {
                        title: anime_title.clone(),
                        description: String::new(),
                        season: self.detect_season(&anime_title),
                        year: self.detect_year(&anime_title).unwrap_or(2025),
                        series_no: 1,
                        links: Vec::new(),
                    })
                    .links.push(fetched_link);
            }
        }

        Ok(animes_map.into_values().collect())
    }

    fn parse_title(&self, title: &str) -> Option<(String, String, i32)> {
        // Example format: "[Subtitle Group] Anime Title [01][1080p]"

        // Extract subtitle group (first bracketed part)
        let subtitle_group = title
            .split('[')
            .nth(1)?
            .split(']')
            .next()?
            .to_string();

        // Extract episode number
        let episode_no = self.episode_regex
            .captures(title)
            .and_then(|cap| cap.get(1))
            .and_then(|m| m.as_str().parse::<i32>().ok())?;

        // Extract anime title - between first ] and the episode number bracket
        let anime_title = if let Some(first_close) = title.find(']') {
            if let Some(episode_bracket) = title.rfind('[') {
                let between = &title[first_close + 1..episode_bracket];
                between.trim().to_string()
            } else {
                return None;
            }
        } else {
            return None;
        };

        if anime_title.is_empty() {
            return None;
        }

        Some((anime_title, subtitle_group, episode_no))
    }

    fn generate_hash(&self, url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn detect_season(&self, _title: &str) -> String {
        // Simplified: determine by current month
        let month = Utc::now().month();
        match month {
            1..=3 => "winter",
            4..=6 => "spring",
            7..=9 => "summer",
            _ => "fall",
        }.to_string()
    }

    fn detect_year(&self, _title: &str) -> Option<i32> {
        Some(Utc::now().year() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_title() {
        let parser = RssParser::new();

        let title = "[SubGroup] Anime Title [01][1080p]";
        let result = parser.parse_title(title);

        assert!(result.is_some());
        let (_anime, group, episode) = result.unwrap();
        assert_eq!(group, "SubGroup");
        assert_eq!(episode, 1);
    }

    #[test]
    fn test_generate_hash() {
        let parser = RssParser::new();
        let hash1 = parser.generate_hash("magnet:?xt=test1");
        let hash2 = parser.generate_hash("magnet:?xt=test2");

        assert_ne!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 length
    }
}
