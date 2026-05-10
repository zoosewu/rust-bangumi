use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use diesel::prelude::*;
use serde::Deserialize;

use crate::ai::prompts::{DEFAULT_FIXED_FILTER_PROMPT, DEFAULT_FIXED_PARSER_PROMPT};
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
