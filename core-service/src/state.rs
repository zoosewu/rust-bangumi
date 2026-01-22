use crate::services::{ServiceRegistry, SubscriptionBroadcaster};
use crate::db::DbPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub registry: Arc<ServiceRegistry>,
    pub subscription_broadcaster: SubscriptionBroadcaster,
}
