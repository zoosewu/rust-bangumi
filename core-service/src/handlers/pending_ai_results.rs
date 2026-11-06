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

/// The scope a confirmed result is bound to. A subscription-level confirmation without a
/// target falls back to the subscription the result was generated for, so it never
/// produces a subscription-scoped parser or rule bound to no subscription.
fn confirm_target(
    req: &ConfirmRequest,
    pending_subscription_id: Option<i32>,
) -> Result<(FilterTargetType, Option<i32>), String> {
    match req.level.as_str() {
        "subscription" => req
            .target_id
            .or(pending_subscription_id)
            .map(|id| (FilterTargetType::Subscription, Some(id)))
            .ok_or_else(|| "subscription level requires target_id".to_string()),
        "anime_work" => Ok((FilterTargetType::AnimeWork, req.target_id)),
        _ => Ok((FilterTargetType::Global, None)),
    }
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

    let (target_type, target_id) =
        confirm_target(&req, pending.subscription_id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    match pending.result_type.as_str() {
        "parser" => {
            diesel::update(title_parsers::table.filter(title_parsers::pending_result_id.eq(id)))
                .set((
                    title_parsers::pending_result_id.eq(None::<i32>),
                    title_parsers::created_from_type.eq(Some(target_type)),
                    title_parsers::created_from_id.eq(target_id),
                    title_parsers::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            // 觸發 re-run + conflict detection + 派送（背景非同步）
            let pool_arc = Arc::new(pool.clone());
            let pool_arc_for_filters = pool_arc.clone();
            let subscription_id = pending.subscription_id;
            let conflict_detection = state.conflict_detection.clone();
            let dispatch_service = state.dispatch_service.clone();
            tokio::spawn(async move {
                let new_link_ids = rerun_unmatched_raw_items(pool_arc)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!("rerun_unmatched_raw_items 失敗: {}", e);
                        vec![]
                    });
                // 解析完成後重跑 conflict detection（觸發 AI filter 生成），再派送新連結
                match dispatch_service
                    .detect_conflicts_and_dispatch(&conflict_detection, new_link_ids)
                    .await
                {
                    Ok(r) => {
                        tracing::info!(
                            "parser confirm 後派送：dispatched={}, no_downloader={}, failed={}",
                            r.dispatched,
                            r.no_downloader,
                            r.failed
                        );
                        if let Some(sub_id) = subscription_id {
                            crate::ai::auto_trigger::spawn_filter_generation_if_due(
                                pool_arc_for_filters,
                                sub_id,
                            );
                        }
                    }
                    Err(e) => tracing::warn!("parser confirm 後派送失敗: {}", e),
                }
            });
        }
        "filter" => {
            // 從 generated_data（使用者可能已編輯）建立規則；確認前規則不存在於 filter_rules
            let rules_data = pending
                .generated_data
                .as_ref()
                .and_then(|d| d.get("rules"))
                .and_then(|r| r.as_array())
                .cloned()
                .unwrap_or_default();

            use crate::models::NewFilterRule;
            for (i, rule) in rules_data.iter().enumerate() {
                let new_rule = NewFilterRule {
                    rule_order: rule
                        .get("rule_order")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(i as i64 + 1) as i32,
                    regex_pattern: rule
                        .get("regex_pattern")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    is_positive: rule
                        .get("is_positive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    target_type,
                    target_id,
                    created_at: now,
                    updated_at: now,
                };
                diesel::insert_into(filter_rules::table)
                    .values(&new_rule)
                    .execute(&mut conn)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }

            // 觸發 filter recalc + conflict detection（背景非同步）
            let db = pool.clone();
            let conflict_detection = state.conflict_detection.clone();
            let cancel_service = state.cancel_service.clone();
            let dispatch_service = state.dispatch_service.clone();
            tokio::spawn(async move {
                let recalc = if let Ok(mut conn) = db.get() {
                    match crate::services::filter_recalc::recalculate_filtered_flags(
                        &mut conn,
                        target_type,
                        target_id,
                    ) {
                        Ok(r) => {
                            tracing::info!(
                                "filter_recalc after confirm: updated {} links",
                                r.updated_count
                            );
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
                    match cancel_service
                        .cancel_downloads_for_links(&recalc.newly_filtered)
                        .await
                    {
                        Ok(n) => {
                            tracing::info!("Cancelled {} downloads for newly filtered links", n)
                        }
                        Err(e) => tracing::warn!("Failed to cancel downloads: {}", e),
                    }
                }

                match dispatch_service
                    .detect_conflicts_and_dispatch(&conflict_detection, recalc.newly_unfiltered)
                    .await
                {
                    Ok(r) => tracing::info!(
                        "Re-dispatched after filter confirm: {} dispatched, {} no_downloader, {} failed",
                        r.dispatched, r.no_downloader, r.failed
                    ),
                    Err(e) => tracing::warn!("Failed to dispatch after filter confirm: {}", e),
                }
                // 全域 / 訂閱層級的規則也影響解析前過濾
                crate::services::prefilter::refresh_after_rule_change(
                    std::sync::Arc::new(db.clone()),
                    target_type,
                    target_id,
                );
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

    // 待確認的 filter 只存在 generated_data；parser 則已寫入 title_parsers，要一併刪除
    if pending.result_type == "parser" {
        diesel::delete(title_parsers::table.filter(title_parsers::pending_result_id.eq(id)))
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
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

    let (result_type, source_title, batch_subscription) = {
        let mut conn = pool
            .get()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let p = pending_ai_results::table
            .find(id)
            .first::<PendingAiResult>(&mut conn)
            .map_err(|_| (StatusCode::NOT_FOUND, "Not found".to_string()))?;
        // 訂閱的批次結果沒有真正的來源標題（source_title 是「批次解析（iteration 1）」），
        // 重新生成就是對那個訂閱重跑一次批次產生
        let batch_subscription = p
            .subscription_id
            .filter(|_| p.source_title.starts_with("批次"));

        // 先刪除舊的 parser（待確認的 filter 只存在 generated_data）
        if p.result_type == "parser" {
            diesel::delete(title_parsers::table.filter(title_parsers::pending_result_id.eq(id)))
                .execute(&mut conn)
                .ok();
        }

        // 單筆：重設為 generating 再產生；批次：舊結果標成已取代，由新的批次結果接手
        diesel::update(pending_ai_results::table.find(id))
            .set((
                pending_ai_results::status.eq(if batch_subscription.is_some() {
                    "rejected"
                } else {
                    "generating"
                }),
                pending_ai_results::error_message.eq(None::<String>),
                pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        (p.result_type, p.source_title, batch_subscription)
    };

    if let Some(subscription_id) = batch_subscription {
        return regenerate_batch(pool, &result_type, subscription_id).await;
    }

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

/// 對訂閱重跑一次批次產生（使用者按「重新生成」），回傳新的結果。
/// 同一訂閱同一種結果已經在產生時回 409。
async fn regenerate_batch(
    pool: Arc<DbPool>,
    result_type: &str,
    subscription_id: i32,
) -> Result<Json<PendingAiResult>, (StatusCode, String)> {
    let kind: &'static str = if result_type == "filter" {
        "filter"
    } else {
        "parser"
    };
    let _claim = crate::ai::auto_trigger::claim(kind, subscription_id).ok_or((
        StatusCode::CONFLICT,
        "這個訂閱正在產生中，請稍後再試".to_string(),
    ))?;
    let generated = if kind == "filter" {
        let groups = {
            let mut conn = pool
                .get()
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            crate::ai::auto_trigger::conflict_groups_for_subscription(&mut conn, subscription_id)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        };
        if groups.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "這個訂閱目前沒有衝突".to_string()));
        }
        crate::ai::filter_generator::generate_filters_for_subscription_batch(
            pool.clone(),
            Some(subscription_id),
            groups,
        )
        .await
    } else {
        crate::ai::parser_generator::generate_parsers_for_subscription_batch(
            pool.clone(),
            subscription_id,
        )
        .await
    };
    generated.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut conn = pool
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    pending_ai_results::table
        .filter(pending_ai_results::result_type.eq(kind))
        .filter(pending_ai_results::subscription_id.eq(subscription_id))
        .order(pending_ai_results::id.desc())
        .first::<PendingAiResult>(&mut conn)
        .map(Json)
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "沒有產生新的結果（這個訂閱可能已經沒有未匹配的標題）".to_string(),
            )
        })
}

/// 重新解析所有未匹配的 raw_items：先用現有解析器 re-parse，仍失敗的按 subscription 觸發批次 AI 生成。
/// 回傳新建立的 anime_link id；呼叫端要用 `detect_conflicts_and_dispatch` 派送，
/// 不能直接派送，否則同一集的多個版本會在衝突標記之前全部被下載。
pub(crate) async fn rerun_unmatched_raw_items(pool: Arc<DbPool>) -> Result<Vec<i32>, String> {
    use crate::models::RawAnimeItem;
    use crate::schema::raw_anime_items;
    use crate::services::title_parser::{ParseStatus, TitleParserService};
    use std::collections::HashSet;

    let items: Vec<RawAnimeItem> = {
        let mut conn = pool
            .get()
            .map_err(|e: diesel::r2d2::PoolError| e.to_string())?;
        raw_anime_items::table
            .filter(
                raw_anime_items::status
                    .eq(ParseStatus::NoMatch.as_str())
                    .or(raw_anime_items::status.eq(ParseStatus::Failed.as_str())),
            )
            .load::<RawAnimeItem>(&mut conn)
            .map_err(|e| e.to_string())?
    };

    let identity = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        crate::handlers::fetcher_results::prepare_identity(&mut conn, &pool, &items).await
    };

    // 先嘗試用現有解析器重新解析，並建立 anime_links
    let mut new_link_ids: Vec<i32> = Vec::new();
    let mut still_unmatched_subs: HashSet<i32> = HashSet::new();
    for item in &items {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        // 全域 / 訂閱層級的過濾規則在解析前套用：被排除的標成 filtered，不算未匹配
        if crate::services::prefilter::excluded_for(&mut conn, item.subscription_id, &item.title) {
            crate::services::prefilter::mark_filtered(&mut conn, item.item_id).ok();
            continue;
        }
        match TitleParserService::parse_title(&mut conn, &item.title, item.subscription_id) {
            Ok(Some(parsed)) => {
                match crate::handlers::fetcher_results::process_parsed_result(
                    &mut conn, &identity, item, &parsed,
                ) {
                    Ok(link_ids) => {
                        new_link_ids.extend(link_ids);
                        TitleParserService::update_raw_item_status(
                            &mut conn,
                            item.item_id,
                            ParseStatus::Parsed,
                            Some(parsed.parser_id),
                            None,
                        )
                        .ok();
                    }
                    Err(e) => {
                        TitleParserService::update_raw_item_status(
                            &mut conn,
                            item.item_id,
                            ParseStatus::Failed,
                            Some(parsed.parser_id),
                            Some(&e),
                        )
                        .ok();
                    }
                }
            }
            Ok(None) => {
                still_unmatched_subs.insert(item.subscription_id);
            }
            Err(_) => {}
        }
    }

    // 對每個仍有未匹配項目的訂閱，沒有等待確認 / 產生中的解析器時觸發（同一訂閱同時只跑一個）
    for sub_id in still_unmatched_subs {
        crate::ai::auto_trigger::spawn_parser_generation_if_due(pool.clone(), sub_id);
    }

    Ok(new_link_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(level: &str, target_id: Option<i32>) -> ConfirmRequest {
        ConfirmRequest {
            level: level.to_string(),
            target_id,
        }
    }

    #[test]
    fn subscription_confirm_without_target_binds_the_source_subscription() {
        assert_eq!(
            confirm_target(&req("subscription", None), Some(27)).unwrap(),
            (FilterTargetType::Subscription, Some(27))
        );
        assert_eq!(
            confirm_target(&req("subscription", Some(8)), Some(27)).unwrap(),
            (FilterTargetType::Subscription, Some(8))
        );
    }

    #[test]
    fn subscription_confirm_with_no_subscription_is_rejected() {
        assert!(confirm_target(&req("subscription", None), None).is_err());
    }

    #[test]
    fn global_confirm_has_no_target() {
        assert_eq!(
            confirm_target(&req("global", Some(3)), Some(27)).unwrap(),
            (FilterTargetType::Global, None)
        );
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn regenerating_a_batch_result_reruns_the_subscription_batch() {
        use crate::services::identity::test_db;
        #[derive(diesel::QueryableByName)]
        struct Id {
            #[diesel(sql_type = diesel::sql_types::Integer)]
            id: i32,
        }
        let pool = Arc::new(test_db::pool());
        let (sub, old) = {
            let mut conn = pool.get().unwrap();
            diesel::sql_query("UPDATE ai_providers SET is_enabled = false")
                .execute(&mut conn)
                .unwrap();
            let sub = diesel::sql_query("INSERT INTO subscriptions (fetcher_id, source_url, name, created_at, updated_at) SELECT module_id, 'https://regen-test', 's', now(), now() FROM service_modules LIMIT 1 RETURNING subscription_id AS id")
                .get_result::<Id>(&mut conn).unwrap().id;
            diesel::sql_query(format!("INSERT INTO raw_anime_items (title, download_url, subscription_id, status, created_at) VALUES ('[G] T 第01-12话 合集', 'u-regen', {sub}, 'no_match', now())"))
                .execute(&mut conn).unwrap();
            let old = diesel::sql_query(format!("INSERT INTO pending_ai_results (result_type, source_title, status, subscription_id) VALUES ('parser', '批次解析（iteration 1）', 'pending', {sub}) RETURNING id"))
                .get_result::<Id>(&mut conn).unwrap().id;
            (sub, old)
        };

        let held = crate::ai::auto_trigger::claim("parser", sub).unwrap();
        let busy = regenerate_batch(pool.clone(), "parser", sub)
            .await
            .unwrap_err();
        assert_eq!(busy.0, StatusCode::CONFLICT);
        drop(held);

        // 沒有啟用的 provider：批次產生會建立一筆失敗的新結果
        let Json(result) = regenerate_batch(pool.clone(), "parser", sub).await.unwrap();
        assert_ne!(result.id, old, "a new batch result, not the old one");
        assert_eq!(result.subscription_id, Some(sub));
        assert!(result.source_title.starts_with("批次解析"));
    }
}
