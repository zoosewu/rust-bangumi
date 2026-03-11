use chrono::Utc;
use diesel::prelude::*;
use serde_json::Value;
use std::sync::Arc;

use crate::db::DbPool;
use crate::models::{AiPromptSettings, FilterTargetType, NewFilterRule, NewPendingAiResult, PendingAiResult};
use crate::schema::{ai_prompt_settings, filter_rules, pending_ai_results};
use super::client::{AiClient, AiError};
use super::parser_generator::build_ai_client;
use super::prompts::*;

pub async fn generate_filter_for_conflict(
    pool: Arc<DbPool>,
    conflict_titles: Vec<String>,
    source_title: String,
    temp_custom_prompt: Option<String>,
    subscription_id: Option<i32>,
    temp_fixed_prompt: Option<String>,
) -> Result<PendingAiResult, String> {
    let now = Utc::now().naive_utc();

    let (fixed_prompt, custom_prompt) = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let prompt_settings = ai_prompt_settings::table
            .first::<crate::models::AiPromptSettings>(&mut conn)
            .optional()
            .map_err(|e| e.to_string())?;
        let fixed = non_empty(temp_fixed_prompt)
            .or_else(|| non_empty(prompt_settings.as_ref().and_then(|p| p.fixed_filter_prompt.clone())))
            .unwrap_or_else(|| DEFAULT_FIXED_FILTER_PROMPT.to_string());
        let custom = non_empty(temp_custom_prompt)
            .or_else(|| non_empty(prompt_settings.and_then(|p| p.custom_filter_prompt)));
        (fixed, custom)
    };

    let pending = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        diesel::insert_into(pending_ai_results::table)
            .values(NewPendingAiResult {
                result_type: "filter".to_string(),
                source_title: source_title.clone(),
                generated_data: None,
                status: "generating".to_string(),
                error_message: None,
                raw_item_id: None,
                used_fixed_prompt: fixed_prompt.clone(),
                used_custom_prompt: custom_prompt.clone(),
                expires_at: None,
                created_at: now,
                updated_at: now,
                subscription_id,
                confirm_level: None,
                confirm_target_id: None,
            })
            .get_result::<PendingAiResult>(&mut conn)
            .map_err(|e| e.to_string())?
    };

    let pending_id = pending.id;

    let client_result = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        build_ai_client(&mut conn)
    };

    let ai_result = match client_result {
        Ok(Some(client)) => {
            let system = build_system_prompt(Some(&fixed_prompt));
            let user = build_filter_user_prompt(&conflict_titles, custom_prompt.as_deref());
            client.chat_completion_structured(&system, &user, &filter_schema()).await
        }
        Ok(None) => Err(AiError::NotConfigured),
        Err(e) => Err(AiError::ApiError(e)),
    };

    match ai_result {
        Ok(json_str) => {
            let extracted = super::extract_json(&json_str);
            tracing::debug!("AI 原始回應 pending_id={}: {:?}", pending_id, json_str);
            if extracted.is_empty() {
                return update_filter_pending_failed(
                    &pool,
                    pending_id,
                    "AI 回應為空或無法提取 JSON",
                )
                .await;
            }
            let sanitized = super::sanitize_ai_json(extracted);
            match serde_json::from_str::<Value>(&sanitized) {
                Ok(mut data) => {
                    if data.get("rules").and_then(|r| r.as_array()).is_none() {
                        let err = "AI 返回的 JSON 缺少 rules 陣列".to_string();
                        return update_filter_pending_failed(&pool, pending_id, &err).await;
                    }
                    // 修正雙重轉義的 regex_pattern 欄位
                    if let Some(rules) = data.get_mut("rules").and_then(|r| r.as_array_mut()) {
                        for rule in rules {
                            if let Some(fixed) = rule.get("regex_pattern")
                                .and_then(|v| v.as_str())
                                .map(super::fix_regex_escaping)
                            {
                                rule["regex_pattern"] = Value::String(fixed);
                            }
                        }
                    }
                    create_unconfirmed_filter_rules(&pool, &data, pending_id).await?;
                    update_filter_pending_success(&pool, pending_id, data).await
                }
                Err(e) => {
                    tracing::warn!(
                        "AI JSON 解析失敗 pending_id={}: {}\n原始回應: {:?}",
                        pending_id,
                        e,
                        json_str
                    );
                    update_filter_pending_failed(
                        &pool,
                        pending_id,
                        &format!("JSON 解析失敗: {}（原始回應長度: {} 字元）", e, json_str.len()),
                    )
                    .await
                }
            }
        }
        Err(e) => update_filter_pending_failed(&pool, pending_id, &e.to_string()).await,
    }
}

async fn create_unconfirmed_filter_rules(
    pool: &Arc<DbPool>,
    data: &Value,
    pending_id: i32,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let mut conn = pool.get().map_err(|e| e.to_string())?;

    let rules = data["rules"].as_array().unwrap();
    for rule in rules {
        let new_rule = NewFilterRule {
            rule_order: rule
                .get("rule_order")
                .and_then(|v| v.as_i64())
                .unwrap_or(1) as i32,
            regex_pattern: rule
                .get("regex_pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            is_positive: rule
                .get("is_positive")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            target_type: FilterTargetType::Global,
            target_id: None,
            created_at: now,
            updated_at: now,
            pending_result_id: Some(pending_id),
        };
        diesel::insert_into(filter_rules::table)
            .values(&new_rule)
            .execute(&mut conn)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn update_filter_pending_success(
    pool: &Arc<DbPool>,
    id: i32,
    data: Value,
) -> Result<PendingAiResult, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    diesel::update(pending_ai_results::table.find(id))
        .set((
            pending_ai_results::status.eq("pending"),
            pending_ai_results::generated_data.eq(Some(data)),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| e.to_string())
}

/// 載入 prompt 設定的 helper
fn load_filter_prompts(
    conn: &mut diesel::PgConnection,
    temp_custom: Option<String>,
    temp_fixed: Option<String>,
) -> Result<(String, Option<String>), String> {
    let prompt_settings = ai_prompt_settings::table
        .first::<AiPromptSettings>(conn)
        .optional()
        .map_err(|e| e.to_string())?;
    let fixed = non_empty(temp_fixed)
        .or_else(|| non_empty(prompt_settings.as_ref().and_then(|p| p.fixed_filter_prompt.clone())))
        .unwrap_or_else(|| DEFAULT_FIXED_FILTER_PROMPT.to_string());
    let custom = non_empty(temp_custom)
        .or_else(|| non_empty(prompt_settings.and_then(|p| p.custom_filter_prompt)));
    Ok((fixed, custom))
}

/// 批次為一個訂閱的所有衝突群組生成過濾規則（最多 5 次迭代）
///
/// `conflict_groups`: 每個元素是 (衝突標題列表, source_title)
pub async fn generate_filters_for_subscription_batch(
    pool: Arc<DbPool>,
    subscription_id: Option<i32>,
    conflict_groups: Vec<(Vec<String>, String)>,
) -> Result<(), String> {
    if conflict_groups.is_empty() {
        return Ok(());
    }

    const MAX_ITERATIONS: usize = 5;

    let (fixed_prompt, custom_prompt) = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        load_filter_prompts(&mut conn, None, None)?
    };

    let mut remaining: Vec<(Vec<String>, String)> = conflict_groups;

    for iteration in 1..=MAX_ITERATIONS {
        if remaining.is_empty() {
            break;
        }

        let now = Utc::now().naive_utc();
        let source_title = format!("批次過濾（iteration {}）", iteration);

        let pending_id = {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            diesel::insert_into(pending_ai_results::table)
                .values(NewPendingAiResult {
                    result_type: "filter".to_string(),
                    source_title: source_title.clone(),
                    generated_data: None,
                    status: "generating".to_string(),
                    error_message: None,
                    raw_item_id: None,
                    used_fixed_prompt: fixed_prompt.clone(),
                    used_custom_prompt: custom_prompt.clone(),
                    expires_at: None,
                    created_at: now,
                    updated_at: now,
                    subscription_id,
                    confirm_level: None,
                    confirm_target_id: None,
                })
                .get_result::<PendingAiResult>(&mut conn)
                .map_err(|e| e.to_string())?
                .id
        };

        let all_groups: Vec<Vec<String>> = remaining.iter().map(|(t, _)| t.clone()).collect();
        let system = build_system_prompt(Some(&fixed_prompt));
        let user = build_filter_batch_user_prompt(&all_groups, custom_prompt.as_deref());

        let client_result = {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            build_ai_client(&mut conn)
        };

        let ai_result = match client_result {
            Ok(Some(client)) => client.chat_completion_structured(&system, &user, &filter_schema()).await,
            Ok(None) => Err(AiError::NotConfigured),
            Err(e) => Err(AiError::ApiError(e)),
        };

        let json_str = match ai_result {
            Ok(s) => s,
            Err(e) => {
                let _ = update_filter_pending_failed(&pool, pending_id, &e.to_string()).await;
                break;
            }
        };

        tracing::debug!(
            "AI 批次過濾回應 pending_id={} iteration={}: {:?}",
            pending_id,
            iteration,
            json_str
        );

        let extracted = super::extract_json(&json_str);
        if extracted.is_empty() {
            let _ = update_filter_pending_failed(
                &pool,
                pending_id,
                "AI 回應為空或無法提取 JSON",
            )
            .await;
            break;
        }

        let sanitized = super::sanitize_ai_json(extracted);
        let mut data: Value = match serde_json::from_str(&sanitized) {
            Ok(v) => v,
            Err(e) => {
                let _ = update_filter_pending_failed(
                    &pool,
                    pending_id,
                    &format!(
                        "JSON 解析失敗: {}（原始回應長度: {} 字元）",
                        e,
                        json_str.len()
                    ),
                )
                .await;
                break;
            }
        };

        if data.get("rules").and_then(|r| r.as_array()).is_none() {
            let _ = update_filter_pending_failed(
                &pool,
                pending_id,
                "AI 返回的 JSON 缺少 rules 陣列",
            )
            .await;
            break;
        }

        // 修正雙重轉義的 regex_pattern 欄位
        if let Some(rules) = data.get_mut("rules").and_then(|r| r.as_array_mut()) {
            for rule in rules {
                if let Some(fixed) = rule.get("regex_pattern")
                    .and_then(|v| v.as_str())
                    .map(super::fix_regex_escaping)
                {
                    rule["regex_pattern"] = Value::String(fixed);
                }
            }
        }

        if let Err(e) = create_unconfirmed_filter_rules(&pool, &data, pending_id).await {
            let _ = update_filter_pending_failed(&pool, pending_id, &e).await;
            break;
        }

        let _ = update_filter_pending_success(&pool, pending_id, data.clone()).await;

        // 根據 AI 的 unresolved_groups 計算下次迭代的剩餘群組（1-indexed）
        let unresolved_indices: Vec<usize> = data
            .get("unresolved_groups")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64())
                    .filter_map(|i| i.checked_sub(1))
                    .map(|i| i as usize)
                    .collect()
            })
            .unwrap_or_default();

        if unresolved_indices.is_empty() {
            tracing::info!(
                "批次過濾完成：iteration {} 解決所有衝突群組",
                iteration
            );
            break;
        }

        let prev_count = remaining.len();
        remaining = unresolved_indices
            .into_iter()
            .filter_map(|i| remaining.get(i).cloned())
            .collect();

        tracing::info!(
            "批次過濾 iteration {}: {} 個群組中 {} 個仍未解決，進入下次迭代",
            iteration,
            prev_count,
            remaining.len()
        );

        // 若無進展則停止
        if remaining.len() >= prev_count {
            tracing::info!("批次過濾無進展，停止迭代");
            break;
        }
    }

    Ok(())
}

async fn update_filter_pending_failed(
    pool: &Arc<DbPool>,
    id: i32,
    error: &str,
) -> Result<PendingAiResult, String> {
    tracing::warn!("AI filter 生成失敗 pending_id={}: {}", id, error);
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    diesel::update(pending_ai_results::table.find(id))
        .set((
            pending_ai_results::status.eq("failed"),
            pending_ai_results::error_message.eq(error),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| e.to_string())
}
