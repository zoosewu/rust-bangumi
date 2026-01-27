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
use crate::models::{Subscription, FetcherModule};
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
            let config_str = payload.config.as_ref().map(|s| s.as_str());
            let insert_result = diesel::sql_query(
                "INSERT INTO subscriptions (fetcher_id, source_url, name, description, last_fetched_at, next_fetch_at, \
                fetch_interval_minutes, is_active, config, created_at, updated_at, source_type, assignment_status, assigned_at, auto_selected) \
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15) \
                RETURNING subscription_id, fetcher_id, source_url, name, description, last_fetched_at, next_fetch_at, \
                fetch_interval_minutes, is_active, config, created_at, updated_at, source_type, assignment_status, assigned_at, auto_selected"
            )
            .bind::<diesel::sql_types::Int4, _>(fetcher_id)
            .bind::<diesel::sql_types::Varchar, _>(&payload.source_url)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Varchar>, _>(&payload.name)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(&payload.description)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Timestamp>, _>(None::<chrono::NaiveDateTime>)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Timestamp>, _>(Some(now))
            .bind::<diesel::sql_types::Int4, _>(fetch_interval)
            .bind::<diesel::sql_types::Bool, _>(true)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(config_str)
            .bind::<diesel::sql_types::Timestamp, _>(now)
            .bind::<diesel::sql_types::Timestamp, _>(now)
            .bind::<diesel::sql_types::Varchar, _>(&source_type)
            .bind::<diesel::sql_types::Varchar, _>(&assignment_status)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Timestamp>, _>(if auto_selected { None } else { Some(now) })
            .bind::<diesel::sql_types::Bool, _>(auto_selected)
            .get_result::<(i32, i32, String, Option<String>, Option<String>, Option<chrono::NaiveDateTime>, Option<chrono::NaiveDateTime>, i32, bool, Option<String>, chrono::NaiveDateTime, chrono::NaiveDateTime, String, String, Option<chrono::NaiveDateTime>, bool)>(&mut conn);

            match insert_result {
                Ok((sub_id, f_id, src_url, n, d, lf, nf, fi, ia, c, ca, ua, st, ass, aa, aus)) => {
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

/// Broadcast can_handle requests to all fetchers and collect responses with timeout
pub async fn broadcast_can_handle(
    state: &AppState,
    source_url: &str,
    source_type: &str,
    timeout_secs: u64,
) -> Vec<CanHandleResponse> {
    let mut responses = Vec::new();

    // Get all enabled fetchers from database
    match state.db.get() {
        Ok(mut conn) => {
            use crate::schema::fetcher_modules::dsl::*;

            let fetcher_list = match fetcher_modules
                .filter(is_enabled.eq(true))
                .load::<FetcherModule>(&mut conn)
            {
                Ok(list) => list,
                Err(e) => {
                    tracing::error!("Failed to load fetchers for broadcast: {}", e);
                    return responses;
                }
            };

            // Create async tasks for parallel requests
            let mut tasks = vec![];
            for fetcher in fetcher_list {
                let source_url = source_url.to_string();
                let source_type = source_type.to_string();

                let task = tokio::spawn(async move {
                    let client = reqwest::Client::new();
                    let url = format!(
                        "http://{}:{}/can-handle-subscription",
                        fetcher.name, fetcher.fetcher_id
                    );

                    let payload = CanHandleRequest {
                        source_url: source_url.clone(),
                        source_type: source_type.clone(),
                    };

                    match timeout(
                        Duration::from_secs(timeout_secs),
                        client.post(&url).json(&payload).send(),
                    )
                    .await
                    {
                        Ok(Ok(response)) => {
                            match response.json::<serde_json::Value>().await {
                                Ok(json) => {
                                    if let (Some(can_handle), Some(priority)) = (
                                        json.get("can_handle").and_then(|v| v.as_bool()),
                                        json.get("priority").and_then(|v| v.as_i64()),
                                    ) {
                                        if can_handle {
                                            Some(CanHandleResponse {
                                                fetcher_id: fetcher.fetcher_id,
                                                can_handle: true,
                                                priority: priority as i32,
                                            })
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to parse response from fetcher {}: {}",
                                        fetcher.fetcher_id,
                                        e
                                    );
                                    None
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            tracing::debug!(
                                "Failed to contact fetcher {}: {}",
                                fetcher.fetcher_id,
                                e
                            );
                            None
                        }
                        Err(_) => {
                            tracing::debug!("Timeout contacting fetcher {}", fetcher.fetcher_id);
                            None
                        }
                    }
                });

                tasks.push(task);
            }

            // Wait for all responses
            for task in tasks {
                if let Ok(Some(response)) = task.await {
                    responses.push(response);
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection for broadcast: {}", e);
        }
    }

    // Sort by priority (highest first)
    responses.sort_by(|a, b| b.priority.cmp(&a.priority));
    responses
}

/// List all active subscriptions
pub async fn list_subscriptions(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match subscriptions::table
                .filter(subscriptions::is_active.eq(true))
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
