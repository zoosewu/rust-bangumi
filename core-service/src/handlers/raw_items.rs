//! 原始資料管理 API

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use crate::models::RawAnimeItem;
use crate::schema::{anime_links, downloads, raw_anime_items};
use crate::services::title_parser::ParseStatus;
use crate::services::TitleParserService;
use crate::state::AppState;

// ============ DTOs ============

#[derive(Debug, Deserialize)]
pub struct ListRawItemsQuery {
    pub status: Option<String>,
    pub subscription_id: Option<i32>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RawItemDownloadInfo {
    pub status: String,
    pub progress: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct RawItemResponse {
    pub item_id: i32,
    pub title: String,
    pub description: Option<String>,
    pub download_url: String,
    pub pub_date: Option<String>,
    pub subscription_id: i32,
    pub status: String,
    pub parser_id: Option<i32>,
    pub error_message: Option<String>,
    pub parsed_at: Option<String>,
    pub created_at: String,
    pub download: Option<RawItemDownloadInfo>,
    pub filter_passed: Option<bool>,
}

impl RawItemResponse {
    fn from_item(item: RawAnimeItem, download: Option<RawItemDownloadInfo>, filter_passed: Option<bool>) -> Self {
        Self {
            item_id: item.item_id,
            title: item.title,
            description: item.description,
            download_url: item.download_url,
            pub_date: item.pub_date.map(|d| d.to_string()),
            subscription_id: item.subscription_id,
            status: item.status,
            parser_id: item.parser_id,
            error_message: item.error_message,
            parsed_at: item.parsed_at.map(|d| d.to_string()),
            created_at: item.created_at.to_string(),
            download,
            filter_passed,
        }
    }
}

/// Batch-load download info for a set of raw_item_ids.
/// Returns a map from item_id to RawItemDownloadInfo.
fn load_download_info(
    conn: &mut diesel::PgConnection,
    item_ids: &[i32],
) -> Result<HashMap<i32, RawItemDownloadInfo>, diesel::result::Error> {
    let rows: Vec<(Option<i32>, String, Option<f32>)> = anime_links::table
        .inner_join(downloads::table.on(downloads::link_id.eq(anime_links::link_id)))
        .filter(anime_links::raw_item_id.eq_any(item_ids))
        .select((
            anime_links::raw_item_id,
            downloads::status,
            downloads::progress,
        ))
        .load(conn)?;

    let mut map = HashMap::new();
    for (raw_item_id, status, progress) in rows {
        if let Some(rid) = raw_item_id {
            // If multiple downloads exist for same item, prefer non-pending / latest
            map.entry(rid)
                .and_modify(|existing: &mut RawItemDownloadInfo| {
                    // Keep the "most progressed" status
                    if status_priority(&status) > status_priority(&existing.status) {
                        existing.status = status.clone();
                        existing.progress = progress;
                    }
                })
                .or_insert(RawItemDownloadInfo { status, progress });
        }
    }
    Ok(map)
}

fn status_priority(status: &str) -> u8 {
    match status {
        "completed" => 4,
        "downloading" => 3,
        "failed" | "no_downloader" => 2,
        "pending" => 1,
        _ => 0,
    }
}

#[derive(Debug, Serialize)]
pub struct ReparseResponse {
    pub success: bool,
    pub items_processed: usize,
    pub items_parsed: usize,
    pub message: String,
}

// ============ Handlers ============

/// GET /raw-items - 列出原始資料
pub async fn list_raw_items(
    State(state): State<AppState>,
    Query(query): Query<ListRawItemsQuery>,
) -> Result<Json<Vec<RawItemResponse>>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut q = raw_anime_items::table.into_boxed();

    if let Some(status) = &query.status {
        q = q.filter(raw_anime_items::status.eq(status));
    }

    if let Some(sub_id) = query.subscription_id {
        q = q.filter(raw_anime_items::subscription_id.eq(sub_id));
    }

    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let items = q
        .order(raw_anime_items::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load::<RawAnimeItem>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let item_ids: Vec<i32> = items.iter().map(|i| i.item_id).collect();
    let dl_map = load_download_info(&mut conn, &item_ids)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        items
            .into_iter()
            .map(|item| {
                let dl = dl_map.get(&item.item_id).map(|d| RawItemDownloadInfo {
                    status: d.status.clone(),
                    progress: d.progress,
                });
                RawItemResponse::from_item(item, dl, None)
            })
            .collect(),
    ))
}

/// GET /raw-items/:item_id - 取得單一項目
pub async fn get_raw_item(
    State(state): State<AppState>,
    Path(item_id): Path<i32>,
) -> Result<Json<RawItemResponse>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let item = raw_anime_items::table
        .filter(raw_anime_items::item_id.eq(item_id))
        .first::<RawAnimeItem>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Item not found".to_string()))?;

    let dl_map = load_download_info(&mut conn, &[item_id])
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let dl = dl_map.get(&item_id).map(|d| RawItemDownloadInfo {
        status: d.status.clone(),
        progress: d.progress,
    });

    // Evaluate filter rules for this item
    let filter_passed = {
        use crate::models::{FilterRule, FilterTargetType};
        use crate::schema::filter_rules;
        use crate::services::filter::FilterEngine;

        let global_rules: Vec<FilterRule> = filter_rules::table
            .filter(filter_rules::target_type.eq(FilterTargetType::Global))
            .filter(filter_rules::target_id.is_null())
            .order(filter_rules::rule_order.asc())
            .load(&mut conn)
            .unwrap_or_default();

        let sub_rules: Vec<FilterRule> = filter_rules::table
            .filter(filter_rules::target_type.eq(FilterTargetType::Fetcher))
            .filter(filter_rules::target_id.eq(item.subscription_id))
            .order(filter_rules::rule_order.asc())
            .load(&mut conn)
            .unwrap_or_default();

        let all_rules: Vec<FilterRule> = global_rules.into_iter().chain(sub_rules).collect();

        if all_rules.is_empty() {
            None
        } else {
            let engine = FilterEngine::with_priority_sorted(all_rules);
            Some(engine.should_include(&item.title))
        }
    };

    Ok(Json(RawItemResponse::from_item(item, dl, filter_passed)))
}

/// POST /raw-items/:item_id/reparse - 重新解析單一項目
pub async fn reparse_item(
    State(state): State<AppState>,
    Path(item_id): Path<i32>,
) -> Result<Json<ReparseResponse>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let item = raw_anime_items::table
        .filter(raw_anime_items::item_id.eq(item_id))
        .first::<RawAnimeItem>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Item not found".to_string()))?;

    match TitleParserService::parse_title(&mut conn, &item.title) {
        Ok(Some(parsed)) => {
            match super::fetcher_results::process_parsed_result(&mut conn, &item, &parsed) {
                Ok(link_id) => {
                    TitleParserService::update_raw_item_status(
                        &mut conn,
                        item_id,
                        ParseStatus::Parsed,
                        Some(parsed.parser_id),
                        None,
                    )
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

                    // 觸發 dispatch 下載
                    let dispatch_service = state.dispatch_service.clone();
                    tokio::spawn(async move {
                        if let Err(e) = dispatch_service.dispatch_new_links(vec![link_id]).await {
                            tracing::warn!("reparse dispatch 失敗: {}", e);
                        }
                    });

                    Ok(Json(ReparseResponse {
                        success: true,
                        items_processed: 1,
                        items_parsed: 1,
                        message: format!("Parsed: {} EP{}", parsed.anime_title, parsed.episode_no),
                    }))
                }
                Err(e) => {
                    TitleParserService::update_raw_item_status(
                        &mut conn,
                        item_id,
                        ParseStatus::Failed,
                        Some(parsed.parser_id),
                        Some(&e),
                    )
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

                    Err((StatusCode::INTERNAL_SERVER_ERROR, e))
                }
            }
        }
        Ok(None) => {
            TitleParserService::update_raw_item_status(
                &mut conn,
                item_id,
                ParseStatus::NoMatch,
                None,
                Some("No matching parser"),
            )
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

            Ok(Json(ReparseResponse {
                success: false,
                items_processed: 1,
                items_parsed: 0,
                message: "No matching parser found".to_string(),
            }))
        }
        Err(e) => {
            TitleParserService::update_raw_item_status(
                &mut conn,
                item_id,
                ParseStatus::Failed,
                None,
                Some(&e),
            )
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

            Err((StatusCode::INTERNAL_SERVER_ERROR, e))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CountRawItemsQuery {
    pub subscription_id: i32,
    pub status: Option<String>,
}

/// GET /raw-items/count - count raw items by subscription and status
pub async fn count_raw_items(
    State(state): State<AppState>,
    Query(query): Query<CountRawItemsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut q = raw_anime_items::table
        .filter(raw_anime_items::subscription_id.eq(query.subscription_id))
        .into_boxed();

    if let Some(status) = &query.status {
        let statuses: Vec<&str> = status.split(',').collect();
        q = q.filter(raw_anime_items::status.eq_any(statuses));
    }

    let count: i64 = q
        .count()
        .get_result(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "count": count })))
}

/// POST /raw-items/:item_id/skip - 標記為跳過
pub async fn skip_item(
    State(state): State<AppState>,
    Path(item_id): Path<i32>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    TitleParserService::update_raw_item_status(
        &mut conn,
        item_id,
        ParseStatus::Skipped,
        None,
        Some("Manually skipped"),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(StatusCode::NO_CONTENT)
}
