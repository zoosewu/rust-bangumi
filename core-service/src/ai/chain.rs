use crate::models::AiProvider;
use crate::schema::ai_providers;
use diesel::prelude::*;
use serde_json::Value;

use super::client::{AiClient, AiError};
use super::factory::build_provider;
use super::queue::ai_queue;
use std::time::Duration;

/// 一輪全部失敗、且有 provider 限流時最多重試幾次
const MAX_RATE_LIMIT_RETRIES: u32 = 5;
/// provider 沒說要等多久時的等待時間
const DEFAULT_RATE_LIMIT_WAIT: Duration = Duration::from_secs(30);
/// 單次等待上限
const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_secs(120);

/// 限流後的等待時間：provider 指定的秒數多等 1 秒，未指定則用預設值，最多 `MAX_RATE_LIMIT_WAIT`。
fn rate_limit_wait(retry_after_secs: Option<u32>) -> Duration {
    retry_after_secs
        .map(|secs| Duration::from_secs(u64::from(secs) + 1))
        .unwrap_or(DEFAULT_RATE_LIMIT_WAIT)
        .min(MAX_RATE_LIMIT_WAIT)
}

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

    /// 排進全域 AI 佇列後依序嘗試各 provider。一輪全部失敗且其中有 provider 限流時，佔著佇列名額
    /// 等 provider 指定的時間再重跑一輪（最多 `MAX_RATE_LIMIT_RETRIES` 次）：放掉名額
    /// 只會讓其他請求也撞上同一個額度。
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
        let _permit = ai_queue().acquire().await;
        let mut attempts: Vec<AttemptRecord> = Vec::new();
        let mut retries = 0;
        loop {
            match self.try_providers(&op, &mut attempts).await {
                Ok(resp) => return Ok((resp, attempts)),
                Err(AiError::RateLimited {
                    retry_after_secs, ..
                }) if retries < MAX_RATE_LIMIT_RETRIES => {
                    retries += 1;
                    let wait = rate_limit_wait(retry_after_secs);
                    tracing::warn!(
                        retry = retries,
                        wait_secs = wait.as_secs(),
                        "All AI providers failed with a rate limit among them, retrying after wait"
                    );
                    tokio::time::sleep(wait).await;
                }
                Err(e) => return Err((e, attempts)),
            }
        }
    }

    /// 依序嘗試各 provider 一輪；任何一家失敗都換下一家（設定錯誤只影響那一家）。
    /// 全部失敗時：其中有任何一家是限流 → 回傳 `RateLimited`（等待時間取限流各家中最短的），
    /// 讓呼叫端等額度恢復後再跑一輪；否則回傳 `ProviderUnavailable`。錯誤訊息列出每一家的錯誤。
    async fn try_providers<'a, F, Fut>(
        &'a self,
        op: &F,
        attempts: &mut Vec<AttemptRecord>,
    ) -> Result<String, AiError>
    where
        F: Fn(&'a dyn AiClient) -> Fut,
        Fut: std::future::Future<Output = Result<String, AiError>>,
    {
        let mut rate_limits: Vec<Option<u32>> = Vec::new();
        let mut failures: Vec<String> = Vec::new();
        for entry in &self.entries {
            let error = match op(entry.client.as_ref()).await {
                Ok(resp) if !resp.trim().is_empty() => return Ok(resp),
                Ok(_) => AiError::ProviderUnavailable("empty response".into()),
                Err(e) => e,
            };
            tracing::warn!(
                provider_id = entry.id,
                provider = %entry.name,
                error = %error,
                "AI provider failed, falling back"
            );
            if let AiError::RateLimited {
                retry_after_secs, ..
            } = &error
            {
                rate_limits.push(*retry_after_secs);
            }
            failures.push(format!("{}: {}", entry.name, error));
            attempts.push(AttemptRecord {
                provider_id: entry.id,
                provider_name: entry.name.clone(),
                error: error.to_string(),
            });
        }
        let summary = failures.join("; ");
        if rate_limits.is_empty() {
            Err(AiError::ProviderUnavailable(format!(
                "all providers failed: {summary}"
            )))
        } else {
            Err(AiError::RateLimited {
                retry_after_secs: rate_limits.into_iter().flatten().min(),
                message: format!("all providers failed: {summary}"),
            })
        }
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
        .filter(|p| !p.base_url.is_empty() && !p.model_name.is_empty())
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
            entry(
                1,
                "a",
                vec![Err(AiError::ProviderUnavailable("503".into()))],
            ),
            entry(2, "b", vec![Ok("ok".into())]),
        ]);
        let (resp, attempts) = chain.chat_completion("s", "u").await.unwrap();
        assert_eq!(resp, "ok");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].provider_name, "a");
    }

    #[tokio::test]
    async fn falls_back_on_empty_response() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Ok("".into())]),
            entry(2, "b", vec![Ok("{\"ok\":true}".into())]),
        ]);

        let (resp, attempts) = chain
            .chat_completion_structured("s", "u", &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(resp, "{\"ok\":true}");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].provider_name, "a");
        assert!(attempts[0].error.contains("empty response"));
    }

    #[tokio::test]
    async fn all_retryable_fail() {
        let chain = AiProviderChain::new(vec![
            entry(
                1,
                "a",
                vec![Err(AiError::ProviderUnavailable("503".into()))],
            ),
            entry(
                2,
                "b",
                vec![Err(AiError::ProviderUnavailable("502".into()))],
            ),
        ]);
        let (err, attempts) = unwrap_err_chain(chain.chat_completion("s", "u").await);
        assert!(matches!(err, AiError::ProviderUnavailable(_)));
        assert_eq!(attempts.len(), 2);
    }

    #[tokio::test]
    async fn a_misconfigured_provider_falls_back_to_the_next() {
        // 例如 key 錯（401）、模型不在方案內（403）、max_tokens 太大（400）
        let chain = AiProviderChain::new(vec![
            entry(
                1,
                "a",
                vec![Err(AiError::ApiError("HTTP 400: max_tokens".into()))],
            ),
            entry(2, "b", vec![Ok("ok".into())]),
        ]);
        let (resp, attempts) = chain.chat_completion("s", "u").await.unwrap();
        assert_eq!(resp, "ok");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].provider_name, "a");
    }

    #[tokio::test(start_paused = true)]
    async fn failures_without_a_rate_limit_end_at_once_naming_every_provider() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![Err(AiError::ApiError("HTTP 401".into()))]),
            entry(
                2,
                "b",
                vec![Err(AiError::ProviderUnavailable("HTTP 503".into()))],
            ),
        ]);
        let started = tokio::time::Instant::now();
        let (err, attempts) = unwrap_err_chain(chain.chat_completion("s", "u").await);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(attempts.len(), 2);
        let message = err.to_string();
        assert!(message.contains("a: provider error: HTTP 401"), "{message}");
        assert!(
            message.contains("b: provider unavailable: HTTP 503"),
            "{message}"
        );
    }

    fn rate_limited(secs: Option<u32>) -> Result<String, AiError> {
        Err(AiError::RateLimited {
            retry_after_secs: secs,
            message: "TPM".into(),
        })
    }

    #[test]
    fn rate_limit_wait_follows_the_provider_with_a_margin() {
        assert_eq!(rate_limit_wait(Some(31)), Duration::from_secs(32));
        assert_eq!(rate_limit_wait(None), DEFAULT_RATE_LIMIT_WAIT);
        assert_eq!(rate_limit_wait(Some(600)), MAX_RATE_LIMIT_WAIT);
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limited_request_is_retried_after_the_wait() {
        let chain = AiProviderChain::new(vec![entry(
            1,
            "a",
            vec![rate_limited(Some(31)), Ok("ok".into())],
        )]);
        let started = tokio::time::Instant::now();
        let (resp, attempts) = chain.chat_completion("s", "u").await.unwrap();
        assert_eq!(resp, "ok");
        assert_eq!(attempts.len(), 1);
        assert!(started.elapsed() >= Duration::from_secs(32));
    }

    #[tokio::test(start_paused = true)]
    async fn waits_for_the_provider_that_frees_up_first() {
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![rate_limited(Some(50)), Ok("a".into())]),
            entry(2, "b", vec![rate_limited(Some(10))]),
        ]);
        let started = tokio::time::Instant::now();
        let (resp, _) = chain.chat_completion("s", "u").await.unwrap();
        assert_eq!(resp, "a");
        assert!(started.elapsed() < Duration::from_secs(50));
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_after_the_retry_limit() {
        let results = (0..=MAX_RATE_LIMIT_RETRIES)
            .map(|_| rate_limited(Some(1)))
            .chain([Ok("too late".into())])
            .collect();
        let chain = AiProviderChain::new(vec![entry(1, "a", results)]);
        let (err, attempts) = unwrap_err_chain(chain.chat_completion("s", "u").await);
        assert!(err.to_string().starts_with("[rate_limit_exceeded:1]"));
        assert_eq!(attempts.len(), MAX_RATE_LIMIT_RETRIES as usize + 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_round_with_any_rate_limit_waits_and_tries_again() {
        // a 限流、b 掛掉：等 a 的額度恢復後再跑一輪，a 成功
        let chain = AiProviderChain::new(vec![
            entry(1, "a", vec![rate_limited(Some(5)), Ok("a".into())]),
            entry(
                2,
                "b",
                vec![Err(AiError::ProviderUnavailable("503".into()))],
            ),
        ]);
        let started = tokio::time::Instant::now();
        let (resp, attempts) = chain.chat_completion("s", "u").await.unwrap();
        assert_eq!(resp, "a");
        assert_eq!(attempts.len(), 2);
        assert!(started.elapsed() >= Duration::from_secs(6));
    }
}
