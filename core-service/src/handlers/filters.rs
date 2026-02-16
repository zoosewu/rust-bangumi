use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::dto::{FilterRuleRequest, FilterRuleResponse};
use crate::models::{FilterRule, FilterTargetType, NewFilterRule};
use crate::services::filter::FilterEngine;
use crate::services::filter_recalc;
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

            // Trigger async recalculation of filtered_flag on affected links,
            // then re-run conflict detection (filtered links affect conflict state)
            let db = state.db.clone();
            let conflict_detection = state.conflict_detection.clone();
            let tt = rule.target_type;
            let tid = rule.target_id;
            tokio::spawn(async move {
                if let Ok(mut conn) = db.get() {
                    match crate::services::filter_recalc::recalculate_filtered_flags(&mut conn, tt, tid) {
                        Ok(n) => tracing::info!("filter_recalc after create: updated {} links", n),
                        Err(e) => tracing::error!("filter_recalc after create failed: {}", e),
                    }
                }
                if let Err(e) = conflict_detection.detect_and_mark_conflicts().await {
                    tracing::error!("conflict re-detection after filter create failed: {}", e);
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

                // Trigger async recalculation + conflict re-detection
                if let Some((tt, tid)) = rule_info {
                    let db = state.db.clone();
                    let conflict_detection = state.conflict_detection.clone();
                    tokio::spawn(async move {
                        if let Ok(mut conn) = db.get() {
                            match crate::services::filter_recalc::recalculate_filtered_flags(&mut conn, tt, tid) {
                                Ok(n) => tracing::info!("filter_recalc after delete: updated {} links", n),
                                Err(e) => tracing::error!("filter_recalc after delete failed: {}", e),
                            }
                        }
                        if let Err(e) = conflict_detection.detect_and_mark_conflicts().await {
                            tracing::error!("conflict re-detection after filter delete failed: {}", e);
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
    pub target_type: String,
    pub target_id: Option<i32>,
    pub regex_pattern: String,
    pub is_positive: bool,
    pub exclude_filter_id: Option<i32>,
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
///
/// Preview the effect of adding/removing a filter rule on anime_links
/// scoped to the given target_type/target_id.
pub async fn preview_filter(
    State(state): State<AppState>,
    Json(req): Json<FilterPreviewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Parse target_type
    let target_type: FilterTargetType = match req.target_type.parse() {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("invalid target_type: {}", e) })),
            );
        }
    };

    // Validate regex
    if let Err(e) = Regex::new(&req.regex_pattern) {
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

    // All DB work uses a synchronous connection (same as filter_recalc)
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("DB connection failed: {}", e) })),
            );
        }
    };

    // Load anime_links scoped to this target
    let links = match filter_recalc::find_affected_links(&mut conn, target_type, req.target_id) {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to load links: {}", e) })),
            );
        }
    };

    // Build a temporary FilterRule for the new rule being previewed
    let now = Utc::now().naive_utc();
    let new_rule = FilterRule {
        rule_id: -1, // sentinel
        rule_order: 0,
        is_positive: req.is_positive,
        regex_pattern: req.regex_pattern.clone(),
        created_at: now,
        updated_at: now,
        target_type,
        target_id: req.target_id,
    };

    let mut before_passed = vec![];
    let mut before_filtered = vec![];
    let mut after_passed = vec![];
    let mut after_filtered = vec![];

    for link in &links {
        let title = link.title.as_deref().unwrap_or("");

        // Collect all existing applicable rules for this link (full hierarchy)
        let existing_rules = match filter_recalc::collect_all_rules_for_link(&mut conn, link) {
            Ok(r) => r,
            Err(_) => vec![],
        };

        // "Before" rules = existing rules, excluding the one being edited
        let before_rules: Vec<FilterRule> = existing_rules
            .iter()
            .filter(|r| Some(r.rule_id) != req.exclude_filter_id)
            .cloned()
            .collect();

        let before_engine = FilterEngine::with_priority_sorted(before_rules.clone());
        let before_include = before_engine.should_include(title);

        // "After" rules = before rules + new rule
        let mut after_rules = before_rules;
        after_rules.push(new_rule.clone());
        let after_engine = FilterEngine::with_priority_sorted(after_rules);
        let after_include = after_engine.should_include(title);

        if before_include {
            before_passed.push(PreviewItem { item_id: link.link_id, title: title.to_string() });
        } else {
            before_filtered.push(PreviewItem { item_id: link.link_id, title: title.to_string() });
        }

        if after_include {
            after_passed.push(PreviewItem { item_id: link.link_id, title: title.to_string() });
        } else {
            after_filtered.push(PreviewItem { item_id: link.link_id, title: title.to_string() });
        }
    }

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

// ============ Raw Preview DTOs ============

#[derive(Debug, Serialize)]
pub struct RawPreviewItem {
    pub item_id: i32,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct RawFilterPreviewPanel {
    pub passed_items: Vec<RawPreviewItem>,
    pub filtered_items: Vec<RawPreviewItem>,
}

#[derive(Debug, Serialize)]
pub struct RawFilterPreviewResponse {
    pub regex_valid: bool,
    pub regex_error: Option<String>,
    pub before: RawFilterPreviewPanel,
    pub after: RawFilterPreviewPanel,
}

/// POST /filters/preview-raw
///
/// Preview the effect of adding/removing a filter rule on raw_anime_items
/// scoped to a subscription (target_type must be "fetcher" or "subscription").
pub async fn preview_filter_raw(
    State(state): State<AppState>,
    Json(req): Json<FilterPreviewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate regex
    let _new_regex = match Regex::new(&req.regex_pattern) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::OK,
                Json(json!(RawFilterPreviewResponse {
                    regex_valid: false,
                    regex_error: Some(e.to_string()),
                    before: RawFilterPreviewPanel { passed_items: vec![], filtered_items: vec![] },
                    after: RawFilterPreviewPanel { passed_items: vec![], filtered_items: vec![] },
                })),
            );
        }
    };

    let subscription_id = match req.target_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "target_id (subscription_id) is required for preview-raw" })),
            );
        }
    };

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("DB connection failed: {}", e) })),
            );
        }
    };

    // Load raw_anime_items for this subscription
    use crate::schema::raw_anime_items;
    use diesel::prelude::*;
    let raw_items: Vec<crate::models::RawAnimeItem> = match raw_anime_items::table
        .filter(raw_anime_items::subscription_id.eq(subscription_id))
        .order(raw_anime_items::created_at.desc())
        .load(&mut conn)
    {
        Ok(items) => items,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to load raw items: {}", e) })),
            );
        }
    };

    // Load existing filter rules for this subscription (fetcher target type)
    let existing_rules: Vec<FilterRule> = match crate::schema::filter_rules::table
        .filter(crate::schema::filter_rules::target_type.eq(FilterTargetType::Fetcher))
        .filter(crate::schema::filter_rules::target_id.eq(subscription_id))
        .order(crate::schema::filter_rules::rule_order.asc())
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(_) => vec![],
    };

    // Also load global rules
    let global_rules: Vec<FilterRule> = match crate::schema::filter_rules::table
        .filter(crate::schema::filter_rules::target_type.eq(FilterTargetType::Global))
        .filter(crate::schema::filter_rules::target_id.is_null())
        .order(crate::schema::filter_rules::rule_order.asc())
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(_) => vec![],
    };

    // Build the new temporary rule
    let now = Utc::now().naive_utc();
    let new_rule = FilterRule {
        rule_id: -1,
        rule_order: 0,
        is_positive: req.is_positive,
        regex_pattern: req.regex_pattern.clone(),
        created_at: now,
        updated_at: now,
        target_type: FilterTargetType::Fetcher,
        target_id: Some(subscription_id),
    };

    let mut before_passed = vec![];
    let mut before_filtered = vec![];
    let mut after_passed = vec![];
    let mut after_filtered = vec![];

    for item in &raw_items {
        let title = &item.title;

        // "Before" rules = global + existing, excluding the one being edited
        let before_rules: Vec<FilterRule> = global_rules
            .iter()
            .chain(existing_rules.iter())
            .filter(|r| Some(r.rule_id) != req.exclude_filter_id)
            .cloned()
            .collect();

        let before_engine = FilterEngine::with_priority_sorted(before_rules.clone());
        let before_include = before_engine.should_include(title);

        // "After" rules = before + new rule
        let mut after_rules = before_rules;
        after_rules.push(new_rule.clone());
        let after_engine = FilterEngine::with_priority_sorted(after_rules);
        let after_include = after_engine.should_include(title);

        let preview_item_before = RawPreviewItem {
            item_id: item.item_id,
            title: title.clone(),
            status: item.status.clone(),
        };
        let preview_item_after = RawPreviewItem {
            item_id: item.item_id,
            title: title.clone(),
            status: item.status.clone(),
        };

        if before_include {
            before_passed.push(preview_item_before);
        } else {
            before_filtered.push(preview_item_before);
        }
        if after_include {
            after_passed.push(preview_item_after);
        } else {
            after_filtered.push(preview_item_after);
        }
    }

    (
        StatusCode::OK,
        Json(json!(RawFilterPreviewResponse {
            regex_valid: true,
            regex_error: None,
            before: RawFilterPreviewPanel { passed_items: before_passed, filtered_items: before_filtered },
            after: RawFilterPreviewPanel { passed_items: after_passed, filtered_items: after_filtered },
        })),
    )
}
