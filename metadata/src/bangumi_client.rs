use crate::models::CoverImageInfo;
use crate::series::{parse_cn_date, walks_prequels};
use anyhow::{bail, Result};
use std::collections::HashSet;

const BANGUMI_API_BASE: &str = "https://api.bgm.tv";

/// 前傳鏈最多追幾層（目前最長的系列也遠低於此）
const MAX_PREQUEL_DEPTH: usize = 10;

/// bgm subject（一季）的內容
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Subject {
    pub id: String,
    pub name: String,
    pub name_cn: String,
    pub summary: Option<String>,
    pub air_date: Option<String>,
    pub end_date: Option<String>,
    pub total_episodes: Option<i32>,
    pub platform: String,
    pub cover_images: Vec<CoverImageInfo>,
}

pub struct EpisodeMeta {
    pub episode_no: i32,
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<String>,
    pub summary: Option<String>,
}

pub struct BangumiClient {
    http: reqwest::Client,
    base_url: String,
}

fn non_empty(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn subject_from_json(body: &serde_json::Value) -> Option<Subject> {
    let id = body["id"].as_i64()?.to_string();
    let end_date = body["infobox"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|item| item["key"] == "播放结束")
        .and_then(|item| item["value"].as_str())
        .and_then(parse_cn_date);
    let cover_images = body["images"]["large"]
        .as_str()
        .filter(|url| !url.is_empty() && !url.ends_with("no_img.gif"))
        .map(|url| CoverImageInfo {
            url: url.to_string(),
            source: "bangumi".to_string(),
        })
        .into_iter()
        .collect();
    Some(Subject {
        id,
        name: body["name"].as_str().unwrap_or_default().to_string(),
        name_cn: body["name_cn"].as_str().unwrap_or_default().to_string(),
        summary: non_empty(&body["summary"]),
        air_date: non_empty(&body["date"]),
        end_date,
        total_episodes: body["total_episodes"]
            .as_i64()
            .or_else(|| body["eps"].as_i64())
            .map(|n| n as i32),
        platform: body["platform"].as_str().unwrap_or_default().to_string(),
        cover_images,
    })
}

impl BangumiClient {
    pub fn new() -> Self {
        Self::with_base_url(BANGUMI_API_BASE)
    }

    pub fn with_base_url(base_url: &str) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("anime-manager/0.1")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// 一季的內容；subject 不存在時回傳 None。
    pub async fn get_subject(&self, id: &str) -> Result<Option<Subject>> {
        let resp = self
            .http
            .get(format!("{}/v0/subjects/{}", self.base_url, id))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            bail!("bgm subject {} returned HTTP {}", id, resp.status());
        }
        let body: serde_json::Value = resp.json().await?;
        Ok(subject_from_json(&body))
    }

    /// 這一季的前傳（同為動畫）的 subject id。
    async fn get_prequel_id(&self, id: &str) -> Result<Option<String>> {
        let resp = self
            .http
            .get(format!("{}/v0/subjects/{}/subjects", self.base_url, id))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("bgm relations of {} returned HTTP {}", id, resp.status());
        }
        let relations: serde_json::Value = resp.json().await?;
        Ok(relations
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["relation"] == "前传" && r["type"] == 2)
            .and_then(|r| r["id"].as_i64())
            .map(|id| id.to_string()))
    }

    /// 由第一季排到 `season` 的前傳鏈。只有 TV / WEB 動畫往前追，遇到劇場版、OVA 就停；
    /// 有循環保護與深度上限。
    pub async fn series_chain(&self, season: Subject) -> Result<Vec<Subject>> {
        let mut chain = vec![season];
        let mut seen: HashSet<String> = chain.iter().map(|s| s.id.clone()).collect();
        while chain.len() <= MAX_PREQUEL_DEPTH {
            let current = chain.last().expect("chain is never empty");
            if !walks_prequels(&current.platform) {
                break;
            }
            let Some(prequel_id) = self.get_prequel_id(&current.id).await? else {
                break;
            };
            if !seen.insert(prequel_id.clone()) {
                tracing::warn!("bgm prequel chain loops back to {}", prequel_id);
                break;
            }
            match self.get_subject(&prequel_id).await? {
                Some(prequel) if walks_prequels(&prequel.platform) => chain.push(prequel),
                _ => break,
            }
        }
        chain.reverse();
        Ok(chain)
    }

    /// 指定集數的內容。`episode_no` 先比對該季的集數（ep），再比對跨季連續編號（sort）。
    pub async fn get_episode(&self, id: &str, episode_no: i32) -> Result<Option<EpisodeMeta>> {
        let resp = self
            .http
            .get(format!("{}/v0/episodes", self.base_url))
            .query(&[("subject_id", id), ("type", "0"), ("limit", "200")])
            .send()
            .await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let body: serde_json::Value = resp.json().await?;
        let eps = body["data"].as_array().cloned().unwrap_or_default();
        let number = |e: &serde_json::Value, key: &str| e[key].as_f64() == Some(episode_no as f64);
        let ep = eps
            .iter()
            .find(|e| number(e, "ep"))
            .or_else(|| eps.iter().find(|e| number(e, "sort")));
        Ok(ep.map(|e| EpisodeMeta {
            episode_no,
            title: non_empty(&e["name"]),
            title_cn: non_empty(&e["name_cn"]),
            air_date: non_empty(&e["airdate"]),
            summary: non_empty(&e["desc"]),
        }))
    }
}
