use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use diesel::prelude::*;
use serde::Deserialize;

use crate::ai::prompts::{DEFAULT_FIXED_FILTER_PROMPT, DEFAULT_FIXED_PARSER_PROMPT};
use crate::ai::queue::{ai_queue, MAX_CONCURRENCY};
use crate::models::AiPromptSettings;
use crate::schema::ai_prompt_settings;
use crate::state::AppState;

// GET /ai-prompt-settings
pub async fn get_ai_prompt_settings(
    State(state): State<AppState>,
) -> Result<Json<AiPromptSettings>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let settings = ai_prompt_settings::table
        .first::<AiPromptSettings>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(settings))
}

/// 只更新有送出的欄位：設定頁各區塊分開儲存，沒送的欄位維持原值。
#[derive(Debug, Deserialize, AsChangeset)]
#[diesel(table_name = ai_prompt_settings)]
pub struct UpdateAiPromptSettingsRequest {
    pub fixed_parser_prompt: Option<String>,
    pub fixed_filter_prompt: Option<String>,
    pub custom_parser_prompt: Option<String>,
    pub custom_filter_prompt: Option<String>,
    pub max_concurrent_requests: Option<i32>,
}

fn valid_concurrency(value: i32) -> bool {
    (1..=MAX_CONCURRENCY as i32).contains(&value)
}

// PUT /ai-prompt-settings
pub async fn update_ai_prompt_settings(
    State(state): State<AppState>,
    Json(req): Json<UpdateAiPromptSettingsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if let Some(n) = req
        .max_concurrent_requests
        .filter(|n| !valid_concurrency(*n))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "max_concurrent_requests must be between 1 and {}, got {}",
                MAX_CONCURRENCY, n
            ),
        ));
    }
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = Utc::now().naive_utc();
    diesel::update(ai_prompt_settings::table)
        .set((&req, ai_prompt_settings::updated_at.eq(now)))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Some(n) = req.max_concurrent_requests {
        ai_queue().set_limit(n as usize);
        tracing::info!("AI 請求併行數已設為 {}", n);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// 啟動時把資料庫裡的併行數套用到全域 AI 佇列；讀不到就維持預設值。
pub fn apply_saved_concurrency(conn: &mut PgConnection) {
    match ai_prompt_settings::table
        .select(ai_prompt_settings::max_concurrent_requests)
        .first::<i32>(conn)
    {
        Ok(n) => {
            ai_queue().set_limit(n as usize);
            tracing::info!("AI 請求併行數：{}", ai_queue().limit());
        }
        Err(e) => tracing::warn!(
            "讀取 AI 請求併行數失敗，使用預設值 {}: {}",
            ai_queue().limit(),
            e
        ),
    }
}

// POST /ai-prompt-settings/revert-parser
pub async fn revert_parser_prompt(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    diesel::update(ai_prompt_settings::table)
        .set((
            ai_prompt_settings::fixed_parser_prompt.eq(Some(DEFAULT_FIXED_PARSER_PROMPT)),
            ai_prompt_settings::updated_at.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "ok": true, "value": DEFAULT_FIXED_PARSER_PROMPT }),
    ))
}

// POST /ai-prompt-settings/revert-filter
pub async fn revert_filter_prompt(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    diesel::update(ai_prompt_settings::table)
        .set((
            ai_prompt_settings::fixed_filter_prompt.eq(Some(DEFAULT_FIXED_FILTER_PROMPT)),
            ai_prompt_settings::updated_at.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "ok": true, "value": DEFAULT_FIXED_FILTER_PROMPT }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrency_must_be_within_the_queue_limits() {
        assert!(valid_concurrency(1));
        assert!(valid_concurrency(MAX_CONCURRENCY as i32));
        assert!(!valid_concurrency(0));
        assert!(!valid_concurrency(MAX_CONCURRENCY as i32 + 1));
    }
}
