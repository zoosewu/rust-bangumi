//! 解析器管理 API

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::{NewTitleParser, ParserSourceType, RawAnimeItem, TitleParser};
use crate::schema::{raw_anime_items, title_parsers};
use crate::services::title_parser::ParseStatus;
use crate::services::TitleParserService;
use crate::state::AppState;

// ============ DTOs ============

#[derive(Debug, Deserialize)]
pub struct CreateParserRequest {
    pub name: String,
    pub description: Option<String>,
    pub priority: i32,
    pub is_enabled: Option<bool>,
    pub condition_regex: String,
    pub parse_regex: String,
    pub anime_title_source: String, // "regex" or "static"
    pub anime_title_value: String,
    pub episode_no_source: String,
    pub episode_no_value: String,
    pub series_no_source: Option<String>,
    pub series_no_value: Option<String>,
    pub subtitle_group_source: Option<String>,
    pub subtitle_group_value: Option<String>,
    pub resolution_source: Option<String>,
    pub resolution_value: Option<String>,
    pub season_source: Option<String>,
    pub season_value: Option<String>,
    pub year_source: Option<String>,
    pub year_value: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ParserResponse {
    pub parser_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub priority: i32,
    pub is_enabled: bool,
    pub condition_regex: String,
    pub parse_regex: String,
    pub anime_title_source: String,
    pub anime_title_value: String,
    pub episode_no_source: String,
    pub episode_no_value: String,
    pub series_no_source: Option<String>,
    pub series_no_value: Option<String>,
    pub subtitle_group_source: Option<String>,
    pub subtitle_group_value: Option<String>,
    pub resolution_source: Option<String>,
    pub resolution_value: Option<String>,
    pub season_source: Option<String>,
    pub season_value: Option<String>,
    pub year_source: Option<String>,
    pub year_value: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<TitleParser> for ParserResponse {
    fn from(p: TitleParser) -> Self {
        Self {
            parser_id: p.parser_id,
            name: p.name,
            description: p.description,
            priority: p.priority,
            is_enabled: p.is_enabled,
            condition_regex: p.condition_regex,
            parse_regex: p.parse_regex,
            anime_title_source: p.anime_title_source.to_string(),
            anime_title_value: p.anime_title_value,
            episode_no_source: p.episode_no_source.to_string(),
            episode_no_value: p.episode_no_value,
            series_no_source: p.series_no_source.map(|s| s.to_string()),
            series_no_value: p.series_no_value,
            subtitle_group_source: p.subtitle_group_source.map(|s| s.to_string()),
            subtitle_group_value: p.subtitle_group_value,
            resolution_source: p.resolution_source.map(|s| s.to_string()),
            resolution_value: p.resolution_value,
            season_source: p.season_source.map(|s| s.to_string()),
            season_value: p.season_value,
            year_source: p.year_source.map(|s| s.to_string()),
            year_value: p.year_value,
            created_at: p.created_at.to_string(),
            updated_at: p.updated_at.to_string(),
        }
    }
}

// ============ Handlers ============

/// GET /parsers - 列出所有解析器
pub async fn list_parsers(
    State(state): State<AppState>,
) -> Result<Json<Vec<ParserResponse>>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let parsers = title_parsers::table
        .order(title_parsers::priority.desc())
        .load::<TitleParser>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        parsers.into_iter().map(ParserResponse::from).collect(),
    ))
}

/// GET /parsers/:parser_id - 取得單一解析器
pub async fn get_parser(
    State(state): State<AppState>,
    Path(parser_id): Path<i32>,
) -> Result<Json<ParserResponse>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let parser = title_parsers::table
        .filter(title_parsers::parser_id.eq(parser_id))
        .first::<TitleParser>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Parser not found".to_string()))?;

    Ok(Json(ParserResponse::from(parser)))
}

/// POST /parsers - 新增解析器
pub async fn create_parser(
    State(state): State<AppState>,
    Json(req): Json<CreateParserRequest>,
) -> Result<(StatusCode, Json<ParserResponse>), (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let now = Utc::now().naive_utc();

    let new_parser = NewTitleParser {
        name: req.name,
        description: req.description,
        priority: req.priority,
        is_enabled: req.is_enabled.unwrap_or(true),
        condition_regex: req.condition_regex,
        parse_regex: req.parse_regex,
        anime_title_source: parse_source_type(&req.anime_title_source)?,
        anime_title_value: req.anime_title_value,
        episode_no_source: parse_source_type(&req.episode_no_source)?,
        episode_no_value: req.episode_no_value,
        series_no_source: req
            .series_no_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        series_no_value: req.series_no_value,
        subtitle_group_source: req
            .subtitle_group_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        subtitle_group_value: req.subtitle_group_value,
        resolution_source: req
            .resolution_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        resolution_value: req.resolution_value,
        season_source: req
            .season_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        season_value: req.season_value,
        year_source: req
            .year_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        year_value: req.year_value,
        created_at: now,
        updated_at: now,
    };

    let parser = diesel::insert_into(title_parsers::table)
        .values(&new_parser)
        .get_result::<TitleParser>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 非同步重新解析所有失敗的 raw_anime_items
    let db_pool = state.db.clone();
    let dispatch_service = state.dispatch_service.clone();
    tokio::spawn(async move {
        reparse_failed_items(db_pool, dispatch_service).await;
    });

    Ok((StatusCode::CREATED, Json(ParserResponse::from(parser))))
}

/// DELETE /parsers/:parser_id - 刪除解析器
pub async fn delete_parser(
    State(state): State<AppState>,
    Path(parser_id): Path<i32>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let deleted =
        diesel::delete(title_parsers::table.filter(title_parsers::parser_id.eq(parser_id)))
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted == 0 {
        return Err((StatusCode::NOT_FOUND, "Parser not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

fn parse_source_type(s: &str) -> Result<ParserSourceType, (StatusCode, String)> {
    match s {
        "regex" => Ok(ParserSourceType::Regex),
        "static" => Ok(ParserSourceType::Static),
        _ => Err((
            StatusCode::BAD_REQUEST,
            format!("Invalid source type: {}", s),
        )),
    }
}

/// 重新解析所有 no_match / failed 的 raw_anime_items
async fn reparse_failed_items(
    db: crate::db::DbPool,
    dispatch_service: std::sync::Arc<crate::services::DownloadDispatchService>,
) {
    let mut conn = match db.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("reparse_failed_items: 無法取得 DB 連線: {}", e);
            return;
        }
    };

    let failed_items: Vec<RawAnimeItem> = match raw_anime_items::table
        .filter(
            raw_anime_items::status
                .eq("no_match")
                .or(raw_anime_items::status.eq("failed")),
        )
        .load::<RawAnimeItem>(&mut conn)
    {
        Ok(items) => items,
        Err(e) => {
            tracing::error!("reparse_failed_items: 查詢失敗項目失敗: {}", e);
            return;
        }
    };

    if failed_items.is_empty() {
        return;
    }

    tracing::info!(
        "reparse_failed_items: 開始重新解析 {} 筆失敗項目",
        failed_items.len()
    );

    let mut parsed_count = 0;
    let mut new_link_ids: Vec<i32> = Vec::new();

    for item in &failed_items {
        match TitleParserService::parse_title(&mut conn, &item.title) {
            Ok(Some(parsed)) => {
                match super::fetcher_results::process_parsed_result(&mut conn, item, &parsed) {
                    Ok(link_id) => {
                        new_link_ids.push(link_id);
                        TitleParserService::update_raw_item_status(
                            &mut conn,
                            item.item_id,
                            ParseStatus::Parsed,
                            Some(parsed.parser_id),
                            None,
                        )
                        .ok();
                        parsed_count += 1;
                        tracing::info!(
                            "reparse: {} -> {} EP{}",
                            item.title,
                            parsed.anime_title,
                            parsed.episode_no
                        );
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
                        tracing::warn!("reparse: 建立記錄失敗 {}: {}", item.title, e);
                    }
                }
            }
            Ok(None) => {} // 仍然無匹配，保持原狀
            Err(e) => {
                tracing::warn!("reparse: 解析錯誤 {}: {}", item.title, e);
            }
        }
    }

    tracing::info!(
        "reparse_failed_items: 完成，成功解析 {}/{} 筆",
        parsed_count,
        failed_items.len()
    );

    // 觸發 dispatch 下載
    if !new_link_ids.is_empty() {
        if let Err(e) = dispatch_service.dispatch_new_links(new_link_ids).await {
            tracing::warn!("reparse_failed_items: dispatch 失敗: {}", e);
        }
    }
}
