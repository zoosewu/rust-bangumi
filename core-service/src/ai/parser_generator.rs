use chrono::Utc;
use diesel::prelude::*;
use serde_json::Value;
use std::sync::Arc;

use crate::db::DbPool;
use crate::models::{NewPendingAiResult, NewTitleParser, PendingAiResult};
use crate::schema::{ai_prompt_settings, ai_settings, pending_ai_results, title_parsers};
use super::client::AiClient;
use super::client::AiError;
use super::openai::OpenAiClient;
use super::prompts::*;

/// 從 DB 取得 AiClient，如果未設定則回傳 None
pub fn build_ai_client(conn: &mut PgConnection) -> Result<Option<OpenAiClient>, String> {
    let settings = ai_settings::table
        .first::<crate::models::AiSettings>(conn)
        .optional()
        .map_err(|e| e.to_string())?;

    match settings {
        Some(s) if !s.api_key.is_empty() && !s.base_url.is_empty() => {
            Ok(Some(OpenAiClient::new(&s.base_url, &s.api_key, &s.model_name)))
        }
        _ => Ok(None),
    }
}

/// 為單一動畫標題生成 parser（背景非同步觸發）
pub async fn generate_parser_for_title(
    pool: Arc<DbPool>,
    source_title: String,
    raw_item_id: Option<i32>,
    temp_custom_prompt: Option<String>,
) -> Result<PendingAiResult, String> {
    let now = Utc::now().naive_utc();

    // 取得 prompt 設定
    let (fixed_prompt, custom_prompt) = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let prompt_settings = ai_prompt_settings::table
            .first::<crate::models::AiPromptSettings>(&mut conn)
            .optional()
            .map_err(|e| e.to_string())?;
        let fixed = prompt_settings
            .as_ref()
            .and_then(|p| p.fixed_parser_prompt.clone())
            .unwrap_or_else(|| DEFAULT_FIXED_PARSER_PROMPT.to_string());
        let custom = temp_custom_prompt.or_else(|| {
            prompt_settings.and_then(|p| p.custom_parser_prompt)
        });
        (fixed, custom)
    };

    // 建立 pending record（status=generating）
    let pending = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        diesel::insert_into(pending_ai_results::table)
            .values(NewPendingAiResult {
                result_type: "parser".to_string(),
                source_title: source_title.clone(),
                generated_data: None,
                status: "generating".to_string(),
                error_message: None,
                raw_item_id,
                used_fixed_prompt: fixed_prompt.clone(),
                used_custom_prompt: custom_prompt.clone(),
                expires_at: None,
                created_at: now,
                updated_at: now,
            })
            .get_result::<PendingAiResult>(&mut conn)
            .map_err(|e| e.to_string())?
    };

    let pending_id = pending.id;

    // 建立 AI client 並呼叫
    let client_result = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        build_ai_client(&mut conn)
    };

    let ai_result = match client_result {
        Ok(Some(client)) => {
            let system = build_system_prompt(Some(&fixed_prompt));
            let user = build_parser_user_prompt(&source_title, custom_prompt.as_deref());
            client.chat_completion(&system, &user).await
        }
        Ok(None) => Err(AiError::NotConfigured),
        Err(e) => Err(AiError::ApiError(e)),
    };

    match ai_result {
        Ok(json_str) => match serde_json::from_str::<Value>(super::extract_json(&json_str)) {
            Ok(data) => {
                if data.get("condition_regex").is_none() || data.get("parse_regex").is_none() {
                    let err =
                        "AI 返回的 JSON 缺少必要欄位 condition_regex/parse_regex".to_string();
                    return update_pending_failed(&pool, pending_id, &err).await;
                }
                let parser = create_unconfirmed_parser(&pool, &data, pending_id).await?;
                tracing::info!("parser_id={} 已建立（未確認）", parser);
                update_pending_success(&pool, pending_id, data).await
            }
            Err(e) => {
                update_pending_failed(
                    &pool,
                    pending_id,
                    &format!("JSON 解析失敗: {}", e),
                )
                .await
            }
        },
        Err(e) => update_pending_failed(&pool, pending_id, &e.to_string()).await,
    }
}

async fn create_unconfirmed_parser(
    pool: &Arc<DbPool>,
    data: &Value,
    pending_id: i32,
) -> Result<i32, String> {
    let now = Utc::now().naive_utc();
    let mut conn = pool.get().map_err(|e| e.to_string())?;

    let get_str = |key: &str| -> String {
        data.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let get_opt_str = |key: &str| -> Option<String> {
        data.get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };

    use crate::models::ParserSourceType;
    let parse_source = |key: &str| -> ParserSourceType {
        match data.get(key).and_then(|v| v.as_str()) {
            Some("static") => ParserSourceType::Static,
            _ => ParserSourceType::Regex,
        }
    };
    let parse_opt_source = |key: &str| -> Option<ParserSourceType> {
        match data.get(key).and_then(|v| v.as_str()) {
            Some("static") => Some(ParserSourceType::Static),
            Some("regex") => Some(ParserSourceType::Regex),
            _ => None,
        }
    };

    let new_parser = NewTitleParser {
        name: get_str("name"),
        description: None,
        priority: 50,
        is_enabled: true,
        condition_regex: get_str("condition_regex"),
        parse_regex: get_str("parse_regex"),
        anime_title_source: parse_source("anime_title_source"),
        anime_title_value: get_str("anime_title_value"),
        episode_no_source: parse_source("episode_no_source"),
        episode_no_value: get_str("episode_no_value"),
        episode_end_source: parse_opt_source("episode_end_source"),
        episode_end_value: get_opt_str("episode_end_value"),
        series_no_source: parse_opt_source("series_no_source"),
        series_no_value: get_opt_str("series_no_value"),
        subtitle_group_source: parse_opt_source("subtitle_group_source"),
        subtitle_group_value: get_opt_str("subtitle_group_value"),
        resolution_source: parse_opt_source("resolution_source"),
        resolution_value: get_opt_str("resolution_value"),
        season_source: parse_opt_source("season_source"),
        season_value: get_opt_str("season_value"),
        year_source: parse_opt_source("year_source"),
        year_value: get_opt_str("year_value"),
        created_at: now,
        updated_at: now,
        created_from_type: None,
        created_from_id: None,
        pending_result_id: Some(pending_id),
    };

    diesel::insert_into(title_parsers::table)
        .values(&new_parser)
        .returning(title_parsers::parser_id)
        .get_result::<i32>(&mut conn)
        .map_err(|e| e.to_string())
}

async fn update_pending_success(
    pool: &Arc<DbPool>,
    pending_id: i32,
    data: Value,
) -> Result<PendingAiResult, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    diesel::update(pending_ai_results::table.find(pending_id))
        .set((
            pending_ai_results::status.eq("pending"),
            pending_ai_results::generated_data.eq(Some(data)),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| e.to_string())
}

async fn update_pending_failed(
    pool: &Arc<DbPool>,
    pending_id: i32,
    error: &str,
) -> Result<PendingAiResult, String> {
    tracing::warn!("AI parser 生成失敗 pending_id={}: {}", pending_id, error);
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    diesel::update(pending_ai_results::table.find(pending_id))
        .set((
            pending_ai_results::status.eq("failed"),
            pending_ai_results::error_message.eq(error),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| e.to_string())
}
