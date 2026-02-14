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

#[derive(Debug, Serialize, Default)]
pub struct ReparseStats {
    pub total: usize,
    pub parsed: usize,
    pub failed: usize,
    pub no_match: usize,
    pub resync_triggered: usize,
}

struct UpsertResult {
    link_id: i32,
    is_new: bool,
    metadata_changed: bool,
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
        };

        diesel::insert_into(title_parsers::table)
            .values(&new_parser)
            .get_result::<TitleParser>(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    }; // conn 在此 drop，釋放連線給 reparse 使用

    // 同步重新解析所有 raw_anime_items
    let stats =
        reparse_all_items(state.db.clone(), state.dispatch_service.clone(), state.sync_service.clone()).await;

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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    }; // conn 在此 drop，釋放連線給 reparse 使用

    // 同步重新解析所有 raw_anime_items
    let stats =
        reparse_all_items(state.db.clone(), state.dispatch_service.clone(), state.sync_service.clone()).await;

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
        reparse_all_items(state.db.clone(), state.dispatch_service.clone(), state.sync_service.clone()).await;

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

/// 重新解析所有 raw_anime_items（無論原始狀態）
async fn reparse_all_items(
    db: crate::db::DbPool,
    dispatch_service: std::sync::Arc<crate::services::DownloadDispatchService>,
    sync_service: std::sync::Arc<crate::services::SyncService>,
) -> ReparseStats {
    let all_ids = {
        let mut conn = match db.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("reparse_all_items: 無法取得 DB 連線: {}", e);
                return ReparseStats::default();
            }
        };
        match raw_anime_items::table
            .select(raw_anime_items::item_id)
            .load::<i32>(&mut conn)
        {
            Ok(ids) => ids,
            Err(e) => {
                tracing::error!("reparse_all_items: 查詢項目失敗: {}", e);
                return ReparseStats::default();
            }
        }
    };

    if all_ids.is_empty() {
        tracing::info!("reparse_all_items: 沒有任何項目");
        return ReparseStats::default();
    }

    tracing::info!("reparse_all_items: 開始重新解析全部 {} 筆項目", all_ids.len());
    reparse_affected_items(db, dispatch_service, sync_service, &all_ids).await
}

/// 重新解析指定的 raw_anime_items
///
/// 使用 upsert 邏輯：更新既有的 anime_link 而非刪除重建，
/// 確保 downloads 記錄不會因 CASCADE 被刪除。
///
/// 1. 載入指定項目
/// 2. 對每筆項目重新解析
/// 3. 如果已有 anime_link → 更新欄位（保留 link_id 及關聯的 downloads）
/// 4. 如果沒有 anime_link → 新建
/// 5. 如果無匹配 → 刪除既有 anime_link（此項目本來就沒有成功的下載）並更新狀態
async fn reparse_affected_items(
    db: crate::db::DbPool,
    dispatch_service: std::sync::Arc<crate::services::DownloadDispatchService>,
    sync_service: std::sync::Arc<crate::services::SyncService>,
    item_ids: &[i32],
) -> ReparseStats {

    if item_ids.is_empty() {
        return ReparseStats::default();
    }

    let mut conn = match db.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("reparse_affected_items: 無法取得 DB 連線: {}", e);
            return ReparseStats::default();
        }
    };

    // 載入項目
    let items: Vec<RawAnimeItem> = match raw_anime_items::table
        .filter(raw_anime_items::item_id.eq_any(item_ids))
        .load::<RawAnimeItem>(&mut conn)
    {
        Ok(items) => items,
        Err(e) => {
            tracing::error!("reparse_affected_items: 載入項目失敗: {}", e);
            return ReparseStats::default();
        }
    };

    tracing::info!(
        "reparse_affected_items: 開始重新解析 {} 筆項目",
        items.len()
    );

    let total = items.len();
    let mut parsed_count = 0;
    let mut failed_count = 0;
    let mut no_match_count = 0;
    let mut new_link_ids: Vec<i32> = Vec::new();
    let mut resync_link_ids: Vec<i32> = Vec::new();

    for item in &items {
        match TitleParserService::parse_title(&mut conn, &item.title) {
            Ok(Some(parsed)) => {
                match upsert_anime_link(&mut conn, item, &parsed) {
                    Ok(result) => {
                        if result.is_new {
                            new_link_ids.push(result.link_id);
                        }
                        if result.metadata_changed {
                            resync_link_ids.push(result.link_id);
                        }
                        let is_new = result.is_new;
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
                            "reparse: {} -> {} EP{} ({})",
                            item.title,
                            parsed.anime_title,
                            parsed.episode_no,
                            if is_new { "new" } else { "updated" }
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
                        failed_count += 1;
                        tracing::warn!("reparse: 建立/更新記錄失敗 {}: {}", item.title, e);
                    }
                }
            }
            Ok(None) => {
                // 無匹配：刪除既有 anime_link（沒有正確解析 = 不應有 link）
                // 先查出舊 link 的 series_id，刪除後清理空 series
                let old_series_id: Option<i32> = anime_links::table
                    .filter(anime_links::raw_item_id.eq(item.item_id))
                    .select(anime_links::series_id)
                    .first(&mut conn)
                    .optional()
                    .ok()
                    .flatten();
                diesel::delete(
                    anime_links::table
                        .filter(anime_links::raw_item_id.eq(item.item_id)),
                )
                .execute(&mut conn)
                .ok();
                if let Some(sid) = old_series_id {
                    cleanup_empty_series(&mut conn, sid);
                }
                TitleParserService::update_raw_item_status(
                    &mut conn,
                    item.item_id,
                    ParseStatus::NoMatch,
                    None,
                    None,
                )
                .ok();
                no_match_count += 1;
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
                failed_count += 1;
                tracing::warn!("reparse: 解析錯誤 {}: {}", item.title, e);
            }
        }
    }

    tracing::info!(
        "reparse_affected_items: 完成，共 {} 筆，成功 {}，失敗 {}，無匹配 {}",
        total,
        parsed_count,
        failed_count,
        no_match_count
    );

    // 觸發 dispatch 下載（僅新建的 link 需要 dispatch）
    if !new_link_ids.is_empty() {
        if let Err(e) = dispatch_service.dispatch_new_links(new_link_ids).await {
            tracing::warn!("reparse_affected_items: dispatch 失敗: {}", e);
        }
    }

    // 觸發 resync（metadata 變更的已 synced downloads）
    let mut resync_triggered = 0;
    if !resync_link_ids.is_empty() {
        let mut conn_for_resync = match db.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("reparse: 無法取得 DB 連線用於 resync: {}", e);
                return ReparseStats { total, parsed: parsed_count, failed: failed_count, no_match: no_match_count, resync_triggered };
            }
        };

        // Find synced downloads for these links
        let synced_downloads: Vec<crate::models::Download> = crate::schema::downloads::table
            .filter(crate::schema::downloads::link_id.eq_any(&resync_link_ids))
            .filter(crate::schema::downloads::status.eq("synced"))
            .filter(crate::schema::downloads::file_path.is_not_null())
            .load::<crate::models::Download>(&mut conn_for_resync)
            .unwrap_or_default();

        drop(conn_for_resync);

        if !synced_downloads.is_empty() {
            tracing::info!(
                "reparse: 偵測到 {} 筆已 synced 的 downloads 需要 resync",
                synced_downloads.len()
            );
            for download in &synced_downloads {
                match sync_service.notify_viewer_resync(download).await {
                    Ok(true) => {
                        resync_triggered += 1;
                        tracing::info!("reparse: resync 通知已發送 download_id={}", download.download_id);
                    }
                    Ok(false) => {
                        tracing::warn!("reparse: 無 viewer 可用於 resync download_id={}", download.download_id);
                    }
                    Err(e) => {
                        tracing::error!("reparse: resync 失敗 download_id={}: {}", download.download_id, e);
                    }
                }
            }
        }
    }

    ReparseStats {
        total,
        parsed: parsed_count,
        failed: failed_count,
        no_match: no_match_count,
        resync_triggered,
    }
}

/// 建立或更新 anime_link。
/// 如果此 raw_item 已有 anime_link → 更新欄位（保留 link_id 及 downloads）。
/// 如果沒有 → 新建。
/// 回傳 (link_id, is_new)。
fn upsert_anime_link(
    conn: &mut diesel::PgConnection,
    raw_item: &RawAnimeItem,
    parsed: &crate::services::title_parser::ParsedResult,
) -> Result<UpsertResult, String> {
    use sha2::{Digest, Sha256};

    // 1. 建立或取得 anime / season / series / group
    let anime =
        super::fetcher_results::create_or_get_anime(conn, &parsed.anime_title)?;
    let year = parsed
        .year
        .as_ref()
        .and_then(|y| y.parse::<i32>().ok())
        .unwrap_or(2025);
    let season_name = parsed.season.as_deref().unwrap_or("unknown");
    let season =
        super::fetcher_results::create_or_get_season(conn, year, season_name)?;
    let series = super::fetcher_results::create_or_get_series(
        conn,
        anime.anime_id,
        parsed.series_no,
        season.season_id,
        "",
    )?;
    let group_name = parsed.subtitle_group.as_deref().unwrap_or("未知字幕組");
    let group =
        super::fetcher_results::create_or_get_subtitle_group(conn, group_name)?;

    // 2. 查找既有的 anime_link
    let existing_link: Option<crate::models::AnimeLink> = anime_links::table
        .filter(anime_links::raw_item_id.eq(raw_item.item_id))
        .first(conn)
        .optional()
        .map_err(|e| format!("Failed to query existing link: {}", e))?;

    if let Some(link) = existing_link {
        let old_series_id = link.series_id;
        let old_group_id = link.group_id;
        let old_episode_no = link.episode_no;

        // 3a. 更新既有 link（保留 link_id → downloads 不受影響）
        diesel::update(anime_links::table.filter(anime_links::link_id.eq(link.link_id)))
            .set((
                anime_links::series_id.eq(series.series_id),
                anime_links::group_id.eq(group.group_id),
                anime_links::episode_no.eq(parsed.episode_no),
                anime_links::title.eq(Some(&raw_item.title)),
            ))
            .execute(conn)
            .map_err(|e| format!("Failed to update anime link: {}", e))?;

        // series 變更時，清理已無 link 的舊 series
        if old_series_id != series.series_id {
            cleanup_empty_series(conn, old_series_id);
        }

        // 重算 filtered_flag
        let updated_link: crate::models::AnimeLink = anime_links::table
            .filter(anime_links::link_id.eq(link.link_id))
            .first(conn)
            .map_err(|e| format!("Failed to reload link: {}", e))?;
        if let Ok(flag) =
            crate::services::filter_recalc::compute_filtered_flag_for_link(conn, &updated_link)
        {
            if flag != updated_link.filtered_flag {
                diesel::update(
                    anime_links::table.filter(anime_links::link_id.eq(link.link_id)),
                )
                .set(anime_links::filtered_flag.eq(flag))
                .execute(conn)
                .ok();
            }
        }

        let metadata_changed = old_series_id != series.series_id
            || old_group_id != group.group_id
            || old_episode_no != parsed.episode_no;

        Ok(UpsertResult {
            link_id: link.link_id,
            is_new: false,
            metadata_changed,
        })
    } else {
        // 3b. 新建 link
        let mut hasher = Sha256::new();
        hasher.update(raw_item.download_url.as_bytes());
        let source_hash = format!("{:x}", hasher.finalize());

        let now = chrono::Utc::now().naive_utc();
        let detected_type =
            crate::services::download_type_detector::detect_download_type(&raw_item.download_url);

        let new_link = crate::models::NewAnimeLink {
            series_id: series.series_id,
            group_id: group.group_id,
            episode_no: parsed.episode_no,
            title: Some(raw_item.title.clone()),
            url: raw_item.download_url.clone(),
            source_hash,
            filtered_flag: false,
            created_at: now,
            raw_item_id: Some(raw_item.item_id),
            download_type: detected_type.map(|dt| dt.to_string()),
        };

        let created_link: crate::models::AnimeLink = diesel::insert_into(anime_links::table)
            .values(&new_link)
            .get_result(conn)
            .map_err(|e| format!("Failed to create anime link: {}", e))?;

        // 計算 filtered_flag
        if let Ok(flag) =
            crate::services::filter_recalc::compute_filtered_flag_for_link(conn, &created_link)
        {
            if flag != created_link.filtered_flag {
                diesel::update(
                    anime_links::table.filter(anime_links::link_id.eq(created_link.link_id)),
                )
                .set(anime_links::filtered_flag.eq(flag))
                .execute(conn)
                .ok();
            }
        }

        Ok(UpsertResult {
            link_id: created_link.link_id,
            is_new: true,
            metadata_changed: false,
        })
    }
}

/// 如果指定的 anime_series 底下已經沒有任何 anime_link，就刪除該 series。
fn cleanup_empty_series(conn: &mut diesel::PgConnection, series_id: i32) {
    use crate::schema::anime_series;

    let link_count: i64 = anime_links::table
        .filter(anime_links::series_id.eq(series_id))
        .count()
        .get_result(conn)
        .unwrap_or(1); // 查詢失敗時保守不刪

    if link_count == 0 {
        if let Err(e) = diesel::delete(
            anime_series::table.filter(anime_series::series_id.eq(series_id)),
        )
        .execute(conn)
        {
            tracing::warn!("cleanup_empty_series: 刪除 series {} 失敗: {}", series_id, e);
        } else {
            tracing::info!("cleanup_empty_series: 已移除空的 anime_series {}", series_id);
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
