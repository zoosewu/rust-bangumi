use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("AI returned invalid JSON: {0}")]
    InvalidJson(String),

    #[error("AI provider not configured")]
    NotConfigured,

    /// Provider 端故障：HTTP 5xx、網路錯誤、timeout。
    #[error("provider unavailable: {0}")]
    ProviderUnavailable(String),

    /// Provider 回應 4xx（key 錯、模型不存在、請求被拒等），通常是這一家的設定問題。
    #[error("provider error: {0}")]
    ApiError(String),

    /// Provider 限流（HTTP 429）。一輪全部失敗且其中有限流時，chain 會等
    /// `retry_after_secs` 後重試。訊息前綴 `[rate_limit_exceeded:N]` 供前端顯示倒數。
    #[error("[rate_limit_exceeded{}] {message}", retry_after_secs.map(|s| format!(":{s}")).unwrap_or_default())]
    RateLimited {
        retry_after_secs: Option<u32>,
        message: String,
    },
}

#[async_trait]
pub trait AiClient: Send + Sync {
    async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, AiError>;

    async fn chat_completion_structured(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        _schema: &serde_json::Value,
    ) -> Result<String, AiError> {
        self.chat_completion(system_prompt, user_prompt).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limited_shows_the_wait() {
        let e = AiError::RateLimited {
            retry_after_secs: Some(31),
            message: "TPM".into(),
        };
        assert_eq!(e.to_string(), "[rate_limit_exceeded:31] TPM");
    }
}
