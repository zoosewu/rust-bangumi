use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::repository::raw_item::RawItemFilter;
use crate::dto::{FilterRuleRequest, FilterRuleResponse};
use crate::models::{FilterRule, FilterTargetType, NewFilterRule, RawAnimeItem};
use crate::state::AppState;

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

            // Trigger async recalculation of filtered_flag on affected links
            let db = state.db.clone();
            let tt = rule.target_type;
            let tid = rule.target_id;
            tokio::spawn(async move {
                if let Ok(mut conn) = db.get() {
                    match crate::services::filter_recalc::recalculate_filtered_flags(&mut conn, tt, tid) {
                        Ok(n) => tracing::info!("filter_recalc after create: updated {} links", n),
                        Err(e) => tracing::error!("filter_recalc after create failed: {}", e),
                    }
                }
            });

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

    match state
        .repos
        .filter_rule
        .find_by_target(target_type, query.target_id)
        .await
    {
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
    // Read the rule first to know its target for recalculation
    let rule_info = match state.repos.filter_rule.find_by_id(rule_id).await {
        Ok(Some(r)) => Some((r.target_type, r.target_id)),
        _ => None,
    };

    match state.repos.filter_rule.delete(rule_id).await {
        Ok(deleted) => {
            if deleted {
                tracing::info!("Deleted filter rule: {}", rule_id);

                // Trigger async recalculation
                if let Some((tt, tid)) = rule_info {
                    let db = state.db.clone();
                    tokio::spawn(async move {
                        if let Ok(mut conn) = db.get() {
                            match crate::services::filter_recalc::recalculate_filtered_flags(&mut conn, tt, tid) {
                                Ok(n) => tracing::info!("filter_recalc after delete: updated {} links", n),
                                Err(e) => tracing::error!("filter_recalc after delete failed: {}", e),
                            }
                        }
                    });
                }

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

// ============ Preview DTOs ============

#[derive(Debug, Deserialize)]
pub struct FilterPreviewRequest {
    pub regex_pattern: String,
    pub is_positive: bool,
    pub subscription_id: Option<i32>,
    pub exclude_filter_id: Option<i32>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PreviewItem {
    pub item_id: i32,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct FilterPreviewPanel {
    pub passed_items: Vec<PreviewItem>,
    pub filtered_items: Vec<PreviewItem>,
}

#[derive(Debug, Serialize)]
pub struct FilterPreviewResponse {
    pub regex_valid: bool,
    pub regex_error: Option<String>,
    pub before: FilterPreviewPanel,
    pub after: FilterPreviewPanel,
}

/// POST /filters/preview
pub async fn preview_filter(
    State(state): State<AppState>,
    Json(req): Json<FilterPreviewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate regex
    let regex = match Regex::new(&req.regex_pattern) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::OK,
                Json(json!(FilterPreviewResponse {
                    regex_valid: false,
                    regex_error: Some(e.to_string()),
                    before: FilterPreviewPanel {
                        passed_items: vec![],
                        filtered_items: vec![],
                    },
                    after: FilterPreviewPanel {
                        passed_items: vec![],
                        filtered_items: vec![],
                    },
                })),
            );
        }
    };

    let limit = req.limit.unwrap_or(50).min(200);

    // Load raw items
    let items = match state
        .repos
        .raw_item
        .find_with_filters(RawItemFilter {
            status: None,
            subscription_id: req.subscription_id,
            limit,
            offset: 0,
        })
        .await
    {
        Ok(items) => items,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to load items: {}", e) })),
            );
        }
    };

    // Load existing filter rules (global rules)
    let existing_rules = match state
        .repos
        .filter_rule
        .find_by_target(FilterTargetType::Global, None)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to load rules: {}", e) })),
            );
        }
    };

    // Build "before" rules (exclude current filter being edited)
    let before_rules: Vec<&FilterRule> = existing_rules
        .iter()
        .filter(|r| Some(r.rule_id) != req.exclude_filter_id)
        .collect();

    // Apply before rules
    let (before_passed, before_filtered) = apply_filter_rules(&items, &before_rules);

    // Apply after rules (before rules + new rule)
    let (after_passed, after_filtered) = {
        let mut after_passed = vec![];
        let mut after_filtered = vec![];
        for item in &items {
            let mut passed_existing = true;
            for rule in &before_rules {
                let r = Regex::new(&rule.regex_pattern).unwrap_or_else(|_| Regex::new("$^").unwrap());
                let matches = r.is_match(&item.title);
                if rule.is_positive && !matches {
                    passed_existing = false;
                    break;
                }
                if !rule.is_positive && matches {
                    passed_existing = false;
                    break;
                }
            }
            if !passed_existing {
                after_filtered.push(PreviewItem {
                    item_id: item.item_id,
                    title: item.title.clone(),
                });
                continue;
            }

            // Then check the new rule
            let matches_new = regex.is_match(&item.title);
            let passed_new = if req.is_positive {
                matches_new
            } else {
                !matches_new
            };
            if passed_new {
                after_passed.push(PreviewItem {
                    item_id: item.item_id,
                    title: item.title.clone(),
                });
            } else {
                after_filtered.push(PreviewItem {
                    item_id: item.item_id,
                    title: item.title.clone(),
                });
            }
        }
        (after_passed, after_filtered)
    };

    (
        StatusCode::OK,
        Json(json!(FilterPreviewResponse {
            regex_valid: true,
            regex_error: None,
            before: FilterPreviewPanel {
                passed_items: before_passed,
                filtered_items: before_filtered,
            },
            after: FilterPreviewPanel {
                passed_items: after_passed,
                filtered_items: after_filtered,
            },
        })),
    )
}

fn apply_filter_rules(
    items: &[RawAnimeItem],
    rules: &[&FilterRule],
) -> (Vec<PreviewItem>, Vec<PreviewItem>) {
    let mut passed = vec![];
    let mut filtered = vec![];
    for item in items {
        let mut item_passed = true;
        for rule in rules {
            let r = match Regex::new(&rule.regex_pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let matches = r.is_match(&item.title);
            if rule.is_positive && !matches {
                item_passed = false;
                break;
            }
            if !rule.is_positive && matches {
                item_passed = false;
                break;
            }
        }
        let preview = PreviewItem {
            item_id: item.item_id,
            title: item.title.clone(),
        };
        if item_passed {
            passed.push(preview);
        } else {
            filtered.push(preview);
        }
    }
    (passed, filtered)
}
