use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::{NewWebhook, Webhook};
use crate::state::AppState;
use crate::db::{DieselWebhookRepository, WebhookRepository};

// ─── Request / Response DTOs ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub name: String,
    pub url: String,
    pub payload_template: String,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebhookRequest {
    pub name: Option<String>,
    pub url: Option<String>,
    pub payload_template: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub webhook_id: i32,
    pub name: String,
    pub url: String,
    pub payload_template: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Webhook> for WebhookResponse {
    fn from(w: Webhook) -> Self {
        Self {
            webhook_id: w.webhook_id,
            name: w.name,
            url: w.url,
            payload_template: w.payload_template,
            is_active: w.is_active,
            created_at: w.created_at.to_string(),
            updated_at: w.updated_at.to_string(),
        }
    }
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// GET /webhooks — 列出所有 webhook
pub async fn list_webhooks(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let repo = DieselWebhookRepository::new(state.db.clone());
    match repo.find_all().await {
        Ok(webhooks) => {
            let response: Vec<WebhookResponse> = webhooks.into_iter().map(Into::into).collect();
            (StatusCode::OK, Json(json!(response)))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error", "message": e.to_string()})),
        ),
    }
}

/// POST /webhooks — 建立 webhook
pub async fn create_webhook(
    State(state): State<AppState>,
    Json(payload): Json<CreateWebhookRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if payload.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_name", "message": "name cannot be empty"})),
        );
    }
    if payload.url.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_url", "message": "url cannot be empty"})),
        );
    }

    let now = Utc::now().naive_utc();
    let new_webhook = NewWebhook {
        name: payload.name,
        url: payload.url,
        payload_template: payload.payload_template,
        is_active: payload.is_active.unwrap_or(true),
        created_at: now,
        updated_at: now,
    };

    let repo = DieselWebhookRepository::new(state.db.clone());
    match repo.create(new_webhook).await {
        Ok(webhook) => {
            tracing::info!("Created webhook: {}", webhook.webhook_id);
            (StatusCode::CREATED, Json(json!(WebhookResponse::from(webhook))))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error", "message": e.to_string()})),
        ),
    }
}

/// GET /webhooks/:id — 取得單一 webhook
pub async fn get_webhook(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let repo = DieselWebhookRepository::new(state.db.clone());
    match repo.find_by_id(id).await {
        Ok(Some(webhook)) => (StatusCode::OK, Json(json!(WebhookResponse::from(webhook)))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "not_found", "message": "Webhook not found"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error", "message": e.to_string()})),
        ),
    }
}

/// PUT /webhooks/:id — 更新 webhook
pub async fn update_webhook(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateWebhookRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let repo = DieselWebhookRepository::new(state.db.clone());

    let existing = match repo.find_by_id(id).await {
        Ok(Some(w)) => w,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "not_found", "message": "Webhook not found"})),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "database_error", "message": e.to_string()})),
            )
        }
    };

    let updated = Webhook {
        name: payload.name.unwrap_or(existing.name),
        url: payload.url.unwrap_or(existing.url),
        payload_template: payload.payload_template.unwrap_or(existing.payload_template),
        is_active: payload.is_active.unwrap_or(existing.is_active),
        ..existing
    };

    match repo.update(updated).await {
        Ok(webhook) => (StatusCode::OK, Json(json!(WebhookResponse::from(webhook)))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error", "message": e.to_string()})),
        ),
    }
}

/// DELETE /webhooks/:id — 刪除 webhook
pub async fn delete_webhook(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let repo = DieselWebhookRepository::new(state.db.clone());
    match repo.delete(id).await {
        Ok(true) => (StatusCode::OK, Json(json!({"deleted": true}))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "not_found", "message": "Webhook not found"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "database_error", "message": e.to_string()})),
        ),
    }
}
