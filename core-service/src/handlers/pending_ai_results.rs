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
    pub generated_data: Option<serde_json::Value>,
    pub confirm_level: Option<String>,
    pub confirm_target_id: Option<i32>,
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
            pending_ai_results::generated_data.eq(req.generated_data),
            pending_ai_results::confirm_level.eq(req.confirm_level),
            pending_ai_results::confirm_target_id.eq(req.confirm_target_id),
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

            // 觸發 re-run + conflict detection（背景非同步）
            let pool_arc = Arc::new(pool.clone());
            let conflict_detection = state.conflict_detection.clone();
            tokio::spawn(async move {
                if let Err(e) = rerun_unmatched_raw_items(pool_arc).await {
                    tracing::warn!("rerun_unmatched_raw_items 失敗: {}", e);
                }
                // 解析完成後重跑 conflict detection，觸發 AI filter 生成
                if let Err(e) = conflict_detection.detect_and_mark_conflicts().await {
                    tracing::warn!("conflict detection after parser confirm 失敗: {}", e);
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

            // 觸發 filter recalc + conflict detection（背景非同步）
            let db = pool.clone();
            let conflict_detection = state.conflict_detection.clone();
            let cancel_service = state.cancel_service.clone();
            let dispatch_service = state.dispatch_service.clone();
            let target_id = req.target_id;
            tokio::spawn(async move {
                let recalc = if let Ok(mut conn) = db.get() {
                    match crate::services::filter_recalc::recalculate_filtered_flags(&mut conn, target_type, target_id) {
                        Ok(r) => {
                            tracing::info!("filter_recalc after confirm: updated {} links", r.updated_count);
                            r
                        }
                        Err(e) => {
                            tracing::error!("filter_recalc after confirm failed: {}", e);
                            crate::services::filter_recalc::FilterRecalcResult {
                                updated_count: 0,
                                newly_filtered: vec![],
                                newly_unfiltered: vec![],
                            }
                        }
                    }
                } else {
                    crate::services::filter_recalc::FilterRecalcResult {
                        updated_count: 0,
                        newly_filtered: vec![],
                        newly_unfiltered: vec![],
                    }
                };

                if !recalc.newly_filtered.is_empty() {
                    match cancel_service.cancel_downloads_for_links(&recalc.newly_filtered).await {
                        Ok(n) => tracing::info!("Cancelled {} downloads for newly filtered links", n),
                        Err(e) => tracing::warn!("Failed to cancel downloads: {}", e),
                    }
                }

                let auto_dispatch_ids = match conflict_detection.detect_and_mark_conflicts().await {
                    Ok(result) => result.auto_dispatch_link_ids,
                    Err(e) => {
                        tracing::error!("conflict detection after filter confirm failed: {}", e);
                        vec![]
                    }
                };

                let mut to_dispatch = recalc.newly_unfiltered;
                to_dispatch.extend(auto_dispatch_ids);
                to_dispatch.sort_unstable();
                to_dispatch.dedup();
                if !to_dispatch.is_empty() {
                    match dispatch_service.dispatch_new_links(to_dispatch).await {
                        Ok(r) => tracing::info!(
                            "Re-dispatched after filter confirm: {} dispatched, {} no_downloader, {} failed",
                            r.dispatched, r.no_downloader, r.failed
                        ),
                        Err(e) => tracing::warn!("Failed to dispatch after filter confirm: {}", e),
                    }
                }
            });
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

    diesel::delete(pending_ai_results::table.find(id))
        .execute(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct RegenerateRequest {
    pub custom_prompt: Option<String>,
    pub fixed_prompt: Option<String>,
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
                req.fixed_prompt,
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
                req.fixed_prompt,
            )
            .await
        }
        _ => Err("未知的 result_type".to_string()),
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(result))
}

/// 重新解析所有未匹配的 raw_items：先用現有解析器 re-parse，仍失敗的按 subscription 觸發批次 AI 生成
pub(crate) async fn rerun_unmatched_raw_items(pool: Arc<DbPool>) -> Result<(), String> {
    use crate::models::RawAnimeItem;
    use crate::schema::raw_anime_items;
    use crate::services::title_parser::{ParseStatus, TitleParserService};
    use std::collections::HashSet;

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

    // 先嘗試用現有解析器重新解析，並建立 anime_links
    let mut new_link_ids: Vec<i32> = Vec::new();
    let mut still_unmatched_subs: HashSet<i32> = HashSet::new();
    for item in &items {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        match TitleParserService::parse_title(&mut conn, &item.title) {
            Ok(Some(parsed)) => {
                match crate::handlers::fetcher_results::process_parsed_result(&mut conn, item, &parsed) {
                    Ok(link_ids) => {
                        new_link_ids.extend(link_ids);
                        TitleParserService::update_raw_item_status(
                            &mut conn, item.item_id, ParseStatus::Parsed,
                            Some(parsed.parser_id), None,
                        ).ok();
                    }
                    Err(e) => {
                        TitleParserService::update_raw_item_status(
                            &mut conn, item.item_id, ParseStatus::Failed,
                            Some(parsed.parser_id), Some(&e),
                        ).ok();
                    }
                }
            }
            Ok(None) => {
                still_unmatched_subs.insert(item.subscription_id);
            }
            Err(_) => {}
        }
    }

    // 派送新建立的下載連結
    if !new_link_ids.is_empty() {
        let dispatch = crate::services::DownloadDispatchService::new(pool.as_ref().clone());
        match dispatch.dispatch_new_links(new_link_ids).await {
            Ok(result) => {
                tracing::info!(
                    "rerun 後派送：dispatched={}, no_downloader={}, failed={}",
                    result.dispatched, result.no_downloader, result.failed
                );
            }
            Err(e) => tracing::warn!("rerun 後派送失敗: {}", e),
        }
    }

    // 對每個仍有未匹配項目的訂閱，檢查是否已有進行中的批次 pending，若無則觸發
    for sub_id in still_unmatched_subs {
        let already_generating = {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            pending_ai_results::table
                .filter(pending_ai_results::result_type.eq("parser"))
                .filter(pending_ai_results::subscription_id.eq(sub_id))
                .filter(pending_ai_results::status.eq("generating"))
                .count()
                .get_result::<i64>(&mut conn)
                .unwrap_or(0)
                > 0
        };

        if !already_generating {
            let pool_clone = pool.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::ai::parser_generator::generate_parsers_for_subscription_batch(
                    pool_clone,
                    sub_id,
                ).await {
                    tracing::warn!("Batch parser 生成失敗 subscription={}: {}", sub_id, e);
                }
            });
        }
    }

    Ok(())
}
