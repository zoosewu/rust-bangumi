use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use super::client::{AiClient, AiError};

pub struct OpenAiClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: i32,
    /// "strict" | "non_strict" | "inject_schema"
    response_format_mode: String,
}

impl OpenAiClient {
    pub fn new(base_url: &str, api_key: &str, model: &str, max_tokens: i32, response_format_mode: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            max_tokens,
            response_format_mode: response_format_mode.to_string(),
        }
    }

    fn build_messages(system_prompt: &str, user_prompt: &str) -> Value {
        let mut messages = vec![];
        if !system_prompt.is_empty() {
            messages.push(json!({"role": "system", "content": system_prompt}));
        }
        messages.push(json!({"role": "user", "content": user_prompt}));
        json!(messages)
    }

    async fn do_request(&self, messages: Value, response_format: Option<Value>) -> Result<String, AiError> {
        if self.api_key.is_empty() {
            return Err(AiError::NotConfigured);
        }
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": self.max_tokens,
        });
        if let Some(fmt) = response_format {
            body["response_format"] = fmt;
        }
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
        let messages = Self::build_messages(system_prompt, user_prompt);
        let response_format = if self.response_format_mode == "inject_schema" {
            None
        } else {
            Some(json!({"type": "json_object"}))
        };
        self.do_request(messages, response_format).await
    }

    async fn chat_completion_structured(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &Value,
    ) -> Result<String, AiError> {
        match self.response_format_mode.as_str() {
            "inject_schema" => {
                // 將 JSON Schema 注入 system prompt，不傳 response_format
                let schema_text = serde_json::to_string_pretty(schema)
                    .unwrap_or_else(|_| schema.to_string());
                let augmented_system = format!(
                    "{}\n\n## Output JSON Schema\nYou MUST output valid JSON matching this schema exactly:\n```json\n{}\n```",
                    system_prompt, schema_text
                );
                let messages = Self::build_messages(&augmented_system, user_prompt);
                self.do_request(messages, None).await
            }
            mode => {
                let strict = mode == "strict";
                let messages = Self::build_messages(system_prompt, user_prompt);
                let response_format = json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": "output",
                        "strict": strict,
                        "schema": schema
                    }
                });
                self.do_request(messages, Some(response_format)).await
            }
        }
    }
}
