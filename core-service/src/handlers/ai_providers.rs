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
    AiProvider {
        api_key: MASKED_API_KEY.into(),
        ..p
    }
}

fn validate_kind(kind: &str) -> Result<(), (StatusCode, String)> {
    if !ALLOWED_KINDS.contains(&kind) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("invalid provider_kind: {kind}"),
        ));
    }
    Ok(())
}

fn validate_mode(mode: &str) -> Result<(), (StatusCode, String)> {
    if !ALLOWED_FORMAT_MODES.contains(&mode) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("invalid response_format_mode: {mode}"),
        ));
    }
    Ok(())
}

fn validate_required_config(base_url: &str, model_name: &str) -> Result<(), (StatusCode, String)> {
    if base_url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Base URL is required".into()));
    }
    if model_name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Model name is required".into()));
    }
    Ok(())
}

fn load_provider_api_key(
    conn: &mut PgConnection,
    id: Option<i32>,
) -> Result<Option<String>, (StatusCode, String)> {
    match id {
        Some(id) => ai_providers::table
            .find(id)
            .select(ai_providers::api_key)
            .first::<String>(conn)
            .optional()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or((StatusCode::NOT_FOUND, format!("provider {id} not found")))
            .map(Some),
        None => Ok(None),
    }
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

fn default_max_tokens() -> i32 {
    4096
}
fn default_format_mode() -> String {
    "non_strict".into()
}
fn default_true() -> bool {
    true
}

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

async fn run_provider_test(
    provider: AiProvider,
) -> Result<Json<TestResponse>, (StatusCode, String)> {
    validate_required_config(&provider.base_url, &provider.model_name)?;
    let client =
        crate::ai::factory::build_provider(&provider).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let result = client.chat_completion("", "Reply with OK.").await;
    match result {
        Ok(_) => Ok(Json(TestResponse {
            ok: true,
            error: None,
        })),
        Err(e) => Ok(Json(TestResponse {
            ok: false,
            error: Some(e.to_string()),
        })),
    }
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

    run_provider_test(provider).await
}

#[derive(Debug, Deserialize)]
pub struct TestAiProviderRequest {
    pub existing_provider_id: Option<i32>,
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
}

fn provider_from_test_request(
    req: TestAiProviderRequest,
    fallback_api_key: Option<String>,
) -> Result<AiProvider, (StatusCode, String)> {
    validate_kind(&req.provider_kind)?;
    validate_mode(&req.response_format_mode)?;
    let api_key = if req.api_key.trim().is_empty() {
        fallback_api_key.unwrap_or_default()
    } else {
        req.api_key
    };
    validate_required_config(&req.base_url, &req.model_name)?;

    Ok(AiProvider {
        id: 0,
        name: "test".into(),
        provider_kind: req.provider_kind,
        base_url: req.base_url,
        api_key,
        model_name: req.model_name,
        max_tokens: req.max_tokens,
        response_format_mode: req.response_format_mode,
        is_enabled: true,
        priority: 0,
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    })
}

pub async fn test_ai_provider_config(
    State(state): State<AppState>,
    Json(req): Json<TestAiProviderRequest>,
) -> Result<Json<TestResponse>, (StatusCode, String)> {
    let fallback_api_key = {
        let mut conn = state
            .db
            .get()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        load_provider_api_key(&mut conn, req.existing_provider_id)?
    };
    let provider = provider_from_test_request(req, fallback_api_key)?;
    run_provider_test(provider).await
}

#[derive(Debug, Deserialize)]
pub struct PromptTestRequest {
    /// 要測試的 provider 設定（與 `/ai-providers/test` 相同；api_key 留空就用已存的 key）
    #[serde(flatten)]
    pub provider: TestAiProviderRequest,
    pub kind: crate::ai::prompt_test::PromptKind,
    pub subscription_id: i32,
    /// 只用於這次測試的 prompt（比較不同寫法用，不寫進設定）
    #[serde(flatten)]
    pub overrides: crate::ai::prompt_test::PromptOverrides,
}

#[derive(Debug, Serialize)]
pub struct PromptTestResponse {
    pub ok: bool,
    pub elapsed_ms: u128,
    pub system_prompt: String,
    pub user_prompt: String,
    pub titles: Vec<String>,
    pub raw_response: Option<String>,
    pub parsed: Option<serde_json::Value>,
    pub evaluations: Vec<crate::ai::prompt_test::TitleEvaluation>,
    pub error: Option<String>,
}

/// POST /ai-providers/test-prompt — 用設定好的真實 prompt 與某訂閱的真實標題測試一個 provider。
/// 只送給這個 provider（不 fallback）、不建立待確認結果；會排進全域 AI 佇列，避免和正在跑的生成搶額度。
pub async fn test_provider_with_real_prompt(
    State(state): State<AppState>,
    Json(req): Json<PromptTestRequest>,
) -> Result<Json<PromptTestResponse>, (StatusCode, String)> {
    use crate::ai::prompt_test::{build_sample, evaluate, parse_response};
    let (fallback_api_key, sample) = {
        let mut conn = state
            .db
            .get()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let key = load_provider_api_key(&mut conn, req.provider.existing_provider_id)?;
        let sample = build_sample(&mut conn, req.kind, req.subscription_id, &req.overrides)
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        (key, sample)
    };
    let provider = provider_from_test_request(req.provider, fallback_api_key)?;
    let client =
        crate::ai::factory::build_provider(&provider).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let _permit = crate::ai::queue::ai_queue().acquire().await;
    let started = std::time::Instant::now();
    let result = client
        .chat_completion_structured(&sample.system_prompt, &sample.user_prompt, &sample.schema)
        .await;
    let elapsed_ms = started.elapsed().as_millis();

    let mut response = PromptTestResponse {
        ok: false,
        elapsed_ms,
        system_prompt: sample.system_prompt,
        user_prompt: sample.user_prompt,
        titles: sample.titles,
        raw_response: None,
        parsed: None,
        evaluations: vec![],
        error: None,
    };
    match result {
        Err(e) => response.error = Some(e.to_string()),
        Ok(raw) => {
            match parse_response(req.kind, &raw)
                .and_then(|data| evaluate(req.kind, &data, &response.titles).map(|ev| (data, ev)))
            {
                Ok((data, evaluations)) => {
                    response.ok = true;
                    response.parsed = Some(data);
                    response.evaluations = evaluations;
                }
                Err(e) => response.error = Some(e),
            }
            response.raw_response = Some(raw);
        }
    }
    Ok(Json(response))
}

/// 複本的名稱
fn copy_name(name: &str) -> String {
    format!("{name}（複本）")
}

/// 複製一個 provider（含 API key），複本停用並排在最後；來源不存在時回傳 None
fn duplicate_provider(conn: &mut PgConnection, id: i32) -> QueryResult<Option<AiProvider>> {
    let Some(source) = ai_providers::table
        .find(id)
        .first::<AiProvider>(conn)
        .optional()?
    else {
        return Ok(None);
    };
    let next_priority: Option<i32> = ai_providers::table
        .select(diesel::dsl::max(ai_providers::priority))
        .first(conn)?;
    diesel::insert_into(ai_providers::table)
        .values(&NewAiProvider {
            name: copy_name(&source.name),
            provider_kind: source.provider_kind,
            base_url: source.base_url,
            api_key: source.api_key,
            model_name: source.model_name,
            max_tokens: source.max_tokens,
            response_format_mode: source.response_format_mode,
            is_enabled: false,
            priority: next_priority.map(|v| v + 1).unwrap_or(0),
        })
        .get_result(conn)
        .map(Some)
}

/// POST /ai-providers/:id/duplicate — 在伺服器端複製設定（含 API key，前端不會看到 key）。
/// 複本預設停用並排在最後，改好模型名稱等設定後再啟用。
pub async fn duplicate_ai_provider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<AiProvider>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    duplicate_provider(&mut conn, id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(|copy| Json(mask(copy)))
        .ok_or((StatusCode::NOT_FOUND, format!("provider {id} not found")))
}

#[derive(Debug, Deserialize)]
pub struct ListModelsRequest {
    /// 編輯既有 provider 時帶入；api_key 留空就用它已存的 key
    pub existing_provider_id: Option<i32>,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Serialize)]
pub struct ListModelsResponse {
    pub models: Vec<String>,
}

/// POST /ai-providers/models — 向 provider 取得模型列表（OpenAI 相容的 `GET /models`）
pub async fn list_provider_models(
    State(state): State<AppState>,
    Json(req): Json<ListModelsRequest>,
) -> Result<Json<ListModelsResponse>, (StatusCode, String)> {
    if req.base_url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Base URL is required".into()));
    }
    let api_key = if req.api_key.trim().is_empty() {
        let mut conn = state
            .db
            .get()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        load_provider_api_key(&mut conn, req.existing_provider_id)?.unwrap_or_default()
    } else {
        req.api_key
    };
    crate::ai::openai::list_models(&req.base_url, &api_key)
        .await
        .map(|models| Json(ListModelsResponse { models }))
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))
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
    #[ignore = "needs DATABASE_TEST_URL"]
    fn a_duplicate_keeps_the_key_but_starts_disabled_at_the_end() {
        let mut conn = crate::services::identity::test_db::conn();
        let source: AiProvider = diesel::insert_into(ai_providers::table)
            .values(&NewAiProvider {
                name: "Groq".into(),
                provider_kind: "openai_compatible".into(),
                base_url: "https://api.groq.com/openai/v1".into(),
                api_key: "gsk-secret".into(),
                model_name: "openai/gpt-oss-120b".into(),
                max_tokens: 4096,
                response_format_mode: "strict".into(),
                is_enabled: true,
                priority: 900,
            })
            .get_result(&mut conn)
            .unwrap();

        let copy = duplicate_provider(&mut conn, source.id).unwrap().unwrap();
        assert_ne!(copy.id, source.id);
        assert_eq!(copy.name, "Groq（複本）");
        assert_eq!(copy.api_key, "gsk-secret");
        assert_eq!(copy.model_name, source.model_name);
        assert!(!copy.is_enabled);
        assert!(copy.priority > source.priority);
        assert_eq!(mask(copy).api_key, MASKED_API_KEY);
        assert!(duplicate_provider(&mut conn, -1).unwrap().is_none());
    }

    #[test]
    fn a_copy_is_named_after_its_source() {
        assert_eq!(copy_name("Groq"), "Groq（複本）");
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

    #[test]
    fn test_request_allows_empty_api_key() {
        let req = TestAiProviderRequest {
            existing_provider_id: None,
            provider_kind: "openai_compatible".into(),
            base_url: "https://api.example.test/v1".into(),
            api_key: "".into(),
            model_name: "gpt-test".into(),
            max_tokens: 4096,
            response_format_mode: "non_strict".into(),
        };

        let p = provider_from_test_request(req, None).unwrap();
        assert_eq!(p.api_key, "");
    }

    #[test]
    fn test_request_can_reuse_existing_api_key() {
        let req = TestAiProviderRequest {
            existing_provider_id: Some(1),
            provider_kind: "openai_compatible".into(),
            base_url: "https://api.example.test/v1".into(),
            api_key: "".into(),
            model_name: "gpt-test".into(),
            max_tokens: 4096,
            response_format_mode: "non_strict".into(),
        };

        let p = provider_from_test_request(req, Some("saved-key".into())).unwrap();
        assert_eq!(p.api_key, "saved-key");
    }
}
