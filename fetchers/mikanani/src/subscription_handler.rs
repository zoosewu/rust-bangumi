use std::sync::Arc;
use crate::RssParser;
use tokio::sync::Mutex;
use std::collections::VecDeque;

/// Subscription payload received from core service
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubscriptionBroadcastPayload {
    pub rss_url: String,
    pub service_name: String,
}

/// Pending subscription entry
#[derive(Debug, Clone)]
pub struct PendingSubscription {
    pub rss_url: String,
    pub service_name: String,
}

/// Handles subscriptions for the Mikanani fetcher
/// Validates URLs and manages pending subscriptions
pub struct SubscriptionHandler {
    parser: Arc<RssParser>,
    pending_subscriptions: Arc<Mutex<VecDeque<PendingSubscription>>>,
}

impl SubscriptionHandler {
    /// Create a new subscription handler
    pub fn new(parser: Arc<RssParser>) -> Self {
        Self {
            parser,
            pending_subscriptions: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Check if this handler can handle the given URL
    /// Returns true if URL contains "mikanani.me"
    pub fn can_handle_url(&self, url: &str) -> bool {
        url.contains("mikanani.me")
    }

    /// Register subscription with core service
    pub async fn register_subscription_with_core(&self, payload: &SubscriptionBroadcastPayload) -> anyhow::Result<()> {
        let core_service_url = std::env::var("CORE_SERVICE_URL")
            .unwrap_or_else(|_| "http://core-service:8000".to_string());

        // Validate the URL
        if !self.can_handle_url(&payload.rss_url) {
            return Err(anyhow::anyhow!("URL does not contain mikanani.me"));
        }

        // Register with core service
        let client = reqwest::Client::new();
        client
            .post(&format!("{}/subscriptions/register", core_service_url))
            .json(payload)
            .send()
            .await?;

        tracing::info!("Successfully registered subscription for: {}", payload.rss_url);

        Ok(())
    }

    /// Add a pending subscription to the queue
    pub async fn add_pending_subscription(&self, payload: SubscriptionBroadcastPayload) -> anyhow::Result<()> {
        if !self.can_handle_url(&payload.rss_url) {
            return Err(anyhow::anyhow!("URL does not contain mikanani.me"));
        }

        let subscription = PendingSubscription {
            rss_url: payload.rss_url.clone(),
            service_name: payload.service_name.clone(),
        };

        let mut subscriptions = self.pending_subscriptions.lock().await;
        subscriptions.push_back(subscription);

        tracing::info!("Added pending subscription: {}", payload.rss_url);

        Ok(())
    }

    /// Get and clear all pending subscriptions
    pub async fn get_and_clear_pending(&self) -> Vec<PendingSubscription> {
        let mut subscriptions = self.pending_subscriptions.lock().await;
        subscriptions.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_mikanani_url() {
        let parser = Arc::new(RssParser::new());
        let handler = SubscriptionHandler::new(parser);

        assert!(handler.can_handle_url("https://mikanani.me/rss/bangumi"));
        assert!(handler.can_handle_url("http://mikanani.me/rss"));
        assert!(!handler.can_handle_url("https://example.com/rss"));
    }

    #[tokio::test]
    async fn test_add_pending_subscription() {
        let parser = Arc::new(RssParser::new());
        let handler = SubscriptionHandler::new(parser);

        let payload = SubscriptionBroadcastPayload {
            rss_url: "https://mikanani.me/rss/bangumi".to_string(),
            service_name: "mikanani".to_string(),
        };

        let result = handler.add_pending_subscription(payload).await;
        assert!(result.is_ok());

        let pending = handler.get_and_clear_pending().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].rss_url, "https://mikanani.me/rss/bangumi");
    }

    #[tokio::test]
    async fn test_get_and_clear_pending() {
        let parser = Arc::new(RssParser::new());
        let handler = SubscriptionHandler::new(parser);

        let payload1 = SubscriptionBroadcastPayload {
            rss_url: "https://mikanani.me/rss/1".to_string(),
            service_name: "mikanani".to_string(),
        };

        let payload2 = SubscriptionBroadcastPayload {
            rss_url: "https://mikanani.me/rss/2".to_string(),
            service_name: "mikanani".to_string(),
        };

        handler.add_pending_subscription(payload1).await.ok();
        handler.add_pending_subscription(payload2).await.ok();

        let pending = handler.get_and_clear_pending().await;
        assert_eq!(pending.len(), 2);

        // Verify they are cleared
        let pending_again = handler.get_and_clear_pending().await;
        assert_eq!(pending_again.len(), 0);
    }

    #[tokio::test]
    async fn test_reject_non_mikanani_url() {
        let parser = Arc::new(RssParser::new());
        let handler = SubscriptionHandler::new(parser);

        let payload = SubscriptionBroadcastPayload {
            rss_url: "https://example.com/rss".to_string(),
            service_name: "mikanani".to_string(),
        };

        let result = handler.add_pending_subscription(payload).await;
        assert!(result.is_err());
    }
}
