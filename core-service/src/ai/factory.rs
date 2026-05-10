use super::client::AiClient;
use super::openai::OpenAiClient;
use crate::models::AiProvider;

pub fn build_provider(p: &AiProvider) -> Result<Box<dyn AiClient>, String> {
    match p.provider_kind.as_str() {
        "openai_compatible" => Ok(Box::new(OpenAiClient::new(
            &p.base_url,
            &p.api_key,
            &p.model_name,
            p.max_tokens,
            &p.response_format_mode,
        ))),
        other => Err(format!("unknown provider_kind: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn provider(kind: &str) -> AiProvider {
        AiProvider {
            id: 1,
            name: "x".into(),
            provider_kind: kind.into(),
            base_url: "https://example.com".into(),
            api_key: "k".into(),
            model_name: "m".into(),
            max_tokens: 4096,
            response_format_mode: "non_strict".into(),
            is_enabled: true,
            priority: 0,
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
        }
    }

    #[test]
    fn openai_compatible_builds() {
        assert!(build_provider(&provider("openai_compatible")).is_ok());
    }

    #[test]
    fn unknown_kind_errors() {
        match build_provider(&provider("anthropic")) {
            Err(e) => assert!(e.contains("anthropic")),
            Ok(_) => panic!("expected error for unknown provider_kind"),
        }
    }
}
