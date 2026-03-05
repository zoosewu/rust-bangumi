use chrono::Utc;
use diesel::prelude::*;
use serde_json::Value;
use std::sync::Arc;

use crate::db::DbPool;
use crate::models::{FilterTargetType, NewFilterRule, NewPendingAiResult, PendingAiResult};
use crate::schema::{ai_prompt_settings, filter_rules, pending_ai_results};
use super::client::{AiClient, AiError};
use super::parser_generator::build_ai_client;
use super::prompts::*;

pub async fn generate_filter_for_conflict(
    pool: Arc<DbPool>,
    conflict_titles: Vec<String>,
    source_title: String,
    temp_custom_prompt: Option<String>,
) -> Result<PendingAiResult, String> {
    let now = Utc::now().naive_utc();

    let (fixed_prompt, custom_prompt) = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let prompt_settings = ai_prompt_settings::table
            .first::<crate::models::AiPromptSettings>(&mut conn)
            .optional()
            .map_err(|e| e.to_string())?;
        let fixed = prompt_settings
            .as_ref()
            .and_then(|p| p.fixed_filter_prompt.clone())
            .unwrap_or_else(|| DEFAULT_FIXED_FILTER_PROMPT.to_string());
        let custom = temp_custom_prompt.or_else(|| {
            prompt_settings.and_then(|p| p.custom_filter_prompt)
        });
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
                subscription_id: None,
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
            client.chat_completion(&system, &user).await
        }
        Ok(None) => Err(AiError::NotConfigured),
        Err(e) => Err(AiError::ApiError(e)),
    };

    match ai_result {
        Ok(json_str) => match serde_json::from_str::<Value>(super::extract_json(&json_str)) {
            Ok(data) => {
                if data.get("rules").and_then(|r| r.as_array()).is_none() {
                    let err = "AI 返回的 JSON 缺少 rules 陣列".to_string();
                    return update_filter_pending_failed(&pool, pending_id, &err).await;
                }
                create_unconfirmed_filter_rules(&pool, &data, pending_id).await?;
                update_filter_pending_success(&pool, pending_id, data).await
            }
            Err(e) => {
                update_filter_pending_failed(
                    &pool,
                    pending_id,
                    &format!("JSON 解析失敗: {}", e),
                )
                .await
            }
        },
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
