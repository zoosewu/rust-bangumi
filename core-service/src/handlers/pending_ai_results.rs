use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use diesel::prelude::*;
use serde::Deserialize;
use std::sync::Arc;

use crate::db::DbPool;
use crate::models::{FilterTargetType, PendingAiResult};
use crate::schema::{filter_rules, pending_ai_results, title_parsers};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListPendingQuery {
    pub result_type: Option<String>,
    pub status: Option<String>,
    pub subscription_id: Option<i32>,
}

// GET /pending-ai-results
pub async fn list_pending(
    State(state): State<AppState>,
    Query(q): Query<ListPendingQuery>,
) -> Result<Json<Vec<PendingAiResult>>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut query = pending_ai_results::table
        .filter(
            pending_ai_results::expires_at
                .is_null()
                .or(pending_ai_results::expires_at.gt(Utc::now().naive_utc())),
        )
        .into_boxed();

    if let Some(t) = q.result_type {
        query = query.filter(pending_ai_results::result_type.eq(t));
    }
    if let Some(s) = q.status {
        query = query.filter(pending_ai_results::status.eq(s));
    }
    if let Some(sub_id) = q.subscription_id {
        query = query.filter(pending_ai_results::subscription_id.eq(sub_id));
    }

    let results = query
        .order(pending_ai_results::created_at.desc())
        .load::<PendingAiResult>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(results))
}

// GET /pending-ai-results/:id
pub async fn get_pending(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<PendingAiResult>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let result = pending_ai_results::table
        .find(id)
        .first::<PendingAiResult>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Not found".to_string()))?;
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct UpdatePendingRequest {
    pub generated_data: serde_json::Value,
}

// PUT /pending-ai-results/:id
pub async fn update_pending(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<UpdatePendingRequest>,
) -> Result<Json<PendingAiResult>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let result = diesel::update(pending_ai_results::table.find(id))
        .set((
            pending_ai_results::generated_data.eq(Some(req.generated_data)),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct ConfirmRequest {
    pub level: String, // "global" | "subscription" | "anime_work"
    pub target_id: Option<i32>,
}

// POST /pending-ai-results/:id/confirm
pub async fn confirm_pending(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<ConfirmRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let pool = state.db.clone();
    let now = Utc::now().naive_utc();
    let expires_at = now + chrono::Duration::days(7);

    let mut conn = pool
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let pending = pending_ai_results::table
        .find(id)
        .first::<PendingAiResult>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Not found".to_string()))?;

    let target_type = match req.level.as_str() {
        "subscription" => FilterTargetType::Subscription,
        "anime_work" => FilterTargetType::AnimeWork,
        _ => FilterTargetType::Global,
    };

    match pending.result_type.as_str() {
        "parser" => {
            diesel::update(
                title_parsers::table.filter(title_parsers::pending_result_id.eq(id)),
            )
            .set((
                title_parsers::pending_result_id.eq(None::<i32>),
                title_parsers::created_from_type.eq(Some(target_type)),
                title_parsers::created_from_id.eq(req.target_id),
                title_parsers::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            // 觸發 re-run（背景非同步）
            let pool_arc = Arc::new(pool.clone());
            tokio::spawn(async move {
                if let Err(e) = rerun_unmatched_raw_items(pool_arc).await {
                    tracing::warn!("rerun_unmatched_raw_items 失敗: {}", e);
                }
            });
        }
        "filter" => {
            diesel::update(
                filter_rules::table.filter(filter_rules::pending_result_id.eq(id)),
            )
            .set((
                filter_rules::pending_result_id.eq(None::<i32>),
                filter_rules::target_type.eq(target_type),
                filter_rules::target_id.eq(req.target_id),
                filter_rules::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        _ => {}
    }

    diesel::update(pending_ai_results::table.find(id))
        .set((
            pending_ai_results::status.eq("confirmed"),
            pending_ai_results::expires_at.eq(Some(expires_at)),
            pending_ai_results::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// POST /pending-ai-results/:id/reject
pub async fn reject_pending(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = Utc::now().naive_utc();
    let expires_at = now + chrono::Duration::days(7);

    let pending = pending_ai_results::table
        .find(id)
        .first::<PendingAiResult>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Not found".to_string()))?;

    match pending.result_type.as_str() {
        "parser" => {
            diesel::delete(
                title_parsers::table.filter(title_parsers::pending_result_id.eq(id)),
            )
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        "filter" => {
            diesel::delete(
                filter_rules::table.filter(filter_rules::pending_result_id.eq(id)),
            )
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        _ => {}
    }

    diesel::update(pending_ai_results::table.find(id))
        .set((
            pending_ai_results::status.eq("failed"),
            pending_ai_results::expires_at.eq(Some(expires_at)),
            pending_ai_results::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct RegenerateRequest {
    pub custom_prompt: Option<String>,
}

// POST /pending-ai-results/:id/regenerate
pub async fn regenerate_pending(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<RegenerateRequest>,
) -> Result<Json<PendingAiResult>, (StatusCode, String)> {
    let pool = Arc::new(state.db.clone());

    let (result_type, source_title) = {
        let mut conn = pool
            .get()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let p = pending_ai_results::table
            .find(id)
            .first::<PendingAiResult>(&mut conn)
            .map_err(|_| (StatusCode::NOT_FOUND, "Not found".to_string()))?;

        // 先刪除舊的 parser/filter
        match p.result_type.as_str() {
            "parser" => {
                diesel::delete(
                    title_parsers::table.filter(title_parsers::pending_result_id.eq(id)),
                )
                .execute(&mut conn)
                .ok();
            }
            "filter" => {
                diesel::delete(
                    filter_rules::table.filter(filter_rules::pending_result_id.eq(id)),
                )
                .execute(&mut conn)
                .ok();
            }
            _ => {}
        }

        // 重設為 generating
        diesel::update(pending_ai_results::table.find(id))
            .set((
                pending_ai_results::status.eq("generating"),
                pending_ai_results::error_message.eq(None::<String>),
                pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        (p.result_type, p.source_title)
    };

    let result = match result_type.as_str() {
        "parser" => {
            crate::ai::parser_generator::generate_parser_for_title(
                pool,
                source_title,
                None,
                req.custom_prompt,
            )
            .await
        }
        "filter" => {
            crate::ai::filter_generator::generate_filter_for_conflict(
                pool,
                vec![source_title.clone()],
                source_title,
                req.custom_prompt,
                None,
            )
            .await
        }
        _ => Err("未知的 result_type".to_string()),
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(result))
}

/// 重新解析所有未匹配的 raw_items，對第一個仍然失敗的觸發新一輪 AI 生成
async fn rerun_unmatched_raw_items(pool: Arc<DbPool>) -> Result<(), String> {
    use crate::models::RawAnimeItem;
    use crate::schema::raw_anime_items;
    use crate::services::title_parser::{ParseStatus, TitleParserService};

    let items: Vec<RawAnimeItem> = {
        let mut conn = pool.get().map_err(|e: diesel::r2d2::PoolError| e.to_string())?;
        raw_anime_items::table
            .filter(
                raw_anime_items::status
                    .eq(ParseStatus::NoMatch.as_str())
                    .or(raw_anime_items::status.eq(ParseStatus::Failed.as_str())),
            )
            .load::<RawAnimeItem>(&mut conn)
            .map_err(|e| e.to_string())?
    };

    for item in &items {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        match TitleParserService::parse_title(&mut conn, &item.title) {
            Ok(Some(_)) => {
                TitleParserService::update_raw_item_status(
                    &mut conn,
                    item.item_id,
                    ParseStatus::Parsed,
                    None,
                    None,
                )
                .ok();
            }
            Ok(None) => {
                let already_pending: bool = pending_ai_results::table
                    .filter(pending_ai_results::result_type.eq("parser"))
                    .filter(pending_ai_results::source_title.eq(&item.title))
                    .filter(
                        pending_ai_results::status
                            .eq_any(vec!["generating", "pending"]),
                    )
                    .count()
                    .get_result::<i64>(&mut conn)
                    .unwrap_or(0)
                    > 0;

                if !already_pending {
                    let pool_clone = pool.clone();
                    let title = item.title.clone();
                    let item_id = item.item_id;
                    tokio::spawn(async move {
                        if let Err(e) =
                            crate::ai::parser_generator::generate_parser_for_title(
                                pool_clone,
                                title,
                                Some(item_id),
                                None,
                            )
                            .await
                        {
                            tracing::warn!("AI parser 生成觸發失敗: {}", e);
                        }
                    });
                    break;
                }
            }
            Err(_) => {}
        }
    }
    Ok(())
}
