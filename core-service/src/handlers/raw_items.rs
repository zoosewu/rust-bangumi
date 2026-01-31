//! 原始資料管理 API

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::state::AppState;
use crate::models::RawAnimeItem;
use crate::schema::raw_anime_items;
use crate::services::TitleParserService;
use crate::services::title_parser::ParseStatus;

// ============ DTOs ============

#[derive(Debug, Deserialize)]
pub struct ListRawItemsQuery {
    pub status: Option<String>,
    pub subscription_id: Option<i32>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
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
}

impl From<RawAnimeItem> for RawItemResponse {
    fn from(item: RawAnimeItem) -> Self {
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
        }
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
    let mut conn = state.db.get()
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

    Ok(Json(items.into_iter().map(RawItemResponse::from).collect()))
}

/// GET /raw-items/:item_id - 取得單一項目
pub async fn get_raw_item(
    State(state): State<AppState>,
    Path(item_id): Path<i32>,
) -> Result<Json<RawItemResponse>, (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let item = raw_anime_items::table
        .filter(raw_anime_items::item_id.eq(item_id))
        .first::<RawAnimeItem>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Item not found".to_string()))?;

    Ok(Json(RawItemResponse::from(item)))
}

/// POST /raw-items/:item_id/reparse - 重新解析單一項目
pub async fn reparse_item(
    State(state): State<AppState>,
    Path(item_id): Path<i32>,
) -> Result<Json<ReparseResponse>, (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let item = raw_anime_items::table
        .filter(raw_anime_items::item_id.eq(item_id))
        .first::<RawAnimeItem>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Item not found".to_string()))?;

    match TitleParserService::parse_title(&mut conn, &item.title) {
        Ok(Some(parsed)) => {
            TitleParserService::update_raw_item_status(
                &mut conn,
                item_id,
                ParseStatus::Parsed,
                Some(parsed.parser_id),
                None,
            ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

            Ok(Json(ReparseResponse {
                success: true,
                items_processed: 1,
                items_parsed: 1,
                message: format!("Parsed: {} EP{}", parsed.anime_title, parsed.episode_no),
            }))
        }
        Ok(None) => {
            TitleParserService::update_raw_item_status(
                &mut conn,
                item_id,
                ParseStatus::NoMatch,
                None,
                Some("No matching parser"),
            ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

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
            ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

            Err((StatusCode::INTERNAL_SERVER_ERROR, e))
        }
    }
}

/// POST /raw-items/:item_id/skip - 標記為跳過
pub async fn skip_item(
    State(state): State<AppState>,
    Path(item_id): Path<i32>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut conn = state.db.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    TitleParserService::update_raw_item_status(
        &mut conn,
        item_id,
        ParseStatus::Skipped,
        None,
        Some("Manually skipped"),
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(StatusCode::NO_CONTENT)
}
