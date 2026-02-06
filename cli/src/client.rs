use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::{debug, error};

/// HTTP API 客戶端
#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
}

impl ApiClient {
    /// 創建新的 API 客戶端
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    /// 發送 GET 請求
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{}{}", self.base_url, path);
        debug!("GET {}", url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("GET 請求失敗 {}: {}", url, e);
            anyhow::anyhow!("GET 請求失敗: {}", e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            error!("GET {} 返回狀態 {}: {}", url, status, text);
            return Err(anyhow::anyhow!("HTTP {}: {}", status, text));
        }

        response.json::<T>().await.map_err(|e| {
            error!("解析 JSON 失敗: {}", e);
            anyhow::anyhow!("解析回應失敗: {}", e)
        })
    }

    /// 發送 POST 請求
    pub async fn post<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow::Result<R> {
        let url = format!("{}{}", self.base_url, path);
        debug!("POST {} with body", url);

        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| {
                error!("POST 請求失敗 {}: {}", url, e);
                anyhow::anyhow!("POST 請求失敗: {}", e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            error!("POST {} 返回狀態 {}: {}", url, status, text);
            return Err(anyhow::anyhow!("HTTP {}: {}", status, text));
        }

        response.json::<R>().await.map_err(|e| {
            error!("解析 JSON 失敗: {}", e);
            anyhow::anyhow!("解析回應失敗: {}", e)
        })
    }

    /// 發送 DELETE 請求
    pub async fn delete(&self, path: &str) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url, path);
        debug!("DELETE {}", url);

        let response = self.client.delete(&url).send().await.map_err(|e| {
            error!("DELETE 請求失敗 {}: {}", url, e);
            anyhow::anyhow!("DELETE 請求失敗: {}", e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            error!("DELETE {} 返回狀態 {}: {}", url, status, text);
            return Err(anyhow::anyhow!("HTTP {}: {}", status, text));
        }

        Ok(())
    }
}
