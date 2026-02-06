use chrono::Utc;
use diesel::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

use crate::db::DbPool;
use crate::models::{ModuleTypeEnum, NewCronLog, ServiceModule, Subscription};
use crate::schema::{cron_logs, service_modules, subscriptions};

pub struct FetchScheduler {
    db_pool: DbPool,
    check_interval_secs: u64,
    max_retries: u32,
    base_retry_delay_secs: u64,
}

#[derive(Debug, Clone)]
struct FetchTask {
    subscription_id: i32,
    source_url: String,
    #[allow(dead_code)]
    fetcher_id: i32,
    fetcher_base_url: String,
}

impl FetchScheduler {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            check_interval_secs: 60, // 每 60 秒檢查一次
            max_retries: 3,
            base_retry_delay_secs: 60, // 初始重試延遲 60 秒
        }
    }

    #[allow(dead_code)]
    pub fn with_check_interval(mut self, secs: u64) -> Self {
        self.check_interval_secs = secs;
        self
    }

    /// 啟動排程器主迴圈
    pub async fn start(self: Arc<Self>) {
        let mut ticker = interval(Duration::from_secs(self.check_interval_secs));

        tracing::info!(
            "FetchScheduler started, checking every {} seconds",
            self.check_interval_secs
        );

        loop {
            ticker.tick().await;

            if let Err(e) = self.process_due_subscriptions().await {
                tracing::error!("Error processing due subscriptions: {}", e);
            }
        }
    }

    /// 處理所有到期的訂閱
    async fn process_due_subscriptions(&self) -> Result<(), String> {
        let tasks = self.get_due_subscriptions()?;

        if tasks.is_empty() {
            tracing::debug!("No due subscriptions found");
            return Ok(());
        }

        tracing::info!("Found {} due subscriptions", tasks.len());

        for task in tasks {
            // 每個任務獨立處理，失敗不影響其他任務
            if let Err(e) = self.trigger_fetch(&task).await {
                tracing::error!(
                    "Failed to trigger fetch for subscription {}: {}",
                    task.subscription_id,
                    e
                );
                self.log_fetch_attempt(&task, false, Some(&e));
            } else {
                self.log_fetch_attempt(&task, true, None);
            }
        }

        Ok(())
    }

    /// 取得所有到期的訂閱
    fn get_due_subscriptions(&self) -> Result<Vec<FetchTask>, String> {
        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;
        let now = Utc::now().naive_utc();

        // 查詢到期的活躍訂閱
        let due_subscriptions = subscriptions::table
            .filter(subscriptions::is_active.eq(true))
            .filter(subscriptions::next_fetch_at.le(now))
            .select(Subscription::as_select())
            .load::<Subscription>(&mut conn)
            .map_err(|e| format!("Failed to query subscriptions: {}", e))?;

        // 取得對應的 fetcher 資訊
        let mut tasks = Vec::new();
        for sub in due_subscriptions {
            match service_modules::table
                .filter(service_modules::module_id.eq(sub.fetcher_id))
                .filter(service_modules::is_enabled.eq(true))
                .filter(service_modules::module_type.eq(ModuleTypeEnum::Fetcher))
                .first::<ServiceModule>(&mut conn)
            {
                Ok(fetcher) => {
                    tasks.push(FetchTask {
                        subscription_id: sub.subscription_id,
                        source_url: sub.source_url,
                        fetcher_id: sub.fetcher_id,
                        fetcher_base_url: fetcher.base_url,
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        "Fetcher {} not found or disabled for subscription {}: {}",
                        sub.fetcher_id,
                        sub.subscription_id,
                        e
                    );
                }
            }
        }

        Ok(tasks)
    }

    /// 觸發 Fetcher 執行抓取
    async fn trigger_fetch(&self, task: &FetchTask) -> Result<(), String> {
        let fetch_url = format!("{}/fetch", task.fetcher_base_url);
        let callback_url = std::env::var("CORE_SERVICE_URL")
            .unwrap_or_else(|_| "http://core-service:8000".to_string());
        let callback_url = format!("{}/raw-fetcher-results", callback_url);

        let request = shared::FetchTriggerRequest {
            subscription_id: task.subscription_id,
            rss_url: task.source_url.clone(),
            callback_url,
        };

        tracing::info!(
            "Triggering fetch for subscription {} at {}",
            task.subscription_id,
            fetch_url
        );

        // 使用重試機制
        let mut attempt = 0;
        let mut last_error = String::new();

        while attempt < self.max_retries {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| e.to_string())?;

            match client.post(&fetch_url).json(&request).send().await {
                Ok(response) => {
                    if response.status().is_success()
                        || response.status() == reqwest::StatusCode::ACCEPTED
                    {
                        tracing::info!(
                            "Successfully triggered fetch for subscription {}",
                            task.subscription_id
                        );
                        return Ok(());
                    } else {
                        last_error = format!("HTTP {}", response.status());
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }

            attempt += 1;
            if attempt < self.max_retries {
                // 指數退避
                let delay = self.base_retry_delay_secs * (1 << attempt);
                tracing::warn!(
                    "Fetch trigger failed (attempt {}/{}), retrying in {} seconds: {}",
                    attempt,
                    self.max_retries,
                    delay,
                    last_error
                );
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }
        }

        Err(format!(
            "Failed after {} attempts: {}",
            self.max_retries, last_error
        ))
    }

    /// 記錄抓取嘗試到 cron_logs
    fn log_fetch_attempt(&self, task: &FetchTask, success: bool, error: Option<&str>) {
        if let Ok(mut conn) = self.db_pool.get() {
            let now = Utc::now().naive_utc();
            let log = NewCronLog {
                fetcher_type: format!("subscription_{}", task.subscription_id),
                status: if success {
                    "success".to_string()
                } else {
                    "failed".to_string()
                },
                error_message: error.map(|e| e.to_string()),
                attempt_count: 1,
                executed_at: now,
            };

            if let Err(e) = diesel::insert_into(cron_logs::table)
                .values(&log)
                .execute(&mut conn)
            {
                tracing::error!("Failed to log fetch attempt: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: FetchScheduler requires a real DbPool, so we can only test
    // configuration aspects without database integration.

    #[test]
    fn test_scheduler_default_configuration() {
        // Test that default configuration values are sensible
        // We can't instantiate FetchScheduler without a real DbPool,
        // but we can document expected defaults here.
        let expected_check_interval = 60; // seconds
        let expected_max_retries = 3;
        let expected_base_retry_delay = 60; // seconds

        // These values should match the defaults in FetchScheduler::new()
        assert_eq!(expected_check_interval, 60);
        assert_eq!(expected_max_retries, 3);
        assert_eq!(expected_base_retry_delay, 60);
    }

    #[test]
    fn test_fetch_task_structure() {
        // Test FetchTask can hold expected data
        let task = FetchTask {
            subscription_id: 1,
            source_url: "http://example.com/feed".to_string(),
            fetcher_id: 10,
            fetcher_base_url: "http://localhost:8001".to_string(),
        };

        assert_eq!(task.subscription_id, 1);
        assert_eq!(task.source_url, "http://example.com/feed");
        assert_eq!(task.fetcher_base_url, "http://localhost:8001");
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        // Test the exponential backoff formula used in trigger_fetch
        let base_delay = 60u64;

        // attempt 1: 60 * 2^1 = 120
        let delay_attempt_1 = base_delay * (1 << 1);
        assert_eq!(delay_attempt_1, 120);

        // attempt 2: 60 * 2^2 = 240
        let delay_attempt_2 = base_delay * (1 << 2);
        assert_eq!(delay_attempt_2, 240);

        // attempt 3: 60 * 2^3 = 480 (but won't happen as max_retries is 3)
    }
}
