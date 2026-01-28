use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde_json::json;
use diesel::prelude::*;

use crate::state::AppState;
use crate::models::{SubscriptionConflict, ServiceModule, ModuleTypeEnum};
use crate::schema::{subscription_conflicts, subscriptions, service_modules};

// ============ DTOs ============

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct ResolveConflictRequest {
    pub fetcher_id: i32,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct ConflictInfo {
    pub conflict_id: i32,
    pub subscription_id: i32,
    pub rss_url: String,
    pub conflict_type: String,
    pub conflict_data: serde_json::Value,
    pub candidate_fetchers: Vec<CandidateFetcher>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct CandidateFetcher {
    pub fetcher_id: i32,
    pub name: String,
}

// ============ Handlers ============

/// Get all pending conflicts where resolution is needed
pub async fn get_pending_conflicts(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            // Query all unresolved conflicts
            match subscription_conflicts::table
                .filter(subscription_conflicts::resolution_status.eq("unresolved"))
                .load::<SubscriptionConflict>(&mut conn)
            {
                Ok(conflicts) => {
                    let mut conflict_infos = Vec::new();

                    for conflict in conflicts {
                        // Get the subscription's source URL
                        let source_url_result = subscriptions::table
                            .filter(subscriptions::subscription_id.eq(conflict.subscription_id))
                            .select(subscriptions::source_url)
                            .first::<String>(&mut conn)
                            .optional();

                        let source_url = match source_url_result {
                            Ok(Some(url)) => url,
                            _ => "unknown".to_string(),
                        };

                        // Parse conflict_data to extract candidate fetchers
                        let candidate_fetchers = parse_candidate_fetchers(&conflict.conflict_data, &mut conn);

                        // Parse conflict_data as JSON
                        let conflict_data = serde_json::from_str(&conflict.conflict_data)
                            .unwrap_or(serde_json::json!({}));

                        conflict_infos.push(ConflictInfo {
                            conflict_id: conflict.conflict_id,
                            subscription_id: conflict.subscription_id,
                            rss_url: source_url,
                            conflict_type: conflict.conflict_type,
                            conflict_data,
                            candidate_fetchers,
                            created_at: conflict.created_at,
                        });
                    }

                    tracing::info!("Retrieved {} pending conflicts", conflict_infos.len());
                    (StatusCode::OK, Json(json!({ "conflicts": conflict_infos })))
                }
                Err(e) => {
                    tracing::error!("Failed to get pending conflicts: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to get pending conflicts: {}", e),
                            "conflicts": []
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
                    "conflicts": []
                })),
            )
        }
    }
}

/// Resolve a conflict by assigning it to a specific fetcher
pub async fn resolve_conflict(
    State(state): State<AppState>,
    Path(conflict_id): Path<i32>,
    Json(payload): Json<ResolveConflictRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            // 1. Get the conflict details
            let conflict_result = subscription_conflicts::table
                .filter(subscription_conflicts::conflict_id.eq(conflict_id))
                .first::<SubscriptionConflict>(&mut conn)
                .optional();

            let conflict = match conflict_result {
                Ok(Some(c)) => c,
                Ok(None) => {
                    tracing::warn!("Conflict not found: {}", conflict_id);
                    return (
                        StatusCode::NOT_FOUND,
                        Json(json!({
                            "error": "not_found",
                            "message": format!("Conflict not found: {}", conflict_id)
                        })),
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to get conflict: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to get conflict: {}", e)
                        })),
                    );
                }
            };

            // 2. Verify the fetcher is in the candidate list
            let candidate_fetchers = parse_candidate_fetchers_vec(&conflict.conflict_data);
            if !candidate_fetchers.contains(&payload.fetcher_id) {
                tracing::warn!(
                    "Fetcher {} not in candidate list for conflict {}",
                    payload.fetcher_id,
                    conflict_id
                );
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "invalid_fetcher",
                        "message": format!("Fetcher {} is not a candidate for this conflict", payload.fetcher_id),
                        "candidates": candidate_fetchers
                    })),
                );
            }

            // 3. Verify fetcher exists
            let fetcher_exists = service_modules::table
                .filter(service_modules::module_id.eq(payload.fetcher_id))
                .filter(service_modules::module_type.eq(ModuleTypeEnum::Fetcher))
                .count()
                .get_result::<i64>(&mut conn)
                .map(|count| count > 0)
                .unwrap_or(false);

            if !fetcher_exists {
                tracing::warn!("Fetcher not found: {}", payload.fetcher_id);
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "error": "not_found",
                        "message": format!("Fetcher {} not found", payload.fetcher_id)
                    })),
                );
            }

            // 4. Update subscriptions table with the resolved fetcher
            let update_subscription_result = diesel::update(
                subscriptions::table
                    .filter(subscriptions::subscription_id.eq(conflict.subscription_id))
            )
            .set(subscriptions::fetcher_id.eq(payload.fetcher_id))
            .execute(&mut conn);

            if let Err(e) = update_subscription_result {
                tracing::error!("Failed to update subscription: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "database_error",
                        "message": format!("Failed to update subscription: {}", e)
                    })),
                );
            }

            // 5. Update the conflict record as resolved
            let now = Utc::now().naive_utc();
            let resolution_data = json!({
                "resolved_fetcher_id": payload.fetcher_id,
                "resolved_at": now
            });

            let update_conflict_result = diesel::update(
                subscription_conflicts::table
                    .filter(subscription_conflicts::conflict_id.eq(conflict_id))
            )
            .set((
                subscription_conflicts::resolution_status.eq("resolved"),
                subscription_conflicts::resolution_data.eq(resolution_data.to_string()),
                subscription_conflicts::resolved_at.eq(now),
            ))
            .get_result::<SubscriptionConflict>(&mut conn);

            match update_conflict_result {
                Ok(updated_conflict) => {
                    tracing::info!(
                        "Resolved conflict {} with fetcher {}",
                        conflict_id,
                        payload.fetcher_id
                    );
                    (
                        StatusCode::OK,
                        Json(json!({
                            "message": "Conflict resolved successfully",
                            "conflict_id": updated_conflict.conflict_id,
                            "subscription_id": updated_conflict.subscription_id,
                            "resolved_fetcher_id": payload.fetcher_id,
                            "resolved_at": now
                        })),
                    )
                }
                Err(e) => {
                    tracing::error!("Failed to update conflict: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to update conflict: {}", e)
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

// ============ Helper Functions ============

/// Parse candidate fetchers from conflict_data JSON
fn parse_candidate_fetchers(conflict_data: &str, conn: &mut diesel::PgConnection) -> Vec<CandidateFetcher> {
    if let Ok(data) = serde_json::from_str::<serde_json::Value>(conflict_data) {
        if let Some(fetcher_ids) = data.get("candidate_fetcher_ids").and_then(|v| v.as_array()) {
            let mut fetchers = Vec::new();
            for fetcher_id_val in fetcher_ids {
                if let Some(fetcher_id) = fetcher_id_val.as_i64() {
                    let fetcher_id = fetcher_id as i32;
                    // Get fetcher name from database
                    if let Ok(name) = service_modules::table
                        .filter(service_modules::module_id.eq(fetcher_id))
                        .filter(service_modules::module_type.eq(ModuleTypeEnum::Fetcher))
                        .select(service_modules::name)
                        .first::<String>(conn)
                        .optional()
                    {
                        if let Some(fetcher_name) = name {
                            fetchers.push(CandidateFetcher {
                                fetcher_id,
                                name: fetcher_name,
                            });
                        }
                    }
                }
            }
            return fetchers;
        }
    }
    Vec::new()
}

/// Parse candidate fetcher IDs from conflict_data JSON string
fn parse_candidate_fetchers_vec(conflict_data: &str) -> Vec<i32> {
    if let Ok(data) = serde_json::from_str::<serde_json::Value>(conflict_data) {
        if let Some(fetcher_ids) = data.get("candidate_fetcher_ids").and_then(|v| v.as_array()) {
            return fetcher_ids
                .iter()
                .filter_map(|v| v.as_i64().map(|id| id as i32))
                .collect();
        }
    }
    Vec::new()
}
