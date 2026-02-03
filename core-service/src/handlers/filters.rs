use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::AppState;
use crate::dto::{FilterRuleRequest, FilterRuleResponse};
use crate::models::{NewFilterRule, FilterTargetType};

/// Query parameters for filter rules
#[derive(Debug, Deserialize, Serialize)]
pub struct FilterRulesQuery {
    pub target_type: String,
    pub target_id: Option<i32>,
}

/// Create a new filter rule
pub async fn create_filter_rule(
    State(state): State<AppState>,
    Json(payload): Json<FilterRuleRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Parse and validate target_type
    let target_type: FilterTargetType = match payload.target_type.parse() {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "invalid_target_type",
                    "message": e
                })),
            );
        }
    };

    // Validate: global rules must have null target_id, others must have a target_id
    if target_type == FilterTargetType::Global && payload.target_id.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_target_id",
                "message": "Global rules must not have a target_id"
            })),
        );
    }

    if target_type != FilterTargetType::Global && payload.target_id.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "missing_target_id",
                "message": "Non-global rules must have a target_id"
            })),
        );
    }

    let now = Utc::now().naive_utc();
    let new_rule = NewFilterRule {
        rule_order: payload.rule_order,
        is_positive: payload.is_positive,
        regex_pattern: payload.regex_pattern,
        created_at: now,
        updated_at: now,
        target_type,
        target_id: payload.target_id,
    };

    match state.repos.filter_rule.create(new_rule).await {
        Ok(rule) => {
            tracing::info!("Created filter rule: {}", rule.rule_id);
            let response = FilterRuleResponse {
                rule_id: rule.rule_id,
                target_type: rule.target_type.to_string(),
                target_id: rule.target_id,
                rule_order: rule.rule_order,
                is_positive: rule.is_positive,
                regex_pattern: rule.regex_pattern,
                created_at: rule.created_at,
                updated_at: rule.updated_at,
            };
            (StatusCode::CREATED, Json(json!(response)))
        }
        Err(e) => {
            tracing::error!("Failed to create filter rule: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to create filter rule: {}", e)
                })),
            )
        }
    }
}

/// Get filter rules by target_type and target_id, sorted by rule_order
pub async fn get_filter_rules(
    State(state): State<AppState>,
    Query(query): Query<FilterRulesQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Parse and validate target_type
    let target_type: FilterTargetType = match query.target_type.parse() {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "invalid_target_type",
                    "message": e,
                    "rules": []
                })),
            );
        }
    };

    match state.repos.filter_rule.find_by_target(target_type, query.target_id).await {
        Ok(rules) => {
            let responses: Vec<FilterRuleResponse> = rules
                .into_iter()
                .map(|r| FilterRuleResponse {
                    rule_id: r.rule_id,
                    target_type: r.target_type.to_string(),
                    target_id: r.target_id,
                    rule_order: r.rule_order,
                    is_positive: r.is_positive,
                    regex_pattern: r.regex_pattern,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                })
                .collect();
            tracing::info!(
                "Retrieved {} filter rules for target_type={}, target_id={:?}",
                responses.len(),
                query.target_type,
                query.target_id
            );
            (StatusCode::OK, Json(json!({ "rules": responses })))
        }
        Err(e) => {
            tracing::error!("Failed to list filter rules: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to list filter rules: {}", e),
                    "rules": []
                })),
            )
        }
    }
}

/// Delete a filter rule by rule_id
pub async fn delete_filter_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.filter_rule.delete(rule_id).await {
        Ok(deleted) => {
            if deleted {
                tracing::info!("Deleted filter rule: {}", rule_id);
                (StatusCode::NO_CONTENT, Json(json!({})))
            } else {
                tracing::warn!("Filter rule not found for deletion: {}", rule_id);
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "error": "not_found",
                        "message": format!("Filter rule {} not found", rule_id)
                    })),
                )
            }
        }
        Err(e) => {
            tracing::error!("Failed to delete filter rule: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to delete filter rule: {}", e)
                })),
            )
        }
    }
}
