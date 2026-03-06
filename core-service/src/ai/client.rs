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

    /// 使用 JSON Schema 強制輸出格式（Structured Outputs）
    /// 預設實作回退至 chat_completion（供不支援此功能的 provider 使用）
    async fn chat_completion_structured(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        _schema: &serde_json::Value,
    ) -> Result<String, AiError> {
        self.chat_completion(system_prompt, user_prompt).await
    }
}
