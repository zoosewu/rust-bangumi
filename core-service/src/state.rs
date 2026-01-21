use crate::services::ServiceRegistry;
use crate::db::DbPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub registry: Arc<ServiceRegistry>,
}
