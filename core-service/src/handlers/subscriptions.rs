use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde_json::json;
use diesel::prelude::*;
use tokio::time::{timeout, Duration};

use crate::state::AppState;
use crate::models::{Subscription, FetcherModule, NewSubscription};
use crate::schema::{subscriptions, fetcher_modules};
use crate::services::SubscriptionBroadcast;

// ============ DTOs ============

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct CreateSubscriptionRequest {
    #[serde(alias = "fetcher_id")]
    pub fetcher_id: Option<i32>,
    #[serde(alias = "rss_url")]
    pub source_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub fetch_interval_minutes: Option<i32>,
    pub config: Option<String>,
    pub source_type: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct SubscriptionResponse {
    pub subscription_id: i32,
    pub fetcher_id: i32,
    pub source_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub last_fetched_at: Option<chrono::NaiveDateTime>,
    pub next_fetch_at: Option<chrono::NaiveDateTime>,
    pub fetch_interval_minutes: i32,
    pub is_active: bool,
    pub config: Option<String>,
    pub source_type: String,
    pub assignment_status: String,
    pub assigned_at: Option<chrono::NaiveDateTime>,
    pub auto_selected: bool,
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
    pub priority: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct CanHandleRequest {
    pub source_url: String,
    pub source_type: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct CanHandleResponse {
    pub fetcher_id: i32,
    pub can_handle: bool,
    pub priority: i32,
}

// ============ Handlers ============

/// Create a new subscription with optional auto-selection or explicit fetcher assignment
pub async fn create_subscription(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let now = Utc::now().naive_utc();
    let fetch_interval = payload.fetch_interval_minutes.unwrap_or(60);
    let source_type = payload.source_type.unwrap_or_else(|| "rss".to_string());

    match state.db.get() {
        Ok(mut conn) => {
            // Check if subscription already exists
            let existing = subscriptions::table
                .filter(subscriptions::source_url.eq(&payload.source_url))
                .select(Subscription::as_select())
                .first::<Subscription>(&mut conn)
                .optional();

            match existing {
                Ok(Some(_)) => {
                    tracing::warn!(
                        "Subscription already exists for URL: {}",
                        payload.source_url
                    );
                    return (
                        StatusCode::CONFLICT,
                        Json(json!({
                            "error": "duplicate_url",
                            "message": format!("Subscription already exists for this URL: {}", payload.source_url)
                        })),
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to check existing subscriptions: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to check existing subscriptions: {}", e)
                        })),
                    );
                }
                _ => {} // OK - no existing subscription
            }

            // Determine assignment strategy
            let (fetcher_id, auto_selected, assignment_status) = if let Some(explicit_id) = payload.fetcher_id {
                // Explicit assignment
                (explicit_id, false, "assigned".to_string())
            } else {
                // Auto-selection: query fetchers and pick by highest priority
                match auto_select_fetcher(&mut conn) {
                    Ok(Some((id, _))) => (id, true, "auto_assigned".to_string()),
                    Ok(None) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({
                                "error": "no_fetchers",
                                "message": "No active fetchers available for auto-selection"
                            })),
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to auto-select fetcher: {}", e);
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({
                                "error": "selection_error",
                                "message": format!("Failed to select fetcher: {}", e)
                            })),
                        );
                    }
                }
            };

            // Manual insert using raw SQL for JSONB compatibility
            let new_subscription = NewSubscription {
                fetcher_id,
                source_url: payload.source_url.clone(),
                name: payload.name.clone(),
                description: payload.description.clone(),
                last_fetched_at: None,
                next_fetch_at: Some(now),
                fetch_interval_minutes: fetch_interval,
                is_active: true,
                config: payload.config.clone(),
                created_at: now,
                updated_at: now,
                source_type: source_type.clone(),
                assignment_status: assignment_status.clone(),
                assigned_at: if auto_selected { None } else { Some(now) },
                auto_selected,
            };

            let insert_result = diesel::insert_into(subscriptions::table)
                .values(&new_subscription)
                .returning(Subscription::as_returning())
                .get_result::<Subscription>(&mut conn);

            match insert_result {
                Ok(subscription) => {
                    let sub_id = subscription.subscription_id;
                    let f_id = subscription.fetcher_id;
                    let src_url = subscription.source_url.clone();
                    let n = subscription.name.clone();
                    let d = subscription.description.clone();
                    let lf = subscription.last_fetched_at;
                    let nf = subscription.next_fetch_at;
                    let fi = subscription.fetch_interval_minutes;
                    let ia = subscription.is_active;
                    let c = subscription.config.clone();
                    let ca = subscription.created_at;
                    let ua = subscription.updated_at;
                    let st = subscription.source_type.clone();
                    let ass = subscription.assignment_status.clone();
                    let aa = subscription.assigned_at;
                    let aus = subscription.auto_selected;
                    tracing::info!(
                        "Created subscription for URL: {} (fetcher_id: {}, auto_selected: {})",
                        src_url,
                        f_id,
                        aus
                    );

                    // Broadcast subscription event to all fetchers (for auto-selection if not explicit)
                    if payload.fetcher_id.is_none() {
                        let broadcast_event = SubscriptionBroadcast {
                            source_url: src_url.clone(),
                            subscription_name: n.clone().unwrap_or_else(|| src_url.clone()),
                            source_type: st.clone(),
                        };

                        if let Err(e) = state.subscription_broadcaster.send(broadcast_event) {
                            tracing::warn!("Failed to broadcast subscription event: {}", e);
                        }
                    }

                    let response = SubscriptionResponse {
                        subscription_id: sub_id,
                        fetcher_id: f_id,
                        source_url: src_url,
                        name: n,
                        description: d,
                        last_fetched_at: lf,
                        next_fetch_at: nf,
                        fetch_interval_minutes: fi,
                        is_active: ia,
                        config: c,
                        source_type: st,
                        assignment_status: ass,
                        assigned_at: aa,
                        auto_selected: aus,
                        created_at: ca,
                        updated_at: ua,
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

/// Auto-select the best fetcher by priority (highest first)
fn auto_select_fetcher(
    conn: &mut PgConnection,
) -> Result<Option<(i32, i32)>, String> {
    use crate::schema::fetcher_modules::dsl::*;

    match fetcher_modules
        .filter(is_enabled.eq(true))
        .order_by(priority.desc())
        .select((fetcher_id, priority))
        .first::<(i32, i32)>(conn)
    {
        Ok(result) => Ok(Some(result)),
        Err(diesel::result::Error::NotFound) => Ok(None),
        Err(e) => Err(format!("Database error: {}", e)),
    }
}

/// Broadcast can_handle requests to all enabled fetchers (or specific fetcher if target_fetcher_id is provided)
/// Returns a sorted list of fetchers that can handle the subscription (sorted by priority DESC)
/// Empty list means no fetcher can handle it
pub async fn broadcast_can_handle(
    state: &AppState,
    source_url: &str,
    source_type: &str,
    timeout_secs: u64,
    target_fetcher_id: Option<i32>,
) -> Result<Vec<(i32, i32)>, String> {
    let mut conn = state.db.get()
        .map_err(|e| format!("Database connection failed: {}", e))?;

    use crate::schema::fetcher_modules::dsl::*;

    // Build query based on target_fetcher_id
    let fetcher_list = if let Some(target_id) = target_fetcher_id {
        // Query specific fetcher
        fetcher_modules
            .filter(is_enabled.eq(true))
            .filter(fetcher_id.eq(target_id))
            .select(FetcherModule::as_select())
            .load::<FetcherModule>(&mut conn)
    } else {
        // Query all enabled fetchers
        fetcher_modules
            .filter(is_enabled.eq(true))
            .select(FetcherModule::as_select())
            .load::<FetcherModule>(&mut conn)
    };

    let fetcher_list = fetcher_list
        .map_err(|e| format!("Failed to load fetchers: {}", e))?;

    // Edge case 1: No fetchers found
    if fetcher_list.is_empty() {
        return if target_fetcher_id.is_some() {
            Err(format!("Target fetcher {} not found or disabled", target_fetcher_id.unwrap()))
        } else {
            Err("No enabled fetchers available".to_string())
        };
    }

    // Edge case 2: Validate base_url is configured
    for fetcher in &fetcher_list {
        if fetcher.base_url.is_empty() {
            return Err(format!("Fetcher {} has no base_url configured", fetcher.fetcher_id));
        }
    }

    // Spawn concurrent tasks for all fetchers
    let source_url_str = source_url.to_string();
    let source_type_str = source_type.to_string();
    let mut handles = vec![];
    for fetcher in fetcher_list {
        let source_url_clone = source_url_str.clone();
        let source_type_clone = source_type_str.clone();
        let handle = tokio::spawn(async move {
            let client = reqwest::Client::new();
            let url = format!("{}/can-handle-subscription", fetcher.base_url);

            let payload = CanHandleRequest {
                source_url: source_url_clone.clone(),
                source_type: source_type_clone,
            };

            match timeout(
                Duration::from_secs(timeout_secs),
                client.post(&url).json(&payload).send(),
            )
            .await
            {
                Ok(Ok(response)) => {
                    match response.json::<CanHandleResponse>().await {
                        Ok(data) if data.can_handle => {
                            tracing::debug!(
                                "Fetcher {} can handle: {} (priority: {})",
                                fetcher.fetcher_id,
                                source_url_clone,
                                fetcher.priority
                            );
                            Some((fetcher.fetcher_id, fetcher.priority))
                        }
                        Ok(_) => {
                            tracing::debug!("Fetcher {} cannot handle: {}", fetcher.fetcher_id, source_url_clone);
                            None
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Fetcher {} returned invalid response: {}",
                                fetcher.fetcher_id,
                                e
                            );
                            None
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("Fetcher {} request failed: {}", fetcher.fetcher_id, e);
                    None
                }
                Err(_) => {
                    tracing::warn!(
                        "Fetcher {} timeout after {} seconds",
                        fetcher.fetcher_id,
                        timeout_secs
                    );
                    None
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all responses
    let mut capable_fetchers = vec![];
    for handle in handles {
        if let Ok(Some(result)) = handle.await {
            capable_fetchers.push(result);
        }
    }

    // Sort by priority descending
    capable_fetchers.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(capable_fetchers)
}

/// List all active subscriptions
pub async fn list_subscriptions(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match subscriptions::table
                .filter(subscriptions::is_active.eq(true))
                .select(Subscription::as_select())
                .load::<Subscription>(&mut conn)
            {
                Ok(subs) => {
                    let responses: Vec<SubscriptionResponse> = subs
                        .into_iter()
                        .map(|s| SubscriptionResponse {
                            subscription_id: s.subscription_id,
                            fetcher_id: s.fetcher_id,
                            source_url: s.source_url,
                            name: s.name,
                            description: s.description,
                            last_fetched_at: s.last_fetched_at,
                            next_fetch_at: s.next_fetch_at,
                            fetch_interval_minutes: s.fetch_interval_minutes,
                            is_active: s.is_active,
                            config: s.config,
                            source_type: s.source_type,
                            assignment_status: s.assignment_status,
                            assigned_at: s.assigned_at,
                            auto_selected: s.auto_selected,
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
            match subscriptions::table
                .filter(subscriptions::fetcher_id.eq(fetcher_id))
                .filter(subscriptions::is_active.eq(true))
                .select(Subscription::as_select())
                .load::<Subscription>(&mut conn)
            {
                Ok(subs) => {
                    let urls: Vec<String> = subs
                        .iter()
                        .map(|s| s.source_url.clone())
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
            match fetcher_modules::table.select(FetcherModule::as_select()).load::<FetcherModule>(&mut conn) {
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
                            priority: m.priority,
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

/// Delete a subscription by source URL
pub async fn delete_subscription(
    State(state): State<AppState>,
    Path(source_url): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match diesel::delete(
                subscriptions::table.filter(subscriptions::source_url.eq(&source_url))
            )
            .execute(&mut conn)
            {
                Ok(rows_deleted) => {
                    if rows_deleted > 0 {
                        tracing::info!("Deleted {} subscription(s) for URL: {}", rows_deleted, source_url);
                        (
                            StatusCode::OK,
                            Json(json!({
                                "message": "Subscription deleted successfully",
                                "source_url": source_url,
                                "rows_deleted": rows_deleted
                            })),
                        )
                    } else {
                        tracing::warn!("Subscription not found for URL: {}", source_url);
                        (
                            StatusCode::NOT_FOUND,
                            Json(json!({
                                "error": "not_found",
                                "message": format!("Subscription not found for URL: {}", source_url)
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
