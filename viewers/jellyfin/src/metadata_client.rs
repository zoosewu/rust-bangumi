use anyhow::Result;

pub struct MetadataClient {
    http: reqwest::Client,
    base_url: String,
}

pub struct EpisodeInfo {
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<String>,
    pub summary: Option<String>,
}

impl MetadataClient {
    pub fn new(base_url: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap(),
            base_url,
        }
    }

    pub async fn enrich_episodes(
        &self,
        bangumi_id: i32,
        episode_no: i32,
    ) -> Result<Option<EpisodeInfo>> {
        let resp = self
            .http
            .post(format!("{}/enrich/episodes", self.base_url))
            .json(&serde_json::json!({
                "bangumi_id": bangumi_id,
                "episode_no": episode_no
            }))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Ok(None);
        }
        let body: serde_json::Value = resp.json().await?;
        Ok(Some(EpisodeInfo {
            title: body["title"].as_str().map(|s| s.to_string()),
            title_cn: body["title_cn"].as_str().map(|s| s.to_string()),
            air_date: body["air_date"].as_str().map(|s| s.to_string()),
            summary: body["summary"].as_str().map(|s| s.to_string()),
        }))
    }

    /// Download an image from a URL and save it to the given path.
    pub async fn download_image(&self, url: &str, target_path: &std::path::Path) -> Result<()> {
        let resp = self.http.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to download image from {}: {}",
                url,
                resp.status()
            ));
        }
        let bytes = resp.bytes().await?;
        tokio::fs::write(target_path, &bytes).await?;
        Ok(())
    }
}
