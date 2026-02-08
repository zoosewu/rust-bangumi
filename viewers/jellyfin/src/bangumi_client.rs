use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::time::Duration;

const BANGUMI_API_BASE: &str = "https://api.bgm.tv";
const USER_AGENT: &str = "bangumi-viewer/1.0";

pub struct BangumiClient {
    http_client: reqwest::Client,
}

// ============ API Response Types ============

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub results: i32,
    pub list: Option<Vec<SearchItem>>,
}

#[derive(Debug, Deserialize)]
pub struct SearchItem {
    pub id: i32,
    pub name: String,
    pub name_cn: Option<String>,
    pub air_date: Option<String>,
    pub images: Option<SearchImages>,
}

#[derive(Debug, Deserialize)]
pub struct SearchImages {
    pub large: Option<String>,
    pub common: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectDetail {
    pub id: i32,
    pub name: String,
    pub name_cn: Option<String>,
    pub summary: Option<String>,
    pub date: Option<String>,
    pub images: Option<SubjectImages>,
    pub rating: Option<SubjectRating>,
    pub total_episodes: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectImages {
    pub large: Option<String>,
    pub common: Option<String>,
    pub medium: Option<String>,
    pub small: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectRating {
    pub score: Option<f32>,
    pub total: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct EpisodesResponse {
    pub data: Vec<EpisodeItem>,
    pub total: i32,
}

#[derive(Debug, Deserialize)]
pub struct EpisodeItem {
    pub id: i32,
    pub ep: Option<i32>,
    pub sort: i32,
    pub name: Option<String>,
    pub name_cn: Option<String>,
    pub airdate: Option<String>,
    pub desc: Option<String>,
}

// ============ Client Implementation ============

impl BangumiClient {
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to create HTTP client");
        Self { http_client }
    }

    /// Search for an anime by title. Returns the first match's bangumi_id.
    pub async fn search_anime(&self, title: &str) -> Result<Option<i32>> {
        let url = format!(
            "{}/search/subject/{}?type=2&responseGroup=small",
            BANGUMI_API_BASE,
            urlencoding::encode(title)
        );

        let resp = self.http_client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!("bangumi.tv search returned {}", resp.status()));
        }

        let result: SearchResult = resp.json().await?;

        if result.results > 0 {
            if let Some(list) = &result.list {
                if let Some(first) = list.first() {
                    return Ok(Some(first.id));
                }
            }
        }

        Ok(None)
    }

    /// Get detailed subject info by bangumi_id.
    pub async fn get_subject(&self, bangumi_id: i32) -> Result<SubjectDetail> {
        let url = format!("{}/v0/subjects/{}", BANGUMI_API_BASE, bangumi_id);
        let resp = self.http_client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "bangumi.tv subject {} returned {}",
                bangumi_id,
                resp.status()
            ));
        }

        Ok(resp.json().await?)
    }

    /// Get episode list for a subject.
    pub async fn get_episodes(&self, bangumi_id: i32) -> Result<Vec<EpisodeItem>> {
        let mut all_episodes = Vec::new();
        let mut offset = 0;
        let limit = 100;

        loop {
            let url = format!(
                "{}/v0/episodes?subject_id={}&type=0&limit={}&offset={}",
                BANGUMI_API_BASE, bangumi_id, limit, offset
            );

            let resp = self.http_client.get(&url).send().await?;

            if !resp.status().is_success() {
                return Err(anyhow!(
                    "bangumi.tv episodes for {} returned {}",
                    bangumi_id,
                    resp.status()
                ));
            }

            let result: EpisodesResponse = resp.json().await?;
            let count = result.data.len();
            all_episodes.extend(result.data);

            if all_episodes.len() >= result.total as usize || count == 0 {
                break;
            }
            offset += limit;

            // Rate limiting
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Ok(all_episodes)
    }

    /// Download an image from URL to a local file path.
    pub async fn download_image(&self, url: &str, target_path: &std::path::Path) -> Result<()> {
        let resp = self.http_client.get(url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to download image: {}", resp.status()));
        }

        let bytes = resp.bytes().await?;
        tokio::fs::write(target_path, &bytes).await?;
        Ok(())
    }
}
