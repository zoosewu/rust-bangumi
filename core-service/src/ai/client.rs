use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("AI returned invalid JSON: {0}")]
    InvalidJson(String),
    #[error("AI settings not configured")]
    NotConfigured,
    #[error("AI error: {0}")]
    ApiError(String),
}

#[async_trait]
pub trait AiClient: Send + Sync {
    async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, AiError>;
}
