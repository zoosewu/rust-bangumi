use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::json;
use tokio::time::{timeout, Duration};

use crate::models::{ModuleTypeEnum, NewSubscription, ServiceModule, Subscription};
use crate::schema::{service_modules, subscriptions};
use crate::state::AppState;

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
    pub config: Option<serde_json::Value>,
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
    pub config: Option<serde_json::Value>,
    pub source_type: String,
    pub assignment_status: String,
    pub assigned_at: Option<chrono::NaiveDateTime>,
    pub auto_selected: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct FetcherModuleResponse {
    pub module_id: i32,
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
    pub can_handle: bool,
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

            // Drop the connection before async broadcast
            drop(conn);

            // Broadcast to determine fetcher capability
            match broadcast_can_handle(
                &state,
                &payload.source_url,
                &source_type,
                60,
                payload.fetcher_id, // None for auto-select, Some(id) for explicit
            )
            .await
            {
                Ok(capable_fetchers) if !capable_fetchers.is_empty() => {
                    // Select highest priority fetcher
                    let (fetcher_id, _priority) = capable_fetchers[0];
                    let auto_selected = payload.fetcher_id.is_none();
                    let assignment_status = if auto_selected {
                        "auto_assigned"
                    } else {
                        "assigned"
                    };

                    // Get connection again for database insert
                    let mut conn = match state.db.get() {
                        Ok(conn) => conn,
                        Err(e) => {
                            tracing::error!("Failed to get database connection: {}", e);
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"error": "database_error"})),
                            );
                        }
                    };

                    // Create subscription record
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
                        assignment_status: assignment_status.to_string(),
                        assigned_at: if auto_selected { None } else { Some(now) },
                        auto_selected,
                    };

                    let insert_result = diesel::insert_into(subscriptions::table)
                        .values(&new_subscription)
                        .returning(Subscription::as_returning())
                        .get_result::<Subscription>(&mut conn);

                    match insert_result {
                        Ok(subscription) => {
                            tracing::info!(
                                "Created subscription {} for URL {} with fetcher {} ({})",
                                subscription.subscription_id,
                                subscription.source_url,
                                fetcher_id,
                                assignment_status
                            );

                            // Fire-and-forget: 立即觸發一次撈取
                            let db = state.db.clone();
                            let sub_id = subscription.subscription_id;
                            let url = subscription.source_url.clone();
                            tokio::spawn(async move {
                                if let Err(e) =
                                    trigger_immediate_fetch(&db, sub_id, &url, fetcher_id).await
                                {
                                    tracing::warn!(
                                        "Immediate fetch failed for subscription {}: {}",
                                        sub_id,
                                        e
                                    );
                                }
                            });

                            (StatusCode::CREATED, Json(json!(subscription)))
                        }
                        Err(e) => {
                            tracing::error!("Failed to create subscription: {}", e);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({
                                    "error": "creation_failed",
                                    "message": format!("Failed to create subscription: {}", e)
                                })),
                            )
                        }
                    }
                }
                Ok(_) => {
                    // No fetcher can handle this subscription (strict mode)
                    tracing::warn!(
                        "No fetcher can handle subscription for URL: {} (type: {})",
                        payload.source_url,
                        source_type
                    );
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "error": "no_capable_fetcher",
                            "message": "No fetcher can handle this subscription request"
                        })),
                    )
                }
                Err(e) => {
                    tracing::error!("Broadcast failed: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "broadcast_failed",
                            "message": e
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
fn auto_select_fetcher(conn: &mut PgConnection) -> Result<Option<(i32, i32)>, String> {
    use crate::schema::service_modules::dsl::*;

    match service_modules
        .filter(is_enabled.eq(true))
        .filter(module_type.eq(ModuleTypeEnum::Fetcher))
        .order_by(priority.desc())
        .select((module_id, priority))
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
    let mut conn = state
        .db
        .get()
        .map_err(|e| format!("Database connection failed: {}", e))?;

    use crate::schema::service_modules::dsl::*;

    // Build query based on target_fetcher_id
    let fetcher_list = if let Some(target_id) = target_fetcher_id {
        // Query specific fetcher
        service_modules
            .filter(is_enabled.eq(true))
            .filter(module_type.eq(ModuleTypeEnum::Fetcher))
            .filter(module_id.eq(target_id))
            .select(ServiceModule::as_select())
            .load::<ServiceModule>(&mut conn)
    } else {
        // Query all enabled fetchers
        service_modules
            .filter(is_enabled.eq(true))
            .filter(module_type.eq(ModuleTypeEnum::Fetcher))
            .select(ServiceModule::as_select())
            .load::<ServiceModule>(&mut conn)
    };

    let fetcher_list = fetcher_list.map_err(|e| format!("Failed to load fetchers: {}", e))?;

    // Edge case 1: No fetchers found
    if fetcher_list.is_empty() {
        return if target_fetcher_id.is_some() {
            Err(format!(
                "Target fetcher {} not found or disabled",
                target_fetcher_id.unwrap()
            ))
        } else {
            Err("No enabled fetchers available".to_string())
        };
    }

    // Edge case 2: Validate base_url is configured
    for fetcher in &fetcher_list {
        if fetcher.base_url.is_empty() {
            return Err(format!(
                "Fetcher {} has no base_url configured",
                fetcher.module_id
            ));
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
                Ok(Ok(response)) => match response.json::<CanHandleResponse>().await {
                    Ok(data) if data.can_handle => {
                        tracing::debug!(
                            "Fetcher {} can handle: {} (priority: {})",
                            fetcher.module_id,
                            source_url_clone,
                            fetcher.priority
                        );
                        Some((fetcher.module_id, fetcher.priority))
                    }
                    Ok(_) => {
                        tracing::debug!(
                            "Fetcher {} cannot handle: {}",
                            fetcher.module_id,
                            source_url_clone
                        );
                        None
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Fetcher {} returned invalid response: {}",
                            fetcher.module_id,
                            e
                        );
                        None
                    }
                },
                Ok(Err(e)) => {
                    tracing::warn!("Fetcher {} request failed: {}", fetcher.module_id, e);
                    None
                }
                Err(_) => {
                    tracing::warn!(
                        "Fetcher {} timeout after {} seconds",
                        fetcher.module_id,
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
                    let urls: Vec<String> = subs.iter().map(|s| s.source_url.clone()).collect();
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
            match service_modules::table
                .filter(service_modules::module_type.eq(ModuleTypeEnum::Fetcher))
                .select(ServiceModule::as_select())
                .load::<ServiceModule>(&mut conn)
            {
                Ok(modules) => {
                    let responses: Vec<FetcherModuleResponse> = modules
                        .into_iter()
                        .map(|m| FetcherModuleResponse {
                            module_id: m.module_id,
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
                    (
                        StatusCode::OK,
                        Json(json!({ "fetcher_modules": responses })),
                    )
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

/// Delete a subscription by ID, cascade-deleting pending/failed raw items
pub async fn delete_subscription(
    State(state): State<AppState>,
    Path(subscription_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            // First, delete pending/failed raw items for this subscription
            let raw_deleted = diesel::delete(
                crate::schema::raw_anime_items::table
                    .filter(crate::schema::raw_anime_items::subscription_id.eq(subscription_id))
                    .filter(crate::schema::raw_anime_items::status.eq_any(vec!["pending", "failed"])),
            )
            .execute(&mut conn);

            let raw_count = match raw_deleted {
                Ok(count) => count,
                Err(e) => {
                    tracing::error!("Failed to delete raw items: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to delete raw items: {}", e)
                        })),
                    );
                }
            };

            match diesel::delete(
                subscriptions::table.filter(subscriptions::subscription_id.eq(subscription_id)),
            )
            .execute(&mut conn)
            {
                Ok(rows_deleted) => {
                    if rows_deleted > 0 {
                        tracing::info!(
                            "Deleted subscription {} (and {} raw items)",
                            subscription_id,
                            raw_count
                        );
                        (
                            StatusCode::OK,
                            Json(json!({
                                "message": "Subscription deleted successfully",
                                "subscription_id": subscription_id,
                                "raw_items_deleted": raw_count
                            })),
                        )
                    } else {
                        tracing::warn!("Subscription not found: {}", subscription_id);
                        (
                            StatusCode::NOT_FOUND,
                            Json(json!({
                                "error": "not_found",
                                "message": format!("Subscription not found: {}", subscription_id)
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

/// Best-effort 即時觸發撈取（新訂閱建立後立即呼叫 Fetcher）
async fn trigger_immediate_fetch(
    db: &crate::db::DbPool,
    subscription_id: i32,
    source_url: &str,
    fetcher_id: i32,
) -> Result<(), String> {
    // 查出 fetcher 的 base_url
    let mut conn = db.get().map_err(|e| format!("DB connection error: {}", e))?;

    let fetcher = service_modules::table
        .filter(service_modules::module_id.eq(fetcher_id))
        .filter(service_modules::is_enabled.eq(true))
        .filter(service_modules::module_type.eq(ModuleTypeEnum::Fetcher))
        .select(ServiceModule::as_select())
        .first::<ServiceModule>(&mut conn)
        .map_err(|e| format!("Fetcher {} not found or disabled: {}", fetcher_id, e))?;

    drop(conn);

    let callback_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());
    let callback_url = format!("{}/raw-fetcher-results", callback_url);

    let request = shared::FetchTriggerRequest {
        subscription_id,
        rss_url: source_url.to_string(),
        callback_url,
    };

    let fetch_url = format!("{}/fetch", fetcher.base_url);
    tracing::info!(
        "Triggering immediate fetch for subscription {} at {}",
        subscription_id,
        fetch_url
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let response = client
        .post(&fetch_url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();
    if status.is_success() || status == reqwest::StatusCode::ACCEPTED {
        tracing::info!(
            "Immediate fetch triggered successfully for subscription {}",
            subscription_id
        );
        Ok(())
    } else {
        Err(format!(
            "Fetcher returned HTTP {}: {}",
            status,
            response.text().await.unwrap_or_default()
        ))
    }
}
