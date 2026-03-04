use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::client::{AiClient, AiError};

pub struct OpenAiClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiClient {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    fmt_type: &'static str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: AssistantMessage,
}

#[derive(Deserialize)]
struct AssistantMessage {
    content: String,
}

#[async_trait]
impl AiClient for OpenAiClient {
    async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, AiError> {
        if self.api_key.is_empty() {
            return Err(AiError::NotConfigured);
        }

        let mut messages = vec![];
        if !system_prompt.is_empty() {
            messages.push(Message { role: "system", content: system_prompt });
        }
        messages.push(Message { role: "user", content: user_prompt });

        let body = ChatRequest {
            model: &self.model,
            messages,
            response_format: ResponseFormat { fmt_type: "json_object" },
        };

        let resp = self.http
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AiError::ApiError(text));
        }

        let chat: ChatResponse = resp.json().await?;
        chat.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| AiError::ApiError("Empty choices".into()))
    }
}
