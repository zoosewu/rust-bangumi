//! 異常通知：送到設定了「異常通知模板」（`webhooks.alert_template`）的 webhook。
//!
//! 同一件事（同一個 key）在 `REPEAT_AFTER` 內只通知一次，避免每小時的檢查一直洗版；
//! 狀況解除時（`resolve`）只在先前通知過的情況下送一次「已恢復」。
//! 記錄放在記憶體，服務重啟後會重新開始計算。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use diesel::prelude::*;

use crate::db::DbPool;
use crate::models::Webhook;
use crate::schema::webhooks;
use crate::services::webhook_service::deliver;

/// 同一件事重複通知的間隔
pub const REPEAT_AFTER: Duration = Duration::from_secs(6 * 60 * 60);

/// Discord 的預設異常通知模板（紅色嵌入），設定頁的「使用 Discord 預設格式」會填入這份
pub const DISCORD_ALERT_TEMPLATE: &str = r#"{
  "content": null,
  "embeds": [{
    "title": "{{alert_title}}",
    "description": "{{alert_message}}",
    "color": 15548997,
    "footer": {"text": "rust-bangumi · {{alert_time}}"}
  }]
}"#;

/// 每個 key 上次通知的時間
fn raised() -> &'static Mutex<HashMap<String, Instant>> {
    static RAISED: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    RAISED.get_or_init(Default::default)
}

/// 這個 key 現在要不要通知（要的話記下時間）
fn due(log: &mut HashMap<String, Instant>, key: &str, now: Instant) -> bool {
    match log.get(key) {
        Some(at) if now.duration_since(*at) < REPEAT_AFTER => false,
        _ => {
            log.insert(key.to_string(), now);
            true
        }
    }
}

/// 發生異常：同一個 key 在 `REPEAT_AFTER` 內只通知一次
pub fn raise(pool: &DbPool, key: &str, title: &str, message: &str) {
    tracing::warn!("Alert [{}] {}: {}", key, title, message);
    let send = due(
        &mut raised().lock().unwrap_or_else(|e| e.into_inner()),
        key,
        Instant::now(),
    );
    if send {
        notify(pool, title, message);
    }
}

/// 異常解除：只有先前為這個 key 通知過時才送「已恢復」
pub fn resolve(pool: &DbPool, key: &str, title: &str, message: &str) {
    let was_raised = raised()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(key)
        .is_some();
    if was_raised {
        tracing::info!("Alert [{}] resolved: {}", key, message);
        notify(pool, title, message);
    }
}

/// 背景送到每個啟用、且有異常通知模板的 webhook
fn notify(pool: &DbPool, title: &str, message: &str) {
    let targets: Vec<Webhook> = match pool.get() {
        Ok(mut conn) => match webhooks::table
            .filter(webhooks::is_active.eq(true))
            .filter(webhooks::alert_template.is_not_null())
            .load::<Webhook>(&mut conn)
        {
            Ok(list) => list,
            Err(e) => {
                tracing::error!("Failed to load alert webhooks: {}", e);
                return;
            }
        },
        Err(e) => {
            tracing::error!("Failed to load alert webhooks: {}", e);
            return;
        }
    };
    let time = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    for webhook in targets {
        let Some(template) = webhook.alert_template.filter(|t| !t.trim().is_empty()) else {
            continue;
        };
        let payload = render_alert(&template, title, message, &time);
        tokio::spawn(async move {
            deliver(client(), webhook.webhook_id, &webhook.url, payload).await;
        });
    }
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default()
    })
}

/// 代入 `{{alert_title}}`、`{{alert_message}}`、`{{alert_time}}`。
/// 值會先做 JSON 字串跳脫（模板把它們放在雙引號裡），錯誤訊息裡的引號、換行不會弄壞 JSON。
pub fn render_alert(template: &str, title: &str, message: &str, time: &str) -> String {
    let escape = |s: &str| {
        let quoted = serde_json::to_string(s).unwrap_or_default();
        quoted[1..quoted.len() - 1].to_string()
    };
    template
        .replace("{{alert_title}}", &escape(title))
        .replace("{{alert_message}}", &escape(message))
        .replace("{{alert_time}}", &escape(time))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_alert_is_sent_once_per_repeat_window() {
        let mut log = HashMap::new();
        let start = Instant::now();
        assert!(due(&mut log, "downloader:1", start));
        assert!(!due(
            &mut log,
            "downloader:1",
            start + Duration::from_secs(3600)
        ));
        assert!(due(&mut log, "downloader:2", start), "keys are independent");
        assert!(due(&mut log, "downloader:1", start + REPEAT_AFTER));
    }

    #[test]
    fn messages_are_escaped_so_the_payload_stays_valid_json() {
        let payload = render_alert(
            DISCORD_ALERT_TEMPLATE,
            "下載器無法連線",
            "Status query failed: \"403 Forbidden\"\nline two",
            "2026-10-28 10:00",
        );
        let json: serde_json::Value = serde_json::from_str(&payload).expect("valid JSON");
        let embed = &json["embeds"][0];
        assert_eq!(embed["title"], "下載器無法連線");
        assert_eq!(
            embed["description"],
            "Status query failed: \"403 Forbidden\"\nline two"
        );
        assert_eq!(embed["footer"]["text"], "rust-bangumi · 2026-10-28 10:00");
    }

    mod with_db {
        use super::super::*;
        use crate::services::identity::test_db;
        use std::sync::Arc;

        /// 假的 webhook：記下收到的每個 payload
        async fn serve() -> (String, Arc<Mutex<Vec<String>>>) {
            let received: Arc<Mutex<Vec<String>>> = Default::default();
            let store = received.clone();
            let app = axum::Router::new().route(
                "/hook",
                axum::routing::post(move |body: String| {
                    let store = store.clone();
                    async move {
                        store.lock().unwrap().push(body);
                        axum::http::StatusCode::NO_CONTENT
                    }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            (format!("http://{addr}/hook"), received)
        }

        async fn wait_for(received: &Arc<Mutex<Vec<String>>>, count: usize) {
            for _ in 0..50 {
                if received.lock().unwrap().len() >= count {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }

        #[tokio::test]
        #[ignore = "needs DATABASE_TEST_URL"]
        async fn only_webhooks_with_an_alert_template_get_alerts_once_until_resolved() {
            let (url, received) = serve().await;
            let pool = test_db::pool();
            {
                let mut conn = pool.get().unwrap();
                diesel::sql_query("UPDATE webhooks SET is_active = false")
                    .execute(&mut conn)
                    .unwrap();
                diesel::sql_query(format!(
                    "INSERT INTO webhooks (name, url, payload_template, is_active, alert_template, created_at, updated_at) VALUES \
                     ('alerts', '{url}', '{{}}', true, '{{\"t\":\"{{{{alert_title}}}}\",\"m\":\"{{{{alert_message}}}}\"}}', now(), now()), \
                     ('downloads only', '{url}', '{{}}', true, NULL, now(), now())"
                ))
                .execute(&mut conn)
                .unwrap();
            }

            let key = "test:only-webhooks-with-an-alert-template";
            raise(&pool, key, "下載器無法查詢", "403 \"Forbidden\"");
            raise(&pool, key, "下載器無法查詢", "403 again");
            wait_for(&received, 1).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert_eq!(
                received.lock().unwrap().len(),
                1,
                "sent once, to one webhook"
            );

            resolve(&pool, key, "下載器已恢復", "ok");
            wait_for(&received, 2).await;
            resolve(&pool, key, "下載器已恢復", "ok");
            tokio::time::sleep(Duration::from_millis(100)).await;

            let got = received.lock().unwrap().clone();
            assert_eq!(got.len(), 2, "a second resolve sends nothing");
            let first: serde_json::Value = serde_json::from_str(&got[0]).unwrap();
            assert_eq!(first["m"], "403 \"Forbidden\"");
            let second: serde_json::Value = serde_json::from_str(&got[1]).unwrap();
            assert_eq!(second["t"], "下載器已恢復");
        }
    }
}
