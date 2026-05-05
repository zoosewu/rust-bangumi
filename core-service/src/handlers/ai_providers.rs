use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::{AiProvider, NewAiProvider, UpdateAiProvider};
use crate::schema::ai_providers;
use crate::state::AppState;

const ALLOWED_KINDS: &[&str] = &["openai_compatible"];
const ALLOWED_FORMAT_MODES: &[&str] = &["strict", "non_strict", "inject_schema"];
const MASKED_API_KEY: &str = "••••••••";

fn mask(p: AiProvider) -> AiProvider {
    AiProvider { api_key: MASKED_API_KEY.into(), ..p }
}

fn validate_kind(kind: &str) -> Result<(), (StatusCode, String)> {
    if !ALLOWED_KINDS.contains(&kind) {
        return Err((StatusCode::BAD_REQUEST, format!("invalid provider_kind: {kind}")));
    }
    Ok(())
}

fn validate_mode(mode: &str) -> Result<(), (StatusCode, String)> {
    if !ALLOWED_FORMAT_MODES.contains(&mode) {
        return Err((StatusCode::BAD_REQUEST, format!("invalid response_format_mode: {mode}")));
    }
    Ok(())
}

pub async fn list_ai_providers(
    State(state): State<AppState>,
) -> Result<Json<Vec<AiProvider>>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let providers = ai_providers::table
        .order(ai_providers::priority.asc())
        .then_order_by(ai_providers::id.asc())
        .load::<AiProvider>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(providers.into_iter().map(mask).collect()))
}

pub async fn get_ai_provider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<AiProvider>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let p = ai_providers::table
        .find(id)
        .first::<AiProvider>(&mut conn)
        .optional()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, format!("provider {id} not found")))?;
    Ok(Json(mask(p)))
}

#[derive(Debug, Deserialize)]
pub struct CreateAiProviderRequest {
    pub name: String,
    pub provider_kind: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model_name: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i32,
    #[serde(default = "default_format_mode")]
    pub response_format_mode: String,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

fn default_max_tokens() -> i32 { 4096 }
fn default_format_mode() -> String { "non_strict".into() }
fn default_true() -> bool { true }

pub async fn create_ai_provider(
    State(state): State<AppState>,
    Json(req): Json<CreateAiProviderRequest>,
) -> Result<Json<AiProvider>, (StatusCode, String)> {
    validate_kind(&req.provider_kind)?;
    validate_mode(&req.response_format_mode)?;

    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let next_priority: Option<i32> = ai_providers::table
        .select(diesel::dsl::max(ai_providers::priority))
        .first::<Option<i32>>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let priority = next_priority.map(|v| v + 1).unwrap_or(0);

    let new_p = NewAiProvider {
        name: req.name,
        provider_kind: req.provider_kind,
        base_url: req.base_url,
        api_key: req.api_key,
        model_name: req.model_name,
        max_tokens: req.max_tokens,
        response_format_mode: req.response_format_mode,
        is_enabled: req.is_enabled,
        priority,
    };
    let inserted: AiProvider = diesel::insert_into(ai_providers::table)
        .values(&new_p)
        .get_result(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(mask(inserted)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateAiProviderRequest {
    pub name: Option<String>,
    pub provider_kind: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub max_tokens: Option<i32>,
    pub response_format_mode: Option<String>,
    pub is_enabled: Option<bool>,
}

pub async fn update_ai_provider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<UpdateAiProviderRequest>,
) -> Result<Json<AiProvider>, (StatusCode, String)> {
    if let Some(ref kind) = req.provider_kind {
        validate_kind(kind)?;
    }
    if let Some(ref mode) = req.response_format_mode {
        validate_mode(mode)?;
    }

    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // api_key 為空字串視為「不更新」
    let api_key = req.api_key.filter(|k| !k.is_empty());

    let changes = UpdateAiProvider {
        name: req.name,
        provider_kind: req.provider_kind,
        base_url: req.base_url,
        api_key,
        model_name: req.model_name,
        max_tokens: req.max_tokens,
        response_format_mode: req.response_format_mode,
        is_enabled: req.is_enabled,
        priority: None,
        updated_at: Utc::now().naive_utc(),
    };

    let updated: AiProvider = diesel::update(ai_providers::table.find(id))
        .set(&changes)
        .get_result(&mut conn)
        .optional()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, format!("provider {id} not found")))?;
    Ok(Json(mask(updated)))
}

pub async fn delete_ai_provider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let n = diesel::delete(ai_providers::table.find(id))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, format!("provider {id} not found")));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct ReorderRequest {
    pub ordered_ids: Vec<i32>,
}

pub async fn reorder_ai_providers(
    State(state): State<AppState>,
    Json(req): Json<ReorderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = Utc::now().naive_utc();

    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        for (idx, id) in req.ordered_ids.iter().enumerate() {
            diesel::update(ai_providers::table.find(id))
                .set((
                    ai_providers::priority.eq(idx as i32),
                    ai_providers::updated_at.eq(now),
                ))
                .execute(conn)?;
        }
        Ok(())
    })
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Serialize)]
pub struct TestResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn test_ai_provider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<TestResponse>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let provider: AiProvider = ai_providers::table
        .find(id)
        .first(&mut conn)
        .optional()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, format!("provider {id} not found")))?;

    let client = crate::ai::factory::build_provider(&provider)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let result = client
        .chat_completion("", "Reply with json: {\"ok\": true}")
        .await;
    match result {
        Ok(_) => Ok(Json(TestResponse { ok: true, error: None })),
        Err(e) => Ok(Json(TestResponse { ok: false, error: Some(e.to_string()) })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn provider() -> AiProvider {
        AiProvider {
            id: 1,
            name: "test".into(),
            provider_kind: "openai_compatible".into(),
            base_url: "https://example.com".into(),
            api_key: "secret-key-do-not-leak".into(),
            model_name: "m".into(),
            max_tokens: 4096,
            response_format_mode: "non_strict".into(),
            is_enabled: true,
            priority: 0,
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
        }
    }

    #[test]
    fn mask_replaces_api_key() {
        let masked = mask(provider());
        assert_eq!(masked.api_key, MASKED_API_KEY);
    }

    #[test]
    fn mask_preserves_other_fields() {
        let p = provider();
        let masked = mask(p.clone());
        assert_eq!(masked.id, p.id);
        assert_eq!(masked.name, p.name);
        assert_eq!(masked.base_url, p.base_url);
        assert_eq!(masked.model_name, p.model_name);
        assert_eq!(masked.is_enabled, p.is_enabled);
    }

    #[test]
    fn validate_kind_accepts_known() {
        assert!(validate_kind("openai_compatible").is_ok());
    }

    #[test]
    fn validate_kind_rejects_unknown() {
        let err = validate_kind("anthropic").unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(err.1.contains("anthropic"));
    }

    #[test]
    fn validate_mode_accepts_all_known() {
        for m in ["strict", "non_strict", "inject_schema"] {
            assert!(validate_mode(m).is_ok(), "mode {} should be valid", m);
        }
    }

    #[test]
    fn validate_mode_rejects_unknown() {
        let err = validate_mode("garbage").unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(err.1.contains("garbage"));
    }
}
