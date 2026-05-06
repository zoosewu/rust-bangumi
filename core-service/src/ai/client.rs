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

    /// Provider 端故障：HTTP 5xx、網路錯誤、timeout、rate limit。可 fallback。
    #[error("provider unavailable: {0}")]
    ProviderUnavailable(String),

    /// Provider 正常回應但內容問題（4xx auth / bad request 等）。不 fallback。
    #[error("provider error: {0}")]
    ApiError(String),
}

impl AiError {
    /// 是否應該 fallback 到下一個 provider
    pub fn is_retryable(&self) -> bool {
        match self {
            AiError::ProviderUnavailable(_) => true,
            AiError::Http(e) => e.is_timeout() || e.is_connect() || e.is_request(),
            _ => false,
        }
    }
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
    fn provider_unavailable_is_retryable() {
        assert!(AiError::ProviderUnavailable("503".into()).is_retryable());
    }

    #[test]
    fn api_error_is_not_retryable() {
        assert!(!AiError::ApiError("401".into()).is_retryable());
    }

    #[test]
    fn invalid_json_is_not_retryable() {
        assert!(!AiError::InvalidJson("oops".into()).is_retryable());
    }

    #[test]
    fn not_configured_is_not_retryable() {
        assert!(!AiError::NotConfigured.is_retryable());
    }

    #[test]
    fn http_builder_error_is_not_retryable() {
        let client = reqwest::Client::new();
        // empty URL forces a builder error path (non-transient, not retryable)
        let err = client.get("").build().unwrap_err();
        let ai_err: AiError = err.into();
        assert!(!ai_err.is_retryable());
    }
}
