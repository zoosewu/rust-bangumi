use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::{NewWebhook, Webhook};
use crate::schema::webhooks;

#[async_trait]
pub trait WebhookRepository: Send + Sync {
    async fn find_all(&self) -> Result<Vec<Webhook>, RepositoryError>;
    async fn find_active(&self) -> Result<Vec<Webhook>, RepositoryError>;
    async fn find_by_id(&self, id: i32) -> Result<Option<Webhook>, RepositoryError>;
    async fn create(&self, new_webhook: NewWebhook) -> Result<Webhook, RepositoryError>;
    async fn update(&self, webhook: Webhook) -> Result<Webhook, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
}

pub struct DieselWebhookRepository {
    pool: DbPool,
}

impl DieselWebhookRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WebhookRepository for DieselWebhookRepository {
    async fn find_all(&self) -> Result<Vec<Webhook>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            webhooks::table
                .order(webhooks::webhook_id.asc())
                .load::<Webhook>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_active(&self) -> Result<Vec<Webhook>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            webhooks::table
                .filter(webhooks::is_active.eq(true))
                .order(webhooks::webhook_id.asc())
                .load::<Webhook>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<Webhook>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            webhooks::table
                .find(id)
                .first::<Webhook>(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn create(&self, new_webhook: NewWebhook) -> Result<Webhook, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::insert_into(webhooks::table)
                .values(&new_webhook)
                .get_result::<Webhook>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn update(&self, webhook: Webhook) -> Result<Webhook, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();
            diesel::update(webhooks::table.find(webhook.webhook_id))
                .set((
                    webhooks::name.eq(&webhook.name),
                    webhooks::url.eq(&webhook.url),
                    webhooks::payload_template.eq(&webhook.payload_template),
                    webhooks::is_active.eq(webhook.is_active),
                    webhooks::updated_at.eq(now),
                ))
                .get_result::<Webhook>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let count = diesel::delete(webhooks::table.find(id))
                .execute(&mut conn)
                .map_err(RepositoryError::from)?;
            Ok(count > 0)
        })
        .await?
    }
}
