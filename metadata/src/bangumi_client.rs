use anyhow::Result;
use crate::models::CoverImageInfo;

const BANGUMI_API_BASE: &str = "https://api.bgm.tv";

pub struct BangumiClient {
    http: reqwest::Client,
}

pub struct SubjectMeta {
    pub summary: Option<String>,
    pub air_date: Option<String>,
}

pub struct EpisodeMeta {
    pub episode_no: i32,
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<String>,
    pub summary: Option<String>,
}

impl BangumiClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent("anime-manager/0.1")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to build HTTP client");
        Self { http }
    }

    /// 搜尋動畫，回傳第一個 Bangumi subject_id
    pub async fn search_anime(&self, title: &str) -> Result<Option<i32>> {
        let url = format!(
            "{}/search/subject/{}?type=2&responseGroup=small",
            BANGUMI_API_BASE,
            urlencoding::encode(title)
        );
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let body: serde_json::Value = resp.json().await?;
        let id = body["list"][0]["id"].as_i64().map(|v| v as i32);
        Ok(id)
    }

    /// 取得封面圖 URL
    pub async fn get_cover_images(&self, bangumi_id: i32) -> Result<Vec<CoverImageInfo>> {
        let url = format!("{}/v0/subjects/{}", BANGUMI_API_BASE, bangumi_id);
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }
        let body: serde_json::Value = resp.json().await?;
        let mut images = vec![];
        if let Some(large) = body["images"]["large"].as_str() {
            if !large.is_empty() && !large.ends_with("no_img.gif") {
                images.push(CoverImageInfo {
                    url: large.to_string(),
                    source: "bangumi".to_string(),
                });
            }
        }
        Ok(images)
    }

    /// 取得 summary 和 air_date
    pub async fn get_subject_meta(&self, bangumi_id: i32) -> Result<SubjectMeta> {
        let url = format!("{}/v0/subjects/{}", BANGUMI_API_BASE, bangumi_id);
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(SubjectMeta { summary: None, air_date: None });
        }
        let body: serde_json::Value = resp.json().await?;
        Ok(SubjectMeta {
            summary: body["summary"].as_str().map(|s| s.to_string()),
            air_date: body["date"].as_str().map(|s| s.to_string()),
        })
    }

    /// 取得指定集數的 metadata
    pub async fn get_episode(&self, bangumi_id: i32, episode_no: i32) -> Result<Option<EpisodeMeta>> {
        let url = format!("{}/v0/episodes", BANGUMI_API_BASE);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("subject_id", bangumi_id.to_string()),
                ("type", "0".to_string()),
                ("limit", "100".to_string()),
            ])
            .send()
            .await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let body: serde_json::Value = resp.json().await?;
        let eps = body["data"].as_array().cloned().unwrap_or_default();
        let ep = eps
            .iter()
            .find(|e| e["ep"].as_i64().map(|n| n as i32) == Some(episode_no));
        Ok(ep.map(|e| EpisodeMeta {
            episode_no,
            title: e["name"].as_str().map(|s| s.to_string()),
            title_cn: e["name_cn"].as_str().map(|s| s.to_string()),
            air_date: e["airdate"].as_str().map(|s| s.to_string()),
            summary: e["desc"].as_str().map(|s| s.to_string()),
        }))
    }
}
