use crate::db::DbPool;
use crate::schema::webhooks;
use crate::models::Webhook;
use diesel::prelude::*;

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

pub struct WebhookService {
    db_pool: DbPool,
    http_client: reqwest::Client,
}

impl WebhookService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap(),
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
            let url = webhook.url.clone();
            let client = self.http_client.clone();
            let webhook_id = webhook.webhook_id;

            tokio::spawn(async move {
                match client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .body(payload)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        tracing::info!(
                            "Webhook {} fired to {}: status {}",
                            webhook_id,
                            url,
                            resp.status()
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Webhook {} failed to fire to {}: {}",
                            webhook_id,
                            url,
                            e
                        );
                    }
                }
            });
        }
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
}
