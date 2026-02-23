use serde::de::DeserializeOwned;
use serde::Serialize;

/// HTTP API 客戶端
#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    pub base_url: String,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    /// 處理非成功 HTTP 回應，嘗試解析錯誤訊息
    async fn handle_error(response: reqwest::Response) -> anyhow::Error {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
                return anyhow::anyhow!("HTTP {}: {}", status, msg);
            }
            if let Some(msg) = val.get("error").and_then(|m| m.as_str()) {
                return anyhow::anyhow!("HTTP {}: {}", status, msg);
            }
        }
        anyhow::anyhow!("HTTP {}: {}", status, text)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        response
            .json::<T>()
            .await
            .map_err(|e| anyhow::anyhow!("解析回應失敗: {}", e))
    }

    pub async fn post<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow::Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        response
            .json::<R>()
            .await
            .map_err(|e| anyhow::anyhow!("解析回應失敗: {}", e))
    }

    /// POST 無 body，回傳 JSON 回應
    pub async fn post_no_body<R: DeserializeOwned>(&self, path: &str) -> anyhow::Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        response
            .json::<R>()
            .await
            .map_err(|e| anyhow::anyhow!("解析回應失敗: {}", e))
    }

    /// POST 無 body，不解析回應
    pub async fn post_no_body_ignore_response(&self, path: &str) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        Ok(())
    }

    pub async fn patch<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow::Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .patch(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        response
            .json::<R>()
            .await
            .map_err(|e| anyhow::anyhow!("解析回應失敗: {}", e))
    }

    pub async fn put<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow::Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .put(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        response
            .json::<R>()
            .await
            .map_err(|e| anyhow::anyhow!("解析回應失敗: {}", e))
    }

    pub async fn delete(&self, path: &str) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("無法連線到 {}: {}", self.base_url, e))?;
        if !response.status().is_success() {
            return Err(Self::handle_error(response).await);
        }
        Ok(())
    }
}
