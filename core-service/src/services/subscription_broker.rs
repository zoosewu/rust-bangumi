use tokio::sync::broadcast;
use serde::{Serialize, Deserialize};

/// Event representing a subscription broadcast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionBroadcast {
    #[serde(alias = "rss_url")]
    pub source_url: String,
    pub subscription_name: String,
    #[serde(default = "default_source_type")]
    pub source_type: String,
}

fn default_source_type() -> String {
    "rss".to_string()
}

/// Type alias for the broadcast sender
pub type SubscriptionBroadcaster = broadcast::Sender<SubscriptionBroadcast>;

/// Create a new subscription broadcaster
pub fn create_subscription_broadcaster() -> SubscriptionBroadcaster {
    let (tx, _rx) = broadcast::channel(100);
    tx
}
