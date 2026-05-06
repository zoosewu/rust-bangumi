use super::client::{AiClient, AiError};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

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
    pub fn new(
        base_url: &str,
        api_key: &str,
        model: &str,
        max_tokens: i32,
        response_format_mode: &str,
    ) -> Self {
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

    async fn do_request(
        &self,
        messages: Value,
        response_format: Option<Value>,
    ) -> Result<String, AiError> {
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": self.max_tokens,
        });
        if let Some(fmt) = response_format {
            body["response_format"] = fmt;
        }
        let mut request = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .json(&body);
        if let Some(token) = bearer_token(Some(&self.api_key)) {
            request = request.bearer_auth(token);
        }
        let resp = request.send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();

            // 偵測 OpenAI 風格 rate limit JSON
            if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&text) {
                if err_json.pointer("/error/code").and_then(|v| v.as_str())
                    == Some("rate_limit_exceeded")
                {
                    let msg = err_json
                        .pointer("/error/message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Rate limit exceeded");
                    let retry_secs = extract_retry_after_secs(msg);
                    let prefix = match retry_secs {
                        Some(s) => format!("[rate_limit_exceeded:{}]", s),
                        None => "[rate_limit_exceeded]".to_string(),
                    };
                    return Err(AiError::ProviderUnavailable(format!("{} {}", prefix, msg)));
                }
            }

            // HTTP 狀態碼分類：5xx 與 429 → ProviderUnavailable（可 fallback）
            // 4xx 其餘（400/401/403/404 等）→ ApiError（不 fallback）
            return if status.is_server_error() || status.as_u16() == 429 {
                Err(AiError::ProviderUnavailable(format!(
                    "HTTP {}: {}",
                    status, text
                )))
            } else {
                Err(AiError::ApiError(format!("HTTP {}: {}", status, text)))
            };
        }

        let chat: ChatResponse = resp.json().await?;
        chat.choices
            .into_iter()
            .next()
            .map(|c| c.message.content_text())
            .ok_or_else(|| AiError::ApiError("Empty choices".into()))
    }
}

fn bearer_token(api_key: Option<&str>) -> Option<&str> {
    api_key.and_then(|key| {
        let trimmed = key.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn chat_completion_response_format(_response_format_mode: &str) -> Option<Value> {
    None
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
    #[serde(default)]
    reasoning_content: Option<String>,
}

impl AssistantMessage {
    fn content_text(self) -> String {
        if !self.content.trim().is_empty() {
            return self.content;
        }
        self.reasoning_content.unwrap_or_default()
    }
}

#[async_trait]
impl AiClient for OpenAiClient {
    async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, AiError> {
        let messages = Self::build_messages(system_prompt, user_prompt);
        let response_format = chat_completion_response_format(&self.response_format_mode);
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
                let schema_text =
                    serde_json::to_string_pretty(schema).unwrap_or_else(|_| schema.to_string());
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

/// 從 rate limit 錯誤訊息中提取 retry-after 秒數
/// 例："Please try again in 28.5675s." → Some(29)
fn extract_retry_after_secs(msg: &str) -> Option<u32> {
    let marker = "Please try again in ";
    let start = msg.find(marker)? + marker.len();
    let rest = &msg[start..];
    let end = rest.find('s')?;
    let num_str = rest[..end].trim();
    let secs: f64 = num_str.parse().ok()?;
    Some(secs.ceil() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_retry_seconds() {
        let msg = "Rate limit. Please try again in 28.5675s. Try later.";
        assert_eq!(extract_retry_after_secs(msg), Some(29));
    }

    #[test]
    fn no_retry_marker_returns_none() {
        assert_eq!(extract_retry_after_secs("nothing here"), None);
    }

    #[test]
    fn empty_api_key_does_not_create_authorization_header() {
        assert_eq!(bearer_token(None), None);
        assert_eq!(bearer_token(Some("")), None);
        assert_eq!(bearer_token(Some("  ")), None);
    }

    #[test]
    fn non_empty_api_key_creates_authorization_header() {
        assert_eq!(bearer_token(Some("local-key")), Some("local-key"));
    }

    #[test]
    fn plain_chat_completion_does_not_force_json_response_format() {
        assert_eq!(chat_completion_response_format("strict"), None);
        assert_eq!(chat_completion_response_format("non_strict"), None);
        assert_eq!(chat_completion_response_format("inject_schema"), None);
    }

    #[test]
    fn extracts_reasoning_content_when_content_is_empty() {
        let response = json!({
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "",
                        "reasoning_content": "{\"ok\":true}"
                    }
                }
            ]
        });
        let chat: ChatResponse = serde_json::from_value(response).unwrap();
        let content = chat
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content_text())
            .unwrap();

        assert_eq!(content, "{\"ok\":true}");
    }
}
