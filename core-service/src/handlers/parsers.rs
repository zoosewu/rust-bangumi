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
use crate::schema::{raw_anime_items, title_parsers};
use crate::services::ReparseStats;
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
    pub episode_end_source: Option<String>,
    pub episode_end_value: Option<String>,
    pub created_from_type: Option<String>,
    pub created_from_id: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ParserWithReparseResponse {
    #[serde(flatten)]
    pub parser: ParserResponse,
    pub reparse: ReparseStats,
}

#[derive(Debug, Serialize)]
pub struct DeleteWithReparseResponse {
    pub reparse: ReparseStats,
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
    pub episode_end_source: Option<String>,
    pub episode_end_value: Option<String>,
    pub created_from_type: Option<String>,
    pub created_from_id: Option<i32>,
    pub created_from_name: Option<String>,
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
            episode_end_source: p.episode_end_source.map(|s| s.to_string()),
            episode_end_value: p.episode_end_value,
            created_from_type: p.created_from_type.map(|t| t.to_string()),
            created_from_id: p.created_from_id,
            created_from_name: None,
            created_at: crate::serde_utils::format_utc(&p.created_at),
            updated_at: crate::serde_utils::format_utc(&p.updated_at),
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

    let (anime_names, series_names, group_names, sub_names) =
        resolve_parser_names(&mut conn, &parsers);

    let responses: Vec<ParserResponse> = parsers
        .into_iter()
        .map(|p| {
            let type_ref = p.created_from_type.as_ref();
            let id = p.created_from_id;
            let name = match (type_ref, id) {
                (Some(FilterTargetType::Global), _) => Some("Global".to_string()),
                (Some(FilterTargetType::AnimeWork), Some(id)) => anime_names.get(&id).cloned(),
                (Some(FilterTargetType::Anime), Some(id)) => series_names.get(&id).cloned(),
                (Some(FilterTargetType::SubtitleGroup), Some(id)) => group_names.get(&id).cloned(),
                (Some(FilterTargetType::Subscription), Some(id))
                | (Some(FilterTargetType::Fetcher), Some(id)) => sub_names.get(&id).cloned(),
                _ => None,
            };
            let mut resp = ParserResponse::from(p);
            resp.created_from_name = name;
            resp
        })
        .collect();

    Ok(Json(responses))
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
) -> Result<(StatusCode, Json<ParserWithReparseResponse>), (StatusCode, String)> {
    let parser = {
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
            episode_end_source: req
                .episode_end_source
                .as_ref()
                .map(|s| parse_source_type(s))
                .transpose()?,
            episode_end_value: req.episode_end_value,
            pending_result_id: None,
        };

        diesel::insert_into(title_parsers::table)
            .values(&new_parser)
            .get_result::<TitleParser>(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    }; // conn 在此 drop，釋放連線給 reparse 使用

    // 同步重新解析所有 raw_anime_items
    let stats =
        crate::services::reparse::reparse_all_items(state.db.clone(), state.dispatch_service.clone(), state.sync_service.clone(), state.conflict_detection.clone(), state.cancel_service.clone()).await;

    Ok((
        StatusCode::CREATED,
        Json(ParserWithReparseResponse {
            parser: ParserResponse::from(parser),
            reparse: stats,
        }),
    ))
}

/// PUT /parsers/:parser_id - 更新解析器
pub async fn update_parser(
    State(state): State<AppState>,
    Path(parser_id): Path<i32>,
    Json(req): Json<CreateParserRequest>,
) -> Result<Json<ParserWithReparseResponse>, (StatusCode, String)> {
    let updated_parser = {
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

        diesel::update(
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
            title_parsers::episode_end_source.eq(req
                .episode_end_source
                .as_ref()
                .map(|s| parse_source_type(s))
                .transpose()?),
            title_parsers::episode_end_value.eq(&req.episode_end_value),
            title_parsers::updated_at.eq(now),
        ))
        .get_result::<TitleParser>(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    }; // conn 在此 drop，釋放連線給 reparse 使用

    // 同步重新解析所有 raw_anime_items
    let stats =
        crate::services::reparse::reparse_all_items(state.db.clone(), state.dispatch_service.clone(), state.sync_service.clone(), state.conflict_detection.clone(), state.cancel_service.clone()).await;

    Ok(Json(ParserWithReparseResponse {
        parser: ParserResponse::from(updated_parser),
        reparse: stats,
    }))
}

/// DELETE /parsers/:parser_id - 刪除解析器
pub async fn delete_parser(
    State(state): State<AppState>,
    Path(parser_id): Path<i32>,
) -> Result<Json<DeleteWithReparseResponse>, (StatusCode, String)> {
    {
        let mut conn = state
            .db
            .get()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // 先解除 raw_anime_items 對此 parser 的參照，避免 FK 違反
        diesel::update(raw_anime_items::table.filter(raw_anime_items::parser_id.eq(parser_id)))
            .set(raw_anime_items::parser_id.eq(Option::<i32>::None))
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let deleted =
            diesel::delete(title_parsers::table.filter(title_parsers::parser_id.eq(parser_id)))
                .execute(&mut conn)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if deleted == 0 {
            return Err((StatusCode::NOT_FOUND, "Parser not found".to_string()));
        }
    } // conn 在此 drop

    // 同步重新解析所有 raw_anime_items
    let stats =
        crate::services::reparse::reparse_all_items(state.db.clone(), state.dispatch_service.clone(), state.sync_service.clone(), state.conflict_detection.clone(), state.cancel_service.clone()).await;

    Ok(Json(DeleteWithReparseResponse { reparse: stats }))
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


/// 批次查詢各實體名稱，分別按 created_from_type 分組。
/// 回傳值為 4 個獨立的 HashMap，以各自的主鍵對應名稱。
fn resolve_parser_names(
    conn: &mut diesel::PgConnection,
    parsers: &[TitleParser],
) -> (
    std::collections::HashMap<i32, String>, // work_id -> title (AnimeWork)
    std::collections::HashMap<i32, String>, // anime_id -> "Title S{n}" (Anime)
    std::collections::HashMap<i32, String>, // group_id -> group_name
    std::collections::HashMap<i32, String>, // subscription_id -> name
) {
    use crate::schema::{anime_works, animes, subtitle_groups, subscriptions};

    let mut work_ids: Vec<i32> = vec![];
    let mut anime_ids: Vec<i32> = vec![];
    let mut group_ids: Vec<i32> = vec![];
    let mut sub_ids: Vec<i32> = vec![];

    for p in parsers {
        if let (Some(t), Some(id)) = (&p.created_from_type, p.created_from_id) {
            match t {
                FilterTargetType::AnimeWork => work_ids.push(id),
                FilterTargetType::Anime => anime_ids.push(id),
                FilterTargetType::SubtitleGroup => group_ids.push(id),
                FilterTargetType::Subscription | FilterTargetType::Fetcher => sub_ids.push(id),
                FilterTargetType::Global => {}
            }
        }
    }

    let mut anime_names: std::collections::HashMap<i32, String> = Default::default();
    let mut series_names: std::collections::HashMap<i32, String> = Default::default();
    let mut group_names: std::collections::HashMap<i32, String> = Default::default();
    let mut sub_names: std::collections::HashMap<i32, String> = Default::default();

    if !work_ids.is_empty() {
        match anime_works::table
            .filter(anime_works::work_id.eq_any(&work_ids))
            .select((anime_works::work_id, anime_works::title))
            .load::<(i32, String)>(conn)
        {
            Ok(rows) => {
                for (id, title) in rows {
                    anime_names.insert(id, title);
                }
            }
            Err(e) => tracing::warn!("resolve_parser_names: 查詢 anime_work 名稱失敗: {}", e),
        }
    }

    if !anime_ids.is_empty() {
        match animes::table
            .inner_join(anime_works::table)
            .filter(animes::anime_id.eq_any(&anime_ids))
            .select((animes::anime_id, anime_works::title, animes::series_no))
            .load::<(i32, String, i32)>(conn)
        {
            Ok(rows) => {
                for (id, title, series_no) in rows {
                    series_names.insert(id, format!("{} S{}", title, series_no));
                }
            }
            Err(e) => tracing::warn!("resolve_parser_names: 查詢 anime 名稱失敗: {}", e),
        }
    }

    if !group_ids.is_empty() {
        match subtitle_groups::table
            .filter(subtitle_groups::group_id.eq_any(&group_ids))
            .select((subtitle_groups::group_id, subtitle_groups::group_name))
            .load::<(i32, String)>(conn)
        {
            Ok(rows) => {
                for (id, name) in rows {
                    group_names.insert(id, name);
                }
            }
            Err(e) => tracing::warn!("resolve_parser_names: 查詢 subtitle_group 名稱失敗: {}", e),
        }
    }

    if !sub_ids.is_empty() {
        match subscriptions::table
            .filter(subscriptions::subscription_id.eq_any(&sub_ids))
            .select((subscriptions::subscription_id, subscriptions::name))
            .load::<(i32, Option<String>)>(conn)
        {
            Ok(rows) => {
                for (id, name) in rows {
                    sub_names.insert(id, name.unwrap_or_else(|| format!("#{}", id)));
                }
            }
            Err(e) => tracing::warn!("resolve_parser_names: 查詢 subscription 名稱失敗: {}", e),
        }
    }

    (anime_names, series_names, group_names, sub_names)
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
    pub episode_end_source: Option<String>,
    pub episode_end_value: Option<String>,
    pub exclude_parser_id: Option<i32>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ParsedFields {
    pub anime_title: String,
    pub episode_no: i32,
    pub episode_end: Option<i32>,
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
    pub parse_error: Option<String>,
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
        episode_end_source: req
            .episode_end_source
            .as_ref()
            .map(|s| parse_source_type(s))
            .transpose()?,
        episode_end_value: req.episode_end_value.clone(),
        pending_result_id: None,
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

        let is_current_after = after_match.map(|p| p.parser_id == -1).unwrap_or(false);

        // Try current parser to capture parse errors.
        // Only meaningful when current parser is the winner (is_current_after)
        // OR when it errored (condition_regex matched but extraction failed,
        // causing find_matching_parser to skip it).
        let (parse_result, parse_error) = if is_current_after {
            match TitleParserService::try_parser(&current_parser, &item.title) {
                Ok(Some(r)) => (Some(ParsedFields {
                    anime_title: r.anime_title,
                    episode_no: r.episode_no,
                    episode_end: r.episode_end,
                    series_no: r.series_no,
                    subtitle_group: r.subtitle_group,
                    resolution: r.resolution,
                    season: r.season,
                    year: r.year,
                }), None),
                Ok(None) => (None, None),
                Err(e) => (None, Some(e)),
            }
        } else {
            // Current parser didn't win via find_matching_parser. Check if it
            // was skipped due to error (condition_regex matched but parse failed).
            match TitleParserService::try_parser(&current_parser, &item.title) {
                Err(e) => (None, Some(e)),
                _ => (None, None),
            }
        };

        // Current parser is relevant if it won or if it errored on this title.
        let current_matched = is_current_after || parse_error.is_some();

        let after_name = if current_matched {
            Some("(current)".to_string())
        } else {
            after_match.map(|p| p.name.clone())
        };

        let is_newly_matched = before_match.is_none() && current_matched;
        let is_override =
            before_match.is_some() && current_matched && before_match.map(|p| p.parser_id) != Some(-1);

        results.push(ParserPreviewResult {
            title: item.title.clone(),
            before_matched_by: before_name,
            after_matched_by: after_name,
            is_newly_matched,
            is_override,
            parse_result,
            parse_error,
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
    use crate::schema::{anime_links, animes as anime_table};

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
        "anime_series" | "anime" => {
            let sid = target_id.ok_or("anime requires target_id")?;
            let sub_ids: Vec<i32> = anime_links::table
                .inner_join(
                    raw_anime_items::table
                        .on(anime_links::raw_item_id.eq(raw_anime_items::item_id.nullable())),
                )
                .filter(anime_links::anime_id.eq(sid))
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
        "anime_work" => {
            let wid = target_id.ok_or("anime_work requires target_id")?;
            let anime_ids: Vec<i32> = anime_table::table
                .filter(anime_table::work_id.eq(wid))
                .select(anime_table::anime_id)
                .load(conn)
                .map_err(|e| e.to_string())?;

            let sub_ids: Vec<i32> = anime_links::table
                .inner_join(
                    raw_anime_items::table
                        .on(anime_links::raw_item_id.eq(raw_anime_items::item_id.nullable())),
                )
                .filter(anime_links::anime_id.eq_any(&anime_ids))
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
