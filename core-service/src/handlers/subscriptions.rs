use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde_json::json;
use diesel::prelude::*;

use crate::state::AppState;
use crate::models::{RssSubscription, FetcherModule};
use crate::schema::{rss_subscriptions, fetcher_modules};

// ============ DTOs ============

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct CreateSubscriptionRequest {
    pub fetcher_id: i32,
    pub rss_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub fetch_interval_minutes: Option<i32>,
    pub config: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct SubscriptionResponse {
    pub subscription_id: i32,
    pub fetcher_id: i32,
    pub rss_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub last_fetched_at: Option<chrono::NaiveDateTime>,
    pub next_fetch_at: Option<chrono::NaiveDateTime>,
    pub fetch_interval_minutes: i32,
    pub is_active: bool,
    pub config: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct FetcherModuleResponse {
    pub fetcher_id: i32,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub config_schema: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

// ============ Handlers ============

/// Create a new RSS subscription
pub async fn create_subscription(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let now = Utc::now().naive_utc();
    let fetch_interval = payload.fetch_interval_minutes.unwrap_or(60);

    let new_subscription = crate::models::NewRssSubscription {
        fetcher_id: payload.fetcher_id,
        rss_url: payload.rss_url.clone(),
        name: payload.name,
        description: payload.description,
        last_fetched_at: None,
        next_fetch_at: Some(now),
        fetch_interval_minutes: fetch_interval,
        is_active: true,
        config: payload.config,
        created_at: now,
        updated_at: now,
    };

    match state.db.get() {
        Ok(mut conn) => {
            match diesel::insert_into(rss_subscriptions::table)
                .values(&new_subscription)
                .get_result::<RssSubscription>(&mut conn)
            {
                Ok(subscription) => {
                    tracing::info!("Created subscription for URL: {}", subscription.rss_url);
                    let response = SubscriptionResponse {
                        subscription_id: subscription.subscription_id,
                        fetcher_id: subscription.fetcher_id,
                        rss_url: subscription.rss_url,
                        name: subscription.name,
                        description: subscription.description,
                        last_fetched_at: subscription.last_fetched_at,
                        next_fetch_at: subscription.next_fetch_at,
                        fetch_interval_minutes: subscription.fetch_interval_minutes,
                        is_active: subscription.is_active,
                        config: subscription.config,
                        created_at: subscription.created_at,
                        updated_at: subscription.updated_at,
                    };
                    (StatusCode::CREATED, Json(json!(response)))
                }
                Err(e) => {
                    tracing::error!("Failed to create subscription: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to create subscription: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}

/// List all active subscriptions
pub async fn list_subscriptions(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match rss_subscriptions::table
                .filter(rss_subscriptions::is_active.eq(true))
                .load::<RssSubscription>(&mut conn)
            {
                Ok(subscriptions) => {
                    let responses: Vec<SubscriptionResponse> = subscriptions
                        .into_iter()
                        .map(|s| SubscriptionResponse {
                            subscription_id: s.subscription_id,
                            fetcher_id: s.fetcher_id,
                            rss_url: s.rss_url,
                            name: s.name,
                            description: s.description,
                            last_fetched_at: s.last_fetched_at,
                            next_fetch_at: s.next_fetch_at,
                            fetch_interval_minutes: s.fetch_interval_minutes,
                            is_active: s.is_active,
                            config: s.config,
                            created_at: s.created_at,
                            updated_at: s.updated_at,
                        })
                        .collect();
                    tracing::info!("Listed {} active subscriptions", responses.len());
                    (StatusCode::OK, Json(json!({ "subscriptions": responses })))
                }
                Err(e) => {
                    tracing::error!("Failed to list subscriptions: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to list subscriptions: {}", e),
                            "subscriptions": []
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e),
                    "subscriptions": []
                })),
            )
        }
    }
}

/// Get subscriptions for a specific fetcher module
pub async fn get_fetcher_subscriptions(
    State(state): State<AppState>,
    Path(fetcher_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match rss_subscriptions::table
                .filter(rss_subscriptions::fetcher_id.eq(fetcher_id))
                .filter(rss_subscriptions::is_active.eq(true))
                .load::<RssSubscription>(&mut conn)
            {
                Ok(subscriptions) => {
                    let urls: Vec<String> = subscriptions
                        .iter()
                        .map(|s| s.rss_url.clone())
                        .collect();
                    tracing::info!(
                        "Listed {} subscriptions for fetcher {}",
                        urls.len(),
                        fetcher_id
                    );
                    (
                        StatusCode::OK,
                        Json(json!({
                            "fetcher_id": fetcher_id,
                            "urls": urls
                        })),
                    )
                }
                Err(e) => {
                    tracing::error!("Failed to get fetcher subscriptions: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to get fetcher subscriptions: {}", e),
                            "fetcher_id": fetcher_id,
                            "urls": []
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e),
                    "fetcher_id": fetcher_id,
                    "urls": []
                })),
            )
        }
    }
}

/// List all registered fetcher modules
pub async fn list_fetcher_modules(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match fetcher_modules::table.load::<FetcherModule>(&mut conn) {
                Ok(modules) => {
                    let responses: Vec<FetcherModuleResponse> = modules
                        .into_iter()
                        .map(|m| FetcherModuleResponse {
                            fetcher_id: m.fetcher_id,
                            name: m.name,
                            version: m.version,
                            description: m.description,
                            is_enabled: m.is_enabled,
                            config_schema: m.config_schema,
                            created_at: m.created_at,
                            updated_at: m.updated_at,
                        })
                        .collect();
                    tracing::info!("Listed {} fetcher modules", responses.len());
                    (StatusCode::OK, Json(json!({ "fetcher_modules": responses })))
                }
                Err(e) => {
                    tracing::error!("Failed to list fetcher modules: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to list fetcher modules: {}", e),
                            "fetcher_modules": []
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e),
                    "fetcher_modules": []
                })),
            )
        }
    }
}

/// Delete a subscription by RSS URL
pub async fn delete_subscription(
    State(state): State<AppState>,
    Path(rss_url): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match diesel::delete(
                rss_subscriptions::table.filter(rss_subscriptions::rss_url.eq(&rss_url))
            )
            .execute(&mut conn)
            {
                Ok(rows_deleted) => {
                    if rows_deleted > 0 {
                        tracing::info!("Deleted {} subscription(s) for URL: {}", rows_deleted, rss_url);
                        (
                            StatusCode::OK,
                            Json(json!({
                                "message": "Subscription deleted successfully",
                                "rss_url": rss_url,
                                "rows_deleted": rows_deleted
                            })),
                        )
                    } else {
                        tracing::warn!("Subscription not found for URL: {}", rss_url);
                        (
                            StatusCode::NOT_FOUND,
                            Json(json!({
                                "error": "not_found",
                                "message": format!("Subscription not found for URL: {}", rss_url)
                            })),
                        )
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to delete subscription: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to delete subscription: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}
