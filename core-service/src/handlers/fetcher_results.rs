use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::{
    Anime, AnimeLink, AnimeWork, NewAnime, NewAnimeLink, NewAnimeWork, NewSeason, NewSubtitleGroup,
    RawAnimeItem, Season, Subscription, SubtitleGroup,
};
use crate::schema::{anime_links, anime_works, animes, seasons, subscriptions, subtitle_groups};
use crate::services::identity::{
    resolve_season, HttpMetadataLookup, IdentityContext, SeasonResolution,
};
use crate::services::title_parser::ParseStatus;
use crate::services::TitleParserService;
use crate::state::AppState;
use shared::models::{RawFetcherResultsPayload, RawFetcherResultsResponse};

// ============ DTOs for Fetcher Results ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedLinkPayload {
    pub episode_no: i32,
    pub subtitle_group: String,
    pub title: String,
    pub url: String,
    pub source_hash: String,
    pub source_rss_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedAnimePayload {
    pub title: String,
    pub description: String,
    pub season: String, // 冬/春/夏/秋
    pub year: i32,
    pub series_no: i32,
    pub links: Vec<FetchedLinkPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherResultsPayload {
    pub subscription_id: Option<i32>, // 可選，向後相容
    pub animes: Vec<FetchedAnimePayload>,
    pub fetcher_source: String,        // e.g., "mikanani"
    pub success: Option<bool>,         // 抓取是否成功
    pub error_message: Option<String>, // 錯誤訊息
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherResultsResponse {
    pub success: bool,
    pub animes_created: usize,
    pub links_created: usize,
    pub message: String,
}

// ============ Main Handler ============

/// Receive and store fetcher results
pub async fn receive_fetcher_results(
    State(state): State<AppState>,
    Json(payload): Json<FetcherResultsPayload>,
) -> (StatusCode, Json<serde_json::Value>) {
    tracing::info!(
        "Received fetcher results from {}: {} animes, subscription_id: {:?}",
        payload.fetcher_source,
        payload.animes.len(),
        payload.subscription_id
    );

    // 更新訂閱的 last_fetched_at
    if let Some(sub_id) = payload.subscription_id {
        if let Err(e) =
            update_subscription_after_fetch(&state, sub_id, payload.success.unwrap_or(true)).await
        {
            tracing::error!("Failed to update subscription {}: {}", sub_id, e);
        }
    }

    let mut animes_created = 0;
    let mut links_created = 0;
    let mut errors = Vec::new();

    match state.db.get() {
        Ok(mut conn) => {
            for fetched_anime in payload.animes {
                // Create or get anime
                match create_or_get_anime(&mut conn, &fetched_anime.title) {
                    Ok(work) => {
                        animes_created += 1;

                        // Create or get season
                        match create_or_get_season(
                            &mut conn,
                            fetched_anime.year,
                            &fetched_anime.season,
                        ) {
                            Ok(season) => {
                                // Create or get anime (formerly series)
                                match create_or_get_series(
                                    &mut conn,
                                    work.work_id,
                                    fetched_anime.series_no,
                                    season.season_id,
                                    &fetched_anime.description,
                                ) {
                                    Ok(series) => {
                                        // Process each link in the anime
                                        for fetched_link in &fetched_anime.links {
                                            // Create or get subtitle group
                                            match create_or_get_subtitle_group(
                                                &mut conn,
                                                &fetched_link.subtitle_group,
                                            ) {
                                                Ok(group) => {
                                                    // Create anime link
                                                    match create_anime_link(
                                                        &mut conn,
                                                        series.anime_id,
                                                        group.group_id,
                                                        fetched_link,
                                                    ) {
                                                        Ok(_) => {
                                                            links_created += 1;
                                                            tracing::debug!(
                                                                "Created link: {} EP{} from {}",
                                                                fetched_anime.title,
                                                                fetched_link.episode_no,
                                                                fetched_link.subtitle_group
                                                            );
                                                        }
                                                        Err(e) => {
                                                            tracing::warn!(
                                                                "Failed to create link for {}: {}",
                                                                fetched_anime.title,
                                                                e
                                                            );
                                                            errors.push(format!(
                                                                "Link creation failed for {}: {}",
                                                                fetched_anime.title, e
                                                            ));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::warn!(
                                                        "Failed to get/create subtitle group '{}': {}",
                                                        fetched_link.subtitle_group, e
                                                    );
                                                    errors.push(format!(
                                                        "Subtitle group creation failed: {}",
                                                        e
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to get/create series for {}: {}",
                                            fetched_anime.title,
                                            e
                                        );
                                        errors.push(format!(
                                            "Series creation failed for {}: {}",
                                            fetched_anime.title, e
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to get/create season {}/{}: {}",
                                    fetched_anime.year,
                                    fetched_anime.season,
                                    e
                                );
                                errors.push(format!("Season creation failed: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get/create anime '{}': {}",
                            fetched_anime.title,
                            e
                        );
                        errors.push(format!(
                            "Anime creation failed for {}: {}",
                            fetched_anime.title, e
                        ));
                    }
                }
            }

            // Trigger conflict detection after all links created
            if links_created > 0 {
                if let Err(e) = state
                    .dispatch_service
                    .detect_conflicts_and_dispatch(&state.conflict_detection, vec![])
                    .await
                {
                    tracing::warn!("Auto-dispatch after conflict detection failed: {}", e);
                }
            }

            let response = FetcherResultsResponse {
                success: errors.is_empty(),
                animes_created,
                links_created,
                message: if errors.is_empty() {
                    format!(
                        "Successfully ingested {} animes and {} links",
                        animes_created, links_created
                    )
                } else {
                    format!(
                        "Partial success: {} animes, {} links. Errors: {:?}",
                        animes_created, links_created, errors
                    )
                },
            };

            tracing::info!(
                "Fetcher results processing complete: {} animes, {} links",
                animes_created,
                links_created
            );

            (StatusCode::OK, Json(json!(response)))
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "animes_created": 0,
                    "links_created": 0,
                    "message": format!("Database connection error: {}", e)
                })),
            )
        }
    }
}

// ============ Helper Functions ============

/// Create or get an anime work by title
pub(crate) fn create_or_get_anime(
    conn: &mut PgConnection,
    title: &str,
) -> Result<AnimeWork, String> {
    // Try to find existing anime work
    match anime_works::table
        .filter(anime_works::title.eq(title))
        .first::<AnimeWork>(conn)
    {
        Ok(work) if !work.is_active => reactivate_work(conn, work.work_id),
        Ok(work) => {
            tracing::debug!("Found existing anime work: {}", title);
            Ok(work)
        }
        Err(diesel::NotFound) => {
            // Create new anime work
            let now = Utc::now().naive_utc();
            let new_work = NewAnimeWork {
                title: title.to_string(),
                created_at: now,
                updated_at: now,
            };

            diesel::insert_into(anime_works::table)
                .values(&new_work)
                .get_result::<AnimeWork>(conn)
                .map_err(|e| format!("Failed to create anime work: {}", e))
        }
        Err(e) => Err(format!("Failed to query anime work: {}", e)),
    }
}

/// 回填時 soft delete 的作品，又有新項目以同名出現時重新啟用（作品名稱是唯一的）。
fn reactivate_work(conn: &mut PgConnection, work_id: i32) -> Result<AnimeWork, String> {
    tracing::info!("Reactivating soft-deleted anime work {}", work_id);
    diesel::update(anime_works::table.find(work_id))
        .set((
            anime_works::is_active.eq(true),
            anime_works::soft_deleted_at.eq(None::<chrono::NaiveDateTime>),
            anime_works::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<AnimeWork>(conn)
        .map_err(|e| format!("Failed to reactivate anime work {}: {}", work_id, e))
}

/// raw item 屬於哪一季：有身分時由身分決定（spec 第 7 節）；
/// 沒有可用身分時沿用 parser 解析出的作品名與季數。
pub(crate) fn season_for(
    conn: &mut PgConnection,
    identity: &IdentityContext,
    raw_item: &RawAnimeItem,
    parsed: &crate::services::title_parser::ParsedResult,
) -> Result<Anime, String> {
    match resolve_season(conn, identity, &raw_item.external_ids(), parsed.series_no)? {
        SeasonResolution::Resolved(anime) => Ok(anime),
        SeasonResolution::Conflict(reason) => Err(reason),
        SeasonResolution::NoIdentity => {
            tracing::warn!(
                "No usable identity for raw item {} (external ids {:?}); using parser title '{}'",
                raw_item.item_id,
                raw_item.external_ids(),
                parsed.anime_title
            );
            let work = create_or_get_anime(conn, &parsed.anime_title)?;
            let year = parsed
                .year
                .as_ref()
                .and_then(|y| y.parse::<i32>().ok())
                .unwrap_or_else(|| chrono::Datelike::year(&Utc::now()));
            let season_name = parsed.season.as_deref().unwrap_or("unknown");
            let season = create_or_get_season(conn, year, season_name)?;
            create_or_get_series(conn, work.work_id, parsed.series_no, season.season_id, "")
        }
    }
}

/// 為一批 raw item 準備身分資料；查詢失敗時退回不使用身分（沿用 parser 作品名）。
pub(crate) async fn prepare_identity(
    conn: &mut PgConnection,
    db: &crate::db::DbPool,
    items: &[RawAnimeItem],
) -> IdentityContext {
    let ids: Vec<Vec<shared::ExternalId>> = items.iter().map(|i| i.external_ids()).collect();
    IdentityContext::prepare(conn, &HttpMetadataLookup::new(db.clone()), &ids)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to prepare identities, using parser titles: {}", e);
            IdentityContext::empty()
        })
}

/// Create or get a season by year and season name
pub(crate) fn create_or_get_season(
    conn: &mut PgConnection,
    year: i32,
    season_name: &str,
) -> Result<Season, String> {
    // Try to find existing season
    match seasons::table
        .filter(seasons::year.eq(year))
        .filter(seasons::season.eq(season_name))
        .first::<Season>(conn)
    {
        Ok(season) => {
            tracing::debug!("Found existing season: {}/{}", year, season_name);
            Ok(season)
        }
        Err(diesel::NotFound) => {
            // Create new season
            let now = Utc::now().naive_utc();
            let new_season = NewSeason {
                year,
                season: season_name.to_string(),
                created_at: now,
            };

            diesel::insert_into(seasons::table)
                .values(&new_season)
                .get_result::<Season>(conn)
                .map_err(|e| format!("Failed to create season: {}", e))
        }
        Err(e) => Err(format!("Failed to query season: {}", e)),
    }
}

/// Create or get an anime (formerly anime series)
pub(crate) fn create_or_get_series(
    conn: &mut PgConnection,
    work_id: i32,
    series_no: i32,
    season_id: i32,
    description: &str,
) -> Result<Anime, String> {
    // The database unique key is (work_id, series_no); season is editable metadata.
    match animes::table
        .filter(animes::work_id.eq(work_id))
        .filter(animes::series_no.eq(series_no))
        .first::<Anime>(conn)
    {
        Ok(anime) => {
            tracing::debug!(
                "Found existing anime: work_id={}, series_no={}",
                work_id,
                series_no
            );
            Ok(anime)
        }
        Err(diesel::NotFound) => {
            // Create new anime
            let now = Utc::now().naive_utc();
            let new_anime = NewAnime {
                work_id,
                series_no,
                season_id,
                description: if description.is_empty() {
                    None
                } else {
                    Some(description.to_string())
                },
                aired_date: None,
                end_date: None,
                created_at: now,
                updated_at: now,
            };

            diesel::insert_into(animes::table)
                .values(&new_anime)
                .get_result::<Anime>(conn)
                .map_err(|e| format!("Failed to create anime: {}", e))
        }
        Err(e) => Err(format!("Failed to query anime: {}", e)),
    }
}

/// Create or get a subtitle group
pub(crate) fn create_or_get_subtitle_group(
    conn: &mut PgConnection,
    group_name: &str,
) -> Result<SubtitleGroup, String> {
    // Try to find existing subtitle group
    match subtitle_groups::table
        .filter(subtitle_groups::group_name.eq(group_name))
        .first::<SubtitleGroup>(conn)
    {
        Ok(group) => {
            tracing::debug!("Found existing subtitle group: {}", group_name);
            Ok(group)
        }
        Err(diesel::NotFound) => {
            // Create new subtitle group
            let now = Utc::now().naive_utc();
            let new_group = NewSubtitleGroup {
                group_name: group_name.to_string(),
                created_at: now,
            };

            diesel::insert_into(subtitle_groups::table)
                .values(&new_group)
                .get_result::<SubtitleGroup>(conn)
                .map_err(|e| format!("Failed to create subtitle group: {}", e))
        }
        Err(e) => Err(format!("Failed to query subtitle group: {}", e)),
    }
}

/// Create an anime link
fn create_anime_link(
    conn: &mut PgConnection,
    anime_id: i32,
    group_id: i32,
    fetched_link: &FetchedLinkPayload,
) -> Result<AnimeLink, String> {
    let now = Utc::now().naive_utc();
    let new_link = NewAnimeLink {
        anime_id,
        group_id,
        episode_no: fetched_link.episode_no,
        title: Some(fetched_link.title.clone()),
        url: fetched_link.url.clone(),
        source_hash: fetched_link.source_hash.clone(),
        filtered_flag: false,
        created_at: now,
        raw_item_id: None,
        download_type: None,
        conflict_flag: false,
        link_status: "active".to_string(),
    };

    diesel::insert_into(anime_links::table)
        .values(&new_link)
        .get_result::<AnimeLink>(conn)
        .map_err(|e| format!("Failed to create anime link: {}", e))
}

/// 更新訂閱的 last_fetched_at 和 next_fetch_at
async fn update_subscription_after_fetch(
    state: &AppState,
    subscription_id: i32,
    _success: bool,
) -> Result<(), String> {
    let mut conn = state.db.get().map_err(|e| e.to_string())?;
    let now = Utc::now().naive_utc();

    // 先取得訂閱資訊
    let subscription = subscriptions::table
        .filter(subscriptions::subscription_id.eq(subscription_id))
        .first::<Subscription>(&mut conn)
        .map_err(|e| format!("Subscription not found: {}", e))?;

    // interval=0 代表單次抓取，抓完後不排下一次
    let next_fetch = if subscription.fetch_interval_minutes == 0 {
        None
    } else {
        Some(now + chrono::Duration::minutes(subscription.fetch_interval_minutes as i64))
    };

    diesel::update(subscriptions::table.filter(subscriptions::subscription_id.eq(subscription_id)))
        .set((
            subscriptions::last_fetched_at.eq(Some(now)),
            subscriptions::next_fetch_at.eq(next_fetch),
            subscriptions::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to update subscription: {}", e))?;

    tracing::info!(
        "Updated subscription {}: last_fetched_at={}, next_fetch_at={:?}",
        subscription_id,
        now,
        next_fetch
    );

    Ok(())
}

// ============ Raw Fetcher Results Handler (New Architecture) ============

/// 處理新架構的原始 fetcher 結果
pub async fn receive_raw_fetcher_results(
    State(state): State<AppState>,
    Json(payload): Json<RawFetcherResultsPayload>,
) -> (StatusCode, Json<serde_json::Value>) {
    tracing::info!(
        "Received raw fetcher results: {} items, subscription_id: {}",
        payload.items.len(),
        payload.subscription_id
    );

    // 更新訂閱的 last_fetched_at
    if let Err(e) =
        update_subscription_after_fetch(&state, payload.subscription_id, payload.success).await
    {
        tracing::error!(
            "Failed to update subscription {}: {}",
            payload.subscription_id,
            e
        );
    }

    let items_received = payload.items.len();
    let mut items_parsed = 0;
    let mut items_failed = 0;
    let mut errors = Vec::new();
    let mut new_link_ids: Vec<i32> = Vec::new();

    let pool = std::sync::Arc::new(state.db.clone());

    match state.db.get() {
        Ok(mut conn) => {
            // 儲存原始項目（重複項目會因 UNIQUE 被略過）
            let saved_items: Vec<RawAnimeItem> = payload
                .items
                .iter()
                .filter_map(|raw_item| {
                    TitleParserService::save_raw_item(
                        &mut conn,
                        &raw_item.title,
                        raw_item.description.as_deref(),
                        &raw_item.download_url,
                        raw_item.pub_date.map(|dt| dt.naive_utc()),
                        payload.subscription_id,
                        &raw_item.external_ids,
                    )
                    .map_err(|e| tracing::debug!("Skipped item (possibly duplicate): {}", e))
                    .ok()
                })
                .collect();

            let identity = prepare_identity(&mut conn, &state.db, &saved_items).await;

            for saved_item in saved_items {
                let raw_item = &saved_item;
                // 嘗試解析標題
                match TitleParserService::parse_title(
                    &mut conn,
                    &raw_item.title,
                    saved_item.subscription_id,
                ) {
                    Ok(Some(parsed)) => {
                        // 解析成功，建立相關記錄
                        match process_parsed_result(&mut conn, &identity, &saved_item, &parsed) {
                            Ok(ids) => {
                                new_link_ids.extend(ids);
                                TitleParserService::update_raw_item_status(
                                    &mut conn,
                                    saved_item.item_id,
                                    ParseStatus::Parsed,
                                    Some(parsed.parser_id),
                                    None,
                                )
                                .ok();
                                items_parsed += 1;
                                tracing::debug!(
                                    "Parsed: {} -> {} EP{}",
                                    raw_item.title,
                                    parsed.anime_title,
                                    parsed.episode_no
                                );
                            }
                            Err(e) => {
                                TitleParserService::update_raw_item_status(
                                    &mut conn,
                                    saved_item.item_id,
                                    ParseStatus::Failed,
                                    Some(parsed.parser_id),
                                    Some(&e),
                                )
                                .ok();
                                items_failed += 1;
                                errors.push(e);
                            }
                        }
                    }
                    Ok(None) => {
                        // 無匹配解析器
                        TitleParserService::update_raw_item_status(
                            &mut conn,
                            saved_item.item_id,
                            ParseStatus::NoMatch,
                            None,
                            Some("No matching parser found"),
                        )
                        .ok();
                        items_failed += 1;
                        tracing::warn!("No parser matched for: {}", raw_item.title);
                    }
                    Err(e) => {
                        TitleParserService::update_raw_item_status(
                            &mut conn,
                            saved_item.item_id,
                            ParseStatus::Failed,
                            None,
                            Some(&e),
                        )
                        .ok();
                        items_failed += 1;
                        errors.push(e);
                    }
                }
            }

            // 若有未匹配項目，背景觸發批次 AI 解析器生成
            if items_failed > 0 {
                let pool_clone = pool.clone();
                let sub_id = payload.subscription_id;
                tokio::spawn(async move {
                    if let Err(e) =
                        crate::ai::parser_generator::generate_parsers_for_subscription_batch(
                            pool_clone, sub_id,
                        )
                        .await
                    {
                        tracing::warn!(
                            "Batch parser generation failed for subscription={}: {}",
                            sub_id,
                            e
                        );
                    }
                });
            }

            // 批次派發新建的下載連結
            if !new_link_ids.is_empty() {
                let link_count = new_link_ids.len();

                match state
                    .dispatch_service
                    .detect_conflicts_and_dispatch(&state.conflict_detection, new_link_ids)
                    .await
                {
                    Ok(result) => {
                        tracing::info!(
                            "Download dispatch: {} links -> {} dispatched, {} no_downloader, {} failed",
                            link_count, result.dispatched, result.no_downloader, result.failed
                        );
                    }
                    Err(e) => {
                        tracing::error!("Download dispatch failed: {}", e);
                    }
                }
            }

            let response = RawFetcherResultsResponse {
                success: items_failed == 0,
                items_received,
                items_parsed,
                items_failed,
                message: if errors.is_empty() {
                    format!(
                        "Processed {} items: {} parsed, {} failed",
                        items_received, items_parsed, items_failed
                    )
                } else {
                    format!(
                        "Processed {} items with errors: {:?}",
                        items_received, errors
                    )
                },
            };

            (StatusCode::OK, Json(json!(response)))
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!(RawFetcherResultsResponse {
                    success: false,
                    items_received: 0,
                    items_parsed: 0,
                    items_failed: 0,
                    message: format!("Database connection error: {}", e),
                })),
            )
        }
    }
}

/// 處理解析成功的結果，建立 anime, series, group, link 記錄
/// 回傳新建的 link_id 列表（批次合輯時為多個）
pub(crate) fn process_parsed_result(
    conn: &mut PgConnection,
    identity: &IdentityContext,
    raw_item: &RawAnimeItem,
    parsed: &crate::services::title_parser::ParsedResult,
) -> Result<Vec<i32>, String> {
    use sha2::{Digest, Sha256};

    // 1–3. 決定季（身分優先，沒有身分時用 parser 作品名）
    let anime = season_for(conn, identity, raw_item, parsed)?;

    // 4. 建立或取得 subtitle_group
    let group_name = parsed.subtitle_group.as_deref().unwrap_or("未知字幕組");
    let group = create_or_get_subtitle_group(conn, group_name)?;

    // 5. 計算 base source_hash
    let mut hasher = Sha256::new();
    hasher.update(raw_item.download_url.as_bytes());
    let base_hash = format!("{:x}", hasher.finalize());

    // 決定 episode 範圍（批次合輯展開）
    let ep_start = parsed.episode_no;
    let ep_end = match parsed.episode_end {
        Some(end) if end >= ep_start && (end - ep_start) <= 200 => end,
        Some(bad) => {
            tracing::warn!(
                "episode_end ({}) is invalid relative to episode_no ({}), treating as single episode",
                bad, ep_start
            );
            ep_start
        }
        None => ep_start,
    };

    let is_batch = ep_end > ep_start;
    let now = Utc::now().naive_utc();
    let detected_type =
        crate::services::download_type_detector::detect_download_type(&raw_item.download_url);
    let mut link_ids = Vec::new();

    for ep in ep_start..=ep_end {
        let source_hash = if is_batch {
            format!("{}#ep{}", base_hash, ep)
        } else {
            base_hash.clone()
        };

        let new_link = NewAnimeLink {
            anime_id: anime.anime_id,
            group_id: group.group_id,
            episode_no: ep,
            title: Some(raw_item.title.clone()),
            url: raw_item.download_url.clone(),
            source_hash,
            filtered_flag: false,
            created_at: now,
            raw_item_id: Some(raw_item.item_id),
            download_type: detected_type.as_ref().map(|dt| dt.to_string()),
            conflict_flag: false,
            link_status: "active".to_string(),
        };

        let created_link: AnimeLink = diesel::insert_into(anime_links::table)
            .values(&new_link)
            .get_result(conn)
            .map_err(|e| format!("Failed to create anime link ep {}: {}", ep, e))?;

        // Compute and update filtered_flag based on current filter rules
        match crate::services::filter_recalc::compute_filtered_flag_for_link(conn, &created_link) {
            Ok(flag) if flag != created_link.filtered_flag => {
                diesel::update(
                    anime_links::table.filter(anime_links::link_id.eq(created_link.link_id)),
                )
                .set(anime_links::filtered_flag.eq(flag))
                .execute(conn)
                .map_err(|e| format!("Failed to update filtered_flag: {}", e))?;
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to compute filtered_flag for link {}: {}",
                    created_link.link_id,
                    e
                );
            }
            _ => {}
        }

        link_ids.push(created_link.link_id);
    }

    Ok(link_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::identity::{save_metadata_namespaces, test_db, IdentityContext};
    use crate::services::title_parser::ParsedResult;

    fn parsed(title: &str, series_no: i32) -> ParsedResult {
        ParsedResult {
            anime_title: title.to_string(),
            episode_no: 1,
            episode_end: None,
            series_no,
            subtitle_group: Some("LoliHouse".to_string()),
            resolution: None,
            season: None,
            year: None,
            parser_id: 0,
        }
    }

    fn raw_item(conn: &mut PgConnection, ids: &[shared::ExternalId]) -> RawAnimeItem {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let sub_id: i32 = diesel::sql_query(
            "INSERT INTO subscriptions (fetcher_id, source_url, name, created_at, updated_at) \
             SELECT module_id, $1, 's', now(), now() \
             FROM service_modules LIMIT 1 RETURNING subscription_id",
        )
        .bind::<diesel::sql_types::Text, _>(format!(
            "https://mikanani.me/RSS/Bangumi?bangumiId={n}"
        ))
        .get_result::<SubscriptionId>(conn)
        .unwrap()
        .subscription_id;
        let url = format!("magnet:?t{n}");
        TitleParserService::save_raw_item(conn, "[X] t [01]", None, &url, None, sub_id, ids)
            .unwrap()
    }

    #[derive(diesel::QueryableByName)]
    struct SubscriptionId {
        #[diesel(sql_type = diesel::sql_types::Integer)]
        subscription_id: i32,
    }

    async fn context(conn: &mut PgConnection, item: &RawAnimeItem) -> IdentityContext {
        IdentityContext::prepare(conn, &test_db::lookup(), &[item.external_ids()])
            .await
            .unwrap()
    }

    fn setup() -> PgConnection {
        let mut conn = test_db::conn();
        test_db::insert_module(&mut conn, "meta-bgm", true);
        save_metadata_namespaces(&mut conn, "meta-bgm", &["bgm".to_string()]).unwrap();
        conn
    }

    fn work_titled(conn: &mut PgConnection, title: &str) -> Option<AnimeWork> {
        anime_works::table
            .filter(anime_works::title.eq(title))
            .first(conn)
            .optional()
            .unwrap()
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn an_item_with_identity_ignores_the_parser_title() {
        let mut conn = setup();
        let item = raw_item(&mut conn, &[shared::ExternalId::new("bgm", "548818")]);
        let identity = context(&mut conn, &item).await;
        let season = season_for(&mut conn, &identity, &item, &parsed("てんびん", 2)).unwrap();
        assert_eq!(season.title.as_deref(), Some("金牌得主 第二季"));
        assert!(work_titled(&mut conn, "てんびん").is_none());
        assert_eq!(
            item.external_ids(),
            vec![shared::ExternalId::new("bgm", "548818")]
        );
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn an_item_without_identity_keeps_the_parser_title() {
        let mut conn = setup();
        let item = raw_item(&mut conn, &[]);
        let identity = context(&mut conn, &item).await;
        let season = season_for(&mut conn, &identity, &item, &parsed("てんびん", 1)).unwrap();
        let work = work_titled(&mut conn, "てんびん").expect("parser-titled work");
        assert_eq!(season.work_id, work.work_id);
        assert_eq!(work.title_source, "parser");
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn a_season_number_conflict_is_an_error_with_the_reason() {
        let mut conn = setup();
        let s1 = raw_item(&mut conn, &[shared::ExternalId::new("bgm", "430699")]);
        let identity = context(&mut conn, &s1).await;
        season_for(&mut conn, &identity, &s1, &parsed("x", 1)).unwrap();

        let s2 = raw_item(&mut conn, &[shared::ExternalId::new("bgm", "548818")]);
        let identity = context(&mut conn, &s2).await;
        let err = season_for(&mut conn, &identity, &s2, &parsed("x", 1)).unwrap_err();
        assert!(err.contains("bgm/430699"), "{err}");
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn a_soft_deleted_work_comes_back_when_its_title_reappears() {
        let mut conn = setup();
        let now = Utc::now().naive_utc();
        let work: AnimeWork = diesel::insert_into(anime_works::table)
            .values(NewAnimeWork {
                title: "殘骸".to_string(),
                created_at: now,
                updated_at: now,
            })
            .get_result(&mut conn)
            .unwrap();
        diesel::update(anime_works::table.find(work.work_id))
            .set((
                anime_works::is_active.eq(false),
                anime_works::soft_deleted_at.eq(Some(now)),
            ))
            .execute(&mut conn)
            .unwrap();
        let item = raw_item(&mut conn, &[]);
        let identity = context(&mut conn, &item).await;
        season_for(&mut conn, &identity, &item, &parsed("殘骸", 1)).unwrap();
        let revived = work_titled(&mut conn, "殘骸").unwrap();
        assert!(revived.is_active);
        assert!(revived.soft_deleted_at.is_none());
    }
}
