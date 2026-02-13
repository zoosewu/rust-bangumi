//! 解析器管理 API

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use diesel::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::models::{FilterTargetType, NewTitleParser, ParserSourceType, RawAnimeItem, TitleParser};
use crate::schema::{anime_links, raw_anime_items, title_parsers};
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
    pub created_from_type: Option<String>,
    pub created_from_id: Option<i32>,
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
    pub created_from_type: Option<String>,
    pub created_from_id: Option<i32>,
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
            created_from_type: p.created_from_type.map(|t| t.to_string()),
            created_from_id: p.created_from_id,
            created_at: p.created_at.to_string(),
            updated_at: p.updated_at.to_string(),
        }
    }
}

// ============ Handlers ============

#[derive(Debug, Deserialize)]
pub struct ListParsersQuery {
    pub created_from_type: Option<String>,
    pub created_from_id: Option<i32>,
}

/// GET /parsers - 列出所有解析器（可選 created_from 篩選）
pub async fn list_parsers(
    State(state): State<AppState>,
    Query(query): Query<ListParsersQuery>,
) -> Result<Json<Vec<ParserResponse>>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut q = title_parsers::table
        .order(title_parsers::priority.desc())
        .into_boxed();

    if let Some(ref type_str) = query.created_from_type {
        let target_type: FilterTargetType = type_str
            .parse()
            .map_err(|e: String| (StatusCode::BAD_REQUEST, e))?;
        q = q.filter(title_parsers::created_from_type.eq(target_type));
    }

    if let Some(id) = query.created_from_id {
        q = q.filter(title_parsers::created_from_id.eq(id));
    }

    let parsers = q
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
        created_from_type: req
            .created_from_type
            .as_ref()
            .map(|s| s.parse::<FilterTargetType>())
            .transpose()
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?,
        created_from_id: req.created_from_id,
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

/// PUT /parsers/:parser_id - 更新解析器
pub async fn update_parser(
    State(state): State<AppState>,
    Path(parser_id): Path<i32>,
    Json(req): Json<CreateParserRequest>,
) -> Result<Json<ParserResponse>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 確認 parser 存在
    title_parsers::table
        .filter(title_parsers::parser_id.eq(parser_id))
        .first::<TitleParser>(&mut conn)
        .map_err(|_| (StatusCode::NOT_FOUND, "Parser not found".to_string()))?;

    let now = Utc::now().naive_utc();

    let updated_parser = diesel::update(
        title_parsers::table.filter(title_parsers::parser_id.eq(parser_id)),
    )
    .set((
        title_parsers::name.eq(&req.name),
        title_parsers::description.eq(&req.description),
        title_parsers::priority.eq(req.priority),
        title_parsers::is_enabled.eq(req.is_enabled.unwrap_or(true)),
        title_parsers::condition_regex.eq(&req.condition_regex),
        title_parsers::parse_regex.eq(&req.parse_regex),
        title_parsers::anime_title_source.eq(parse_source_type(&req.anime_title_source)?),
        title_parsers::anime_title_value.eq(&req.anime_title_value),
        title_parsers::episode_no_source.eq(parse_source_type(&req.episode_no_source)?),
        title_parsers::episode_no_value.eq(&req.episode_no_value),
        title_parsers::series_no_source.eq(req
            .series_no_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?),
        title_parsers::series_no_value.eq(&req.series_no_value),
        title_parsers::subtitle_group_source.eq(req
            .subtitle_group_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?),
        title_parsers::subtitle_group_value.eq(&req.subtitle_group_value),
        title_parsers::resolution_source.eq(req
            .resolution_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?),
        title_parsers::resolution_value.eq(&req.resolution_value),
        title_parsers::season_source.eq(req
            .season_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?),
        title_parsers::season_value.eq(&req.season_value),
        title_parsers::year_source.eq(req
            .year_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?),
        title_parsers::year_value.eq(&req.year_value),
        title_parsers::updated_at.eq(now),
        title_parsers::created_from_type.eq(req
            .created_from_type
            .as_ref()
            .map(|s| s.parse::<FilterTargetType>())
            .transpose()
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?),
        title_parsers::created_from_id.eq(req.created_from_id),
    ))
    .get_result::<TitleParser>(&mut conn)
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 非同步重新解析：被此 parser 解析過的項目 + no_match/failed 項目
    let db_pool = state.db.clone();
    let dispatch_service = state.dispatch_service.clone();
    tokio::spawn(async move {
        // 收集被此 parser 解析過的項目 ID
        let affected_ids = {
            let mut conn = match db_pool.get() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("update_parser reparse: 無法取得 DB 連線: {}", e);
                    return;
                }
            };
            match raw_anime_items::table
                .filter(
                    raw_anime_items::parser_id
                        .eq(parser_id)
                        .and(raw_anime_items::status.eq("parsed")),
                )
                .select(raw_anime_items::item_id)
                .load::<i32>(&mut conn)
            {
                Ok(ids) => ids,
                Err(e) => {
                    tracing::error!("update_parser reparse: 查詢受影響項目失敗: {}", e);
                    return;
                }
            }
        };

        // 也收集 no_match/failed 項目 ID
        let failed_ids = {
            let mut conn = match db_pool.get() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("update_parser reparse: 無法取得 DB 連線: {}", e);
                    return;
                }
            };
            match raw_anime_items::table
                .filter(
                    raw_anime_items::status
                        .eq("no_match")
                        .or(raw_anime_items::status.eq("failed")),
                )
                .select(raw_anime_items::item_id)
                .load::<i32>(&mut conn)
            {
                Ok(ids) => ids,
                Err(e) => {
                    tracing::error!("update_parser reparse: 查詢失敗項目失敗: {}", e);
                    return;
                }
            }
        };

        // 合併去重
        let mut all_ids = affected_ids;
        for id in failed_ids {
            if !all_ids.contains(&id) {
                all_ids.push(id);
            }
        }

        if !all_ids.is_empty() {
            reparse_affected_items(db_pool, dispatch_service, &all_ids).await;
        }
    });

    Ok(Json(ParserResponse::from(updated_parser)))
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

    // 先查出被此 parser 解析過的項目 ID
    let affected_ids = raw_anime_items::table
        .filter(
            raw_anime_items::parser_id
                .eq(parser_id)
                .and(raw_anime_items::status.eq("parsed")),
        )
        .select(raw_anime_items::item_id)
        .load::<i32>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let deleted =
        diesel::delete(title_parsers::table.filter(title_parsers::parser_id.eq(parser_id)))
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted == 0 {
        return Err((StatusCode::NOT_FOUND, "Parser not found".to_string()));
    }

    // 非同步重新解析受影響的項目
    if !affected_ids.is_empty() {
        let db_pool = state.db.clone();
        let dispatch_service = state.dispatch_service.clone();
        tokio::spawn(async move {
            reparse_affected_items(db_pool, dispatch_service, &affected_ids).await;
        });
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

/// 重新解析指定的 raw_anime_items（用於 update/delete parser 時）
///
/// 1. 載入指定項目
/// 2. 刪除這些項目的 anime_links（downloads 會因 ON DELETE CASCADE 自動清除）
/// 3. 重設狀態為 pending，清除 parser_id
/// 4. 對每筆項目重新解析
async fn reparse_affected_items(
    db: crate::db::DbPool,
    dispatch_service: std::sync::Arc<crate::services::DownloadDispatchService>,
    item_ids: &[i32],
) {
    if item_ids.is_empty() {
        return;
    }

    let mut conn = match db.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("reparse_affected_items: 無法取得 DB 連線: {}", e);
            return;
        }
    };

    // 刪除這些項目的 anime_links（downloads 因 CASCADE 自動刪除）
    if let Err(e) = diesel::delete(
        anime_links::table.filter(anime_links::raw_item_id.eq_any(item_ids)),
    )
    .execute(&mut conn)
    {
        tracing::error!("reparse_affected_items: 刪除 anime_links 失敗: {}", e);
        return;
    }

    // 重設狀態為 pending，清除 parser_id
    if let Err(e) = diesel::update(
        raw_anime_items::table.filter(raw_anime_items::item_id.eq_any(item_ids)),
    )
    .set((
        raw_anime_items::status.eq("pending"),
        raw_anime_items::parser_id.eq(None::<i32>),
        raw_anime_items::error_message.eq(None::<String>),
    ))
    .execute(&mut conn)
    {
        tracing::error!("reparse_affected_items: 重設項目狀態失敗: {}", e);
        return;
    }

    // 載入項目
    let items: Vec<RawAnimeItem> = match raw_anime_items::table
        .filter(raw_anime_items::item_id.eq_any(item_ids))
        .load::<RawAnimeItem>(&mut conn)
    {
        Ok(items) => items,
        Err(e) => {
            tracing::error!("reparse_affected_items: 載入項目失敗: {}", e);
            return;
        }
    };

    tracing::info!(
        "reparse_affected_items: 開始重新解析 {} 筆項目",
        items.len()
    );

    let mut parsed_count = 0;
    let mut new_link_ids: Vec<i32> = Vec::new();

    for item in &items {
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
                            "reparse_affected: {} -> {} EP{}",
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
                        tracing::warn!("reparse_affected: 建立記錄失敗 {}: {}", item.title, e);
                    }
                }
            }
            Ok(None) => {
                TitleParserService::update_raw_item_status(
                    &mut conn,
                    item.item_id,
                    ParseStatus::NoMatch,
                    None,
                    None,
                )
                .ok();
            }
            Err(e) => {
                TitleParserService::update_raw_item_status(
                    &mut conn,
                    item.item_id,
                    ParseStatus::Failed,
                    None,
                    Some(&e),
                )
                .ok();
                tracing::warn!("reparse_affected: 解析錯誤 {}: {}", item.title, e);
            }
        }
    }

    tracing::info!(
        "reparse_affected_items: 完成，成功解析 {}/{} 筆",
        parsed_count,
        items.len()
    );

    // 觸發 dispatch 下載
    if !new_link_ids.is_empty() {
        if let Err(e) = dispatch_service.dispatch_new_links(new_link_ids).await {
            tracing::warn!("reparse_affected_items: dispatch 失敗: {}", e);
        }
    }
}

// ============ Preview DTOs ============

#[derive(Debug, Deserialize)]
pub struct ParserPreviewRequest {
    pub target_type: Option<String>,
    pub target_id: Option<i32>,
    pub condition_regex: String,
    pub parse_regex: String,
    pub priority: i32,
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
    pub exclude_parser_id: Option<i32>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ParsedFields {
    pub anime_title: String,
    pub episode_no: i32,
    pub series_no: i32,
    pub subtitle_group: Option<String>,
    pub resolution: Option<String>,
    pub season: Option<String>,
    pub year: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ParserPreviewResult {
    pub title: String,
    pub before_matched_by: Option<String>,
    pub after_matched_by: Option<String>,
    pub is_newly_matched: bool,
    pub is_override: bool,
    pub parse_result: Option<ParsedFields>,
}

#[derive(Debug, Serialize)]
pub struct ParserPreviewResponse {
    pub condition_regex_valid: bool,
    pub parse_regex_valid: bool,
    pub regex_error: Option<String>,
    pub results: Vec<ParserPreviewResult>,
}

/// POST /parsers/preview
pub async fn preview_parser(
    State(state): State<AppState>,
    Json(req): Json<ParserPreviewRequest>,
) -> Result<Json<ParserPreviewResponse>, (StatusCode, String)> {
    // Validate regexes
    if let Err(e) = Regex::new(&req.condition_regex) {
        return Ok(Json(ParserPreviewResponse {
            condition_regex_valid: false,
            parse_regex_valid: true,
            regex_error: Some(format!("condition_regex: {}", e)),
            results: vec![],
        }));
    }
    if let Err(e) = Regex::new(&req.parse_regex) {
        return Ok(Json(ParserPreviewResponse {
            condition_regex_valid: true,
            parse_regex_valid: false,
            regex_error: Some(format!("parse_regex: {}", e)),
            results: vec![],
        }));
    }

    let limit = req.limit.unwrap_or(50).min(200);

    // Load raw items scoped by target_type/target_id
    let items = {
        let mut conn = state
            .db
            .get()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        find_scoped_raw_items(&mut conn, req.target_type.as_deref(), req.target_id, limit)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    };

    // Load existing enabled parsers
    let all_parsers = state
        .repos
        .title_parser
        .find_enabled_sorted_by_priority()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Build "before" parsers (exclude current)
    let before_parsers: Vec<&TitleParser> = all_parsers
        .iter()
        .filter(|p| Some(p.parser_id) != req.exclude_parser_id)
        .collect();

    // Build a temporary TitleParser for the "current" parser being previewed
    let now = Utc::now().naive_utc();
    let current_parser = TitleParser {
        parser_id: -1, // sentinel
        name: "(preview)".to_string(),
        description: None,
        priority: req.priority,
        is_enabled: true,
        condition_regex: req.condition_regex.clone(),
        parse_regex: req.parse_regex.clone(),
        anime_title_source: parse_source_type(&req.anime_title_source)?,
        anime_title_value: req.anime_title_value.clone(),
        episode_no_source: parse_source_type(&req.episode_no_source)?,
        episode_no_value: req.episode_no_value.clone(),
        series_no_source: req
            .series_no_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        series_no_value: req.series_no_value.clone(),
        subtitle_group_source: req
            .subtitle_group_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        subtitle_group_value: req.subtitle_group_value.clone(),
        resolution_source: req
            .resolution_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        resolution_value: req.resolution_value.clone(),
        season_source: req
            .season_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        season_value: req.season_value.clone(),
        year_source: req
            .year_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        year_value: req.year_value.clone(),
        created_at: now,
        updated_at: now,
        created_from_type: None,
        created_from_id: None,
    };

    // Build "after" parsers list: before_parsers + current_parser, sorted by priority desc
    let mut after_parsers: Vec<&TitleParser> = before_parsers.clone();
    after_parsers.push(&current_parser);
    after_parsers.sort_by(|a, b| b.priority.cmp(&a.priority));

    // Process each item
    let mut results = Vec::new();
    for item in &items {
        let before_match = find_matching_parser(&before_parsers, &item.title);
        let after_match = find_matching_parser(&after_parsers, &item.title);

        let before_name = before_match.map(|p| p.name.clone());
        let after_name = after_match.map(|p| {
            if p.parser_id == -1 {
                "(current)".to_string()
            } else {
                p.name.clone()
            }
        });

        let is_current_after = after_match.map(|p| p.parser_id == -1).unwrap_or(false);
        let is_newly_matched = before_match.is_none() && is_current_after;
        let is_override =
            before_match.is_some() && is_current_after && before_match.map(|p| p.parser_id) != Some(-1);

        // Parse result only if current parser matched in "after"
        let parse_result = if is_current_after {
            TitleParserService::try_parser(&current_parser, &item.title)
                .ok()
                .flatten()
                .map(|r| ParsedFields {
                    anime_title: r.anime_title,
                    episode_no: r.episode_no,
                    series_no: r.series_no,
                    subtitle_group: r.subtitle_group,
                    resolution: r.resolution,
                    season: r.season,
                    year: r.year,
                })
        } else {
            None
        };

        results.push(ParserPreviewResult {
            title: item.title.clone(),
            before_matched_by: before_name,
            after_matched_by: after_name,
            is_newly_matched,
            is_override,
            parse_result,
        });
    }

    Ok(Json(ParserPreviewResponse {
        condition_regex_valid: true,
        parse_regex_valid: true,
        regex_error: None,
        results,
    }))
}

/// Find the first parser that matches a title (parsers must be pre-sorted by priority desc)
fn find_matching_parser<'a>(parsers: &[&'a TitleParser], title: &str) -> Option<&'a TitleParser> {
    for parser in parsers {
        if let Ok(Some(_)) = TitleParserService::try_parser(parser, title) {
            return Some(parser);
        }
    }
    None
}

/// Load raw_anime_items scoped by target_type/target_id.
///
/// - global / None: all items
/// - anime_series: items from subscriptions that feed this series (via anime_links)
/// - anime: items from subscriptions that feed any series of this anime
/// - subtitle_group: items from subscriptions that produced links for this group
/// - fetcher: items from this subscription directly
fn find_scoped_raw_items(
    conn: &mut diesel::PgConnection,
    target_type: Option<&str>,
    target_id: Option<i32>,
    limit: i64,
) -> Result<Vec<RawAnimeItem>, String> {
    use crate::schema::{anime_links, anime_series};

    let target_type = match target_type {
        Some(t) => t,
        None => {
            return raw_anime_items::table
                .order(raw_anime_items::item_id.desc())
                .limit(limit)
                .load::<RawAnimeItem>(conn)
                .map_err(|e| e.to_string());
        }
    };

    match target_type {
        "global" => {
            raw_anime_items::table
                .order(raw_anime_items::item_id.desc())
                .limit(limit)
                .load::<RawAnimeItem>(conn)
                .map_err(|e| e.to_string())
        }
        "anime_series" => {
            let sid = target_id.ok_or("anime_series requires target_id")?;
            let sub_ids: Vec<i32> = anime_links::table
                .inner_join(
                    raw_anime_items::table
                        .on(anime_links::raw_item_id.eq(raw_anime_items::item_id.nullable())),
                )
                .filter(anime_links::series_id.eq(sid))
                .select(raw_anime_items::subscription_id)
                .distinct()
                .load(conn)
                .map_err(|e| e.to_string())?;

            if sub_ids.is_empty() {
                return Ok(vec![]);
            }

            raw_anime_items::table
                .filter(raw_anime_items::subscription_id.eq_any(&sub_ids))
                .order(raw_anime_items::item_id.desc())
                .limit(limit)
                .load::<RawAnimeItem>(conn)
                .map_err(|e| e.to_string())
        }
        "anime" => {
            let aid = target_id.ok_or("anime requires target_id")?;
            let series_ids: Vec<i32> = anime_series::table
                .filter(anime_series::anime_id.eq(aid))
                .select(anime_series::series_id)
                .load(conn)
                .map_err(|e| e.to_string())?;

            let sub_ids: Vec<i32> = anime_links::table
                .inner_join(
                    raw_anime_items::table
                        .on(anime_links::raw_item_id.eq(raw_anime_items::item_id.nullable())),
                )
                .filter(anime_links::series_id.eq_any(&series_ids))
                .select(raw_anime_items::subscription_id)
                .distinct()
                .load(conn)
                .map_err(|e| e.to_string())?;

            if sub_ids.is_empty() {
                return Ok(vec![]);
            }

            raw_anime_items::table
                .filter(raw_anime_items::subscription_id.eq_any(&sub_ids))
                .order(raw_anime_items::item_id.desc())
                .limit(limit)
                .load::<RawAnimeItem>(conn)
                .map_err(|e| e.to_string())
        }
        "subtitle_group" => {
            let gid = target_id.ok_or("subtitle_group requires target_id")?;
            let sub_ids: Vec<i32> = anime_links::table
                .inner_join(
                    raw_anime_items::table
                        .on(anime_links::raw_item_id.eq(raw_anime_items::item_id.nullable())),
                )
                .filter(anime_links::group_id.eq(gid))
                .select(raw_anime_items::subscription_id)
                .distinct()
                .load(conn)
                .map_err(|e| e.to_string())?;

            if sub_ids.is_empty() {
                return Ok(vec![]);
            }

            raw_anime_items::table
                .filter(raw_anime_items::subscription_id.eq_any(&sub_ids))
                .order(raw_anime_items::item_id.desc())
                .limit(limit)
                .load::<RawAnimeItem>(conn)
                .map_err(|e| e.to_string())
        }
        "fetcher" | "subscription" => {
            let fid = target_id.ok_or("fetcher/subscription requires target_id")?;
            raw_anime_items::table
                .filter(raw_anime_items::subscription_id.eq(fid))
                .order(raw_anime_items::item_id.desc())
                .limit(limit)
                .load::<RawAnimeItem>(conn)
                .map_err(|e| e.to_string())
        }
        other => Err(format!("Unknown target_type: {}", other)),
    }
}
