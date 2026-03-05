use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use diesel::prelude::*;
use serde::Deserialize;

use crate::ai::prompts::{DEFAULT_FIXED_FILTER_PROMPT, DEFAULT_FIXED_PARSER_PROMPT};
use crate::models::{AiPromptSettings, AiSettings, UpdateAiSettings};
use crate::schema::{ai_prompt_settings, ai_settings};
use crate::state::AppState;

// GET /ai-settings
pub async fn get_ai_settings(
    State(state): State<AppState>,
) -> Result<Json<AiSettings>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let settings = ai_settings::table
        .first::<AiSettings>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // 遮罩 api_key
    Ok(Json(AiSettings {
        api_key: "•".repeat(8),
        ..settings
    }))
}

#[derive(Debug, Deserialize)]
pub struct UpdateAiSettingsRequest {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
}

// PUT /ai-settings
pub async fn update_ai_settings(
    State(state): State<AppState>,
    Json(req): Json<UpdateAiSettingsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = Utc::now().naive_utc();
    diesel::update(ai_settings::table)
        .set(UpdateAiSettings {
            base_url: req.base_url,
            api_key: req.api_key,
            model_name: req.model_name,
            updated_at: now,
        })
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

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

#[derive(Debug, Deserialize)]
pub struct UpdateAiPromptSettingsRequest {
    pub fixed_parser_prompt: Option<String>,
    pub fixed_filter_prompt: Option<String>,
    pub custom_parser_prompt: Option<String>,
    pub custom_filter_prompt: Option<String>,
}

// PUT /ai-prompt-settings
pub async fn update_ai_prompt_settings(
    State(state): State<AppState>,
    Json(req): Json<UpdateAiPromptSettingsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = Utc::now().naive_utc();
    diesel::update(ai_prompt_settings::table)
        .set((
            ai_prompt_settings::fixed_parser_prompt.eq(req.fixed_parser_prompt),
            ai_prompt_settings::fixed_filter_prompt.eq(req.fixed_filter_prompt),
            ai_prompt_settings::custom_parser_prompt.eq(req.custom_parser_prompt),
            ai_prompt_settings::custom_filter_prompt.eq(req.custom_filter_prompt),
            ai_prompt_settings::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
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
            ai_prompt_settings::fixed_parser_prompt
                .eq(Some(DEFAULT_FIXED_PARSER_PROMPT)),
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
            ai_prompt_settings::fixed_filter_prompt
                .eq(Some(DEFAULT_FIXED_FILTER_PROMPT)),
            ai_prompt_settings::updated_at.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "ok": true, "value": DEFAULT_FIXED_FILTER_PROMPT }),
    ))
}

// POST /ai-settings/test
pub async fn test_ai_connection(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match crate::ai::parser_generator::build_ai_client(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        Some(client) => {
            use crate::ai::client::AiClient;
            match client.chat_completion("", "Reply with a simple json: {\"ok\": true}").await {
                Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
                Err(e) => Ok(Json(
                    serde_json::json!({ "ok": false, "error": e.to_string() }),
                )),
            }
        }
        None => Ok(Json(
            serde_json::json!({ "ok": false, "error": "AI 未設定" }),
        )),
    }
}
