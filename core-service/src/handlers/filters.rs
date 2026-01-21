use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde_json::json;
use diesel::prelude::*;

use crate::state::AppState;
use crate::dto::{FilterRuleRequest, FilterRuleResponse};
use crate::models::{NewFilterRule, FilterRule};
use crate::schema::filter_rules;

/// Create a new filter rule
pub async fn create_filter_rule(
    State(state): State<AppState>,
    Json(payload): Json<FilterRuleRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate rule_type
    if payload.rule_type != "Positive" && payload.rule_type != "Negative" {
        tracing::warn!(
            "Invalid rule_type: {}. Must be 'Positive' or 'Negative'",
            payload.rule_type
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_rule_type",
                "message": "rule_type must be 'Positive' or 'Negative'"
            })),
        );
    }

    let now = Utc::now().naive_utc();
    let new_rule = NewFilterRule {
        series_id: payload.series_id,
        group_id: payload.group_id,
        rule_order: payload.rule_order,
        rule_type: payload.rule_type,
        regex_pattern: payload.regex_pattern,
        created_at: now,
    };

    match state.db.get() {
        Ok(mut conn) => {
            match diesel::insert_into(filter_rules::table)
                .values(&new_rule)
                .get_result::<FilterRule>(&mut conn)
            {
                Ok(rule) => {
                    tracing::info!("Created filter rule: {}", rule.rule_id);
                    let response = FilterRuleResponse {
                        rule_id: rule.rule_id,
                        series_id: rule.series_id,
                        group_id: rule.group_id,
                        rule_order: rule.rule_order,
                        rule_type: rule.rule_type,
                        regex_pattern: rule.regex_pattern,
                        created_at: rule.created_at,
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

/// Get filter rules by series_id and group_id, sorted by rule_order
pub async fn get_filter_rules(
    State(state): State<AppState>,
    Path((series_id, group_id)): Path<(i32, i32)>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match filter_rules::table
                .filter(filter_rules::series_id.eq(series_id))
                .filter(filter_rules::group_id.eq(group_id))
                .order(filter_rules::rule_order.asc())
                .load::<FilterRule>(&mut conn)
            {
                Ok(rules) => {
                    let responses: Vec<FilterRuleResponse> = rules
                        .into_iter()
                        .map(|r| FilterRuleResponse {
                            rule_id: r.rule_id,
                            series_id: r.series_id,
                            group_id: r.group_id,
                            rule_order: r.rule_order,
                            rule_type: r.rule_type,
                            regex_pattern: r.regex_pattern,
                            created_at: r.created_at,
                        })
                        .collect();
                    tracing::info!(
                        "Retrieved {} filter rules for series_id={}, group_id={}",
                        responses.len(),
                        series_id,
                        group_id
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
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e),
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
    match state.db.get() {
        Ok(mut conn) => {
            match diesel::delete(filter_rules::table.find(rule_id)).execute(&mut conn) {
                Ok(deleted_count) => {
                    if deleted_count > 0 {
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
