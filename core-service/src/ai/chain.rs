use crate::models::AiProvider;
use crate::schema::ai_providers;
use diesel::prelude::*;
use serde_json::Value;

use super::client::{AiClient, AiError};
use super::factory::build_provider;

pub struct ChainEntry {
    pub id: i32,
    pub name: String,
    pub client: Box<dyn AiClient>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AttemptRecord {
    pub provider_id: i32,
    pub provider_name: String,
    pub error: String,
}

pub struct AiProviderChain {
    entries: Vec<ChainEntry>,
}

impl AiProviderChain {
    pub fn new(entries: Vec<ChainEntry>) -> Self {
        Self { entries }
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    async fn run<'a, F, Fut>(
        &'a self,
        op: F,
    ) -> Result<(String, Vec<AttemptRecord>), (AiError, Vec<AttemptRecord>)>
    where
        F: Fn(&'a dyn AiClient) -> Fut,
        Fut: std::future::Future<Output = Result<String, AiError>>,
    {
        if self.entries.is_empty() {
            return Err((AiError::NotConfigured, vec![]));
        }
        let mut attempts: Vec<AttemptRecord> = Vec::new();
        for entry in &self.entries {
            match op(entry.client.as_ref()).await {
                Ok(resp) => return Ok((resp, attempts)),
                Err(e) if e.is_retryable() => {
                    tracing::warn!(
                        provider_id = entry.id,
                        provider = %entry.name,
                        error = %e,
                        "AI provider failed, falling back"
                    );
                    attempts.push(AttemptRecord {
                        provider_id: entry.id,
                        provider_name: entry.name.clone(),
                        error: e.to_string(),
                    });
                }
                Err(e) => {
                    attempts.push(AttemptRecord {
                        provider_id: entry.id,
                        provider_name: entry.name.clone(),
                        error: e.to_string(),
                    });
                    return Err((e, attempts));
                }
            }
        }
        let last = attempts.last().map(|a| a.error.clone()).unwrap_or_default();
        Err((
            AiError::ProviderUnavailable(format!("all providers failed: {last}")),
            attempts,
        ))
    }

    pub async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<(String, Vec<AttemptRecord>), (AiError, Vec<AttemptRecord>)> {
        self.run(|c| c.chat_completion(system_prompt, user_prompt))
            .await
    }

    pub async fn chat_completion_structured(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &Value,
    ) -> Result<(String, Vec<AttemptRecord>), (AiError, Vec<AttemptRecord>)> {
        self.run(|c| c.chat_completion_structured(system_prompt, user_prompt, schema))
            .await
    }
}

pub fn build_ai_chain(conn: &mut PgConnection) -> Result<Option<AiProviderChain>, String> {
    let providers = ai_providers::table
        .filter(ai_providers::is_enabled.eq(true))
        .order(ai_providers::priority.asc())
        .then_order_by(ai_providers::id.asc())
        .load::<AiProvider>(conn)
        .map_err(|e| e.to_string())?;

    let entries: Vec<ChainEntry> = providers
        .into_iter()
        .filter(|p| !p.api_key.is_empty() && !p.base_url.is_empty())
        .map(|p| {
            build_provider(&p).map(|client| ChainEntry {
                id: p.id,
                name: p.name.clone(),
                client,
            })
        })
        .collect::<Result<_, _>>()?;

    Ok((!entries.is_empty()).then_some(AiProviderChain::new(entries)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Mock client that returns predetermined results in order.
    struct MockAiClient {
        results: Mutex<std::vec::IntoIter<Result<String, AiError>>>,
    }

    impl MockAiClient {
        fn new(results: Vec<Result<String, AiError>>) -> Self {
            Self {
                results: Mutex::new(results.into_iter()),
            }
        }
    }

    #[async_trait]
    impl AiClient for MockAiClient {
        async fn chat_completion(&self, _: &str, _: &str) -> Result<String, AiError> {
            self.results
                .lock()
                .unwrap()
                .next()
                .unwrap_or(Err(AiError::ApiError("exhausted".into())))
        }
    }

    fn entry(id: i32, name: &str, results: Vec<Result<String, AiError>>) -> ChainEntry {
        ChainEntry {
            id,
            name: name.into(),
            client: Box::new(MockAiClient::new(results)),
        }
    }

    fn unwrap_err_chain(
        r: Result<(String, Vec<AttemptRecord>), (AiError, Vec<AttemptRecord>)>,
    ) -> (AiError, Vec<AttemptRecord>) {
        match r {
            Err(e) => e,
            Ok(_) => panic!("expected chain error, got Ok"),
        }
    }

    #[tokio::test]
    async fn empty_chain_returns_not_configured() {
        let chain = AiProviderChain::new(vec![]);
        let (err, attempts) = unwrap_err_chain(chain.chat_completion("s", "u").await);
        assert!(matches!(err, AiError::NotConfigured));
        assert!(attempts.is_empty());
    }

    #[tokio::test]
    async fn first_provider_succeeds() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Ok("hello".into())]),
            entry(2, "b", vec![Ok("ignored".into())]),
        ]);
        let (resp, attempts) = chain.chat_completion("s", "u").await.unwrap();
        assert_eq!(resp, "hello");
        assert!(attempts.is_empty());
    }

    #[tokio::test]
    async fn falls_back_on_retryable() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Err(AiError::ProviderUnavailable("503".into()))]),
            entry(2, "b", vec![Ok("ok".into())]),
        ]);
        let (resp, attempts) = chain.chat_completion("s", "u").await.unwrap();
        assert_eq!(resp, "ok");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].provider_name, "a");
    }

    #[tokio::test]
    async fn all_retryable_fail() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Err(AiError::ProviderUnavailable("503".into()))]),
            entry(2, "b", vec![Err(AiError::ProviderUnavailable("502".into()))]),
        ]);
        let (err, attempts) = unwrap_err_chain(chain.chat_completion("s", "u").await);
        assert!(matches!(err, AiError::ProviderUnavailable(_)));
        assert_eq!(attempts.len(), 2);
    }

    #[tokio::test]
    async fn non_retryable_stops_immediately() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Err(AiError::ApiError("401".into()))]),
            entry(2, "b", vec![Ok("never".into())]),
        ]);
        let (err, attempts) = unwrap_err_chain(chain.chat_completion("s", "u").await);
        assert!(matches!(err, AiError::ApiError(_)));
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].provider_name, "a");
    }
}
