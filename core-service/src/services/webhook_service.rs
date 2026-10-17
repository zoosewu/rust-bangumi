use crate::db::DbPool;
use crate::models::Webhook;
use crate::schema::webhooks;
use diesel::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 模板渲染所需的動畫下載上下文
pub struct WebhookContext {
    pub download_id: i32,
    pub anime_id: i32,
    pub anime_title: String,
    pub episode_no: i32,
    pub series_no: i32,
    pub subtitle_group: String,
    pub video_path: String,
}

/// Attempts per notification before giving up on a webhook that keeps answering 429.
const MAX_ATTEMPTS: u32 = 5;
/// Wait used when a 429 does not say how long to back off.
const DEFAULT_RETRY_WAIT: Duration = Duration::from_secs(1);
/// Upper bound on a single back-off, so one notification cannot stall the queue for long.
const MAX_RETRY_WAIT: Duration = Duration::from_secs(60);

/// How long a 429 asks us to wait: Discord puts `retry_after` (seconds, fractional) in the
/// JSON body; the standard `Retry-After` header (whole seconds) is the fallback.
fn retry_wait(retry_after_header: Option<&str>, body: &str) -> Duration {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("retry_after")?.as_f64())
        .or_else(|| retry_after_header?.trim().parse::<f64>().ok())
        .and_then(|secs| Duration::try_from_secs_f64(secs).ok())
        .unwrap_or(DEFAULT_RETRY_WAIT)
        .min(MAX_RETRY_WAIT)
}

/// Send one notification, retrying while the webhook is rate limited.
/// Only the webhook id is logged: the URL carries the webhook's secret token.
async fn deliver(client: &reqwest::Client, webhook_id: i32, url: &str, payload: String) {
    for attempt in 1..=MAX_ATTEMPTS {
        let resp = match client
            .post(url)
            .header("Content-Type", "application/json")
            .body(payload.clone())
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                tracing::warn!("Webhook {} failed to fire: {}", webhook_id, e.without_url());
                return;
            }
        };

        if resp.status() != reqwest::StatusCode::TOO_MANY_REQUESTS {
            tracing::info!("Webhook {} fired: status {}", webhook_id, resp.status());
            return;
        }

        let header = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = resp.text().await.unwrap_or_default();
        let wait = retry_wait(header.as_deref(), &body);
        tracing::warn!(
            "Webhook {} rate limited (attempt {}/{}), retrying in {:?}",
            webhook_id,
            attempt,
            MAX_ATTEMPTS,
            wait
        );
        if attempt < MAX_ATTEMPTS {
            tokio::time::sleep(wait).await;
        }
    }
    tracing::error!(
        "Webhook {} still rate limited after {} attempts, notification dropped",
        webhook_id,
        MAX_ATTEMPTS
    );
}

pub struct WebhookService {
    db_pool: DbPool,
    http_client: reqwest::Client,
    /// One queue per webhook: a batch of syncs finishing together would otherwise hit
    /// the webhook all at once and get most of its notifications rate limited.
    queues: Mutex<HashMap<i32, Arc<tokio::sync::Mutex<()>>>>,
}

impl WebhookService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap(),
            queues: Mutex::new(HashMap::new()),
        }
    }

    /// 載入所有啟用的 webhook，逐一渲染模板並發送（fire-and-forget）。
    /// 此方法應以 tokio::spawn 包裹，不阻塞主流程。
    pub async fn fire(&self, ctx: WebhookContext) {
        let webhooks = match self.load_active_webhooks() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to load webhooks: {}", e);
                return;
            }
        };

        for webhook in webhooks {
            let payload = render_template(&webhook.payload_template, &ctx);
            let client = self.http_client.clone();
            let queue = self.queue_for(webhook.webhook_id);

            tokio::spawn(async move {
                let _turn = queue.lock().await;
                deliver(&client, webhook.webhook_id, &webhook.url, payload).await;
            });
        }
    }

    fn queue_for(&self, webhook_id: i32) -> Arc<tokio::sync::Mutex<()>> {
        self.queues
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(webhook_id)
            .or_default()
            .clone()
    }

    fn load_active_webhooks(&self) -> Result<Vec<Webhook>, String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;
        webhooks::table
            .filter(webhooks::is_active.eq(true))
            .load::<Webhook>(&mut conn)
            .map_err(|e| e.to_string())
    }
}

/// 將 `{{variable}}` 佔位符替換為對應值。
/// 數字型變數直接插入，字串型變數不加引號（由模板作者自行決定格式）。
pub fn render_template(template: &str, ctx: &WebhookContext) -> String {
    template
        .replace("{{download_id}}", &ctx.download_id.to_string())
        .replace("{{anime_id}}", &ctx.anime_id.to_string())
        .replace("{{anime_title}}", &ctx.anime_title)
        .replace("{{episode_no}}", &ctx.episode_no.to_string())
        .replace("{{series_no}}", &ctx.series_no.to_string())
        .replace("{{subtitle_group}}", &ctx.subtitle_group)
        .replace("{{video_path}}", &ctx.video_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> WebhookContext {
        WebhookContext {
            download_id: 42,
            anime_id: 7,
            anime_title: "進擊的巨人".to_string(),
            episode_no: 3,
            series_no: 2,
            subtitle_group: "字幕組A".to_string(),
            video_path: "/downloads/ep03.mkv".to_string(),
        }
    }

    #[test]
    fn renders_all_variables() {
        let template = r#"{"id":{{download_id}},"title":"{{anime_title}}","ep":{{episode_no}}}"#;
        let result = render_template(template, &make_ctx());
        assert_eq!(result, r#"{"id":42,"title":"進擊的巨人","ep":3}"#);
    }

    #[test]
    fn renders_series_no_and_subtitle_group() {
        let template = "S{{series_no}}E{{episode_no}} - {{subtitle_group}}";
        let result = render_template(template, &make_ctx());
        assert_eq!(result, "S2E3 - 字幕組A");
    }

    #[test]
    fn renders_video_path() {
        let template = "{{video_path}}";
        let result = render_template(template, &make_ctx());
        assert_eq!(result, "/downloads/ep03.mkv");
    }

    #[test]
    fn unknown_placeholder_left_intact() {
        let template = "{{unknown}} {{download_id}}";
        let result = render_template(template, &make_ctx());
        assert_eq!(result, "{{unknown}} 42");
    }

    #[test]
    fn retry_wait_prefers_discord_body() {
        let body = r#"{"message":"You are being rate limited.","retry_after":0.35,"global":false}"#;
        assert_eq!(retry_wait(Some("1"), body), Duration::from_millis(350));
    }

    #[test]
    fn retry_wait_falls_back_to_header_then_default() {
        assert_eq!(retry_wait(Some("3"), "not json"), Duration::from_secs(3));
        assert_eq!(retry_wait(None, ""), DEFAULT_RETRY_WAIT);
        assert_eq!(retry_wait(Some("-1"), ""), DEFAULT_RETRY_WAIT);
    }

    #[test]
    fn retry_wait_is_capped() {
        assert_eq!(retry_wait(None, r#"{"retry_after":3600}"#), MAX_RETRY_WAIT);
    }

    /// Mock webhook answering 429 `rate_limited_times` times, then 204.
    async fn spawn_mock_webhook(
        rate_limited_times: u32,
    ) -> (String, Arc<std::sync::atomic::AtomicU32>) {
        use axum::{extract::State, http::StatusCode, routing::post, Router};
        use std::sync::atomic::{AtomicU32, Ordering};

        let calls = Arc::new(AtomicU32::new(0));
        let app = Router::new()
            .route(
                "/hook",
                post(move |State(calls): State<Arc<AtomicU32>>| async move {
                    if calls.fetch_add(1, Ordering::SeqCst) < rate_limited_times {
                        (StatusCode::TOO_MANY_REQUESTS, r#"{"retry_after":0.01}"#)
                    } else {
                        (StatusCode::NO_CONTENT, "")
                    }
                }),
            )
            .with_state(calls.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{}/hook", addr), calls)
    }

    #[tokio::test]
    async fn rate_limited_notification_is_retried_until_accepted() {
        let (url, calls) = spawn_mock_webhook(2).await;
        deliver(&reqwest::Client::new(), 1, &url, "{}".to_string()).await;
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn gives_up_after_max_attempts() {
        let (url, calls) = spawn_mock_webhook(u32::MAX).await;
        deliver(&reqwest::Client::new(), 1, &url, "{}".to_string()).await;
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            MAX_ATTEMPTS
        );
    }
}
