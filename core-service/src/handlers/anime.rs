use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;

use diesel::dsl::{count_distinct, exists};
use diesel::prelude::*;

use crate::db::CreateAnimeParams;
use crate::dto::{
    AnimeRequest, AnimeResponse, AnimeRichResponse, AnimeWorkRequest, AnimeWorkResponse,
    AnimeWorksListResponse, DownloadInfo, SeasonInfo, SeasonRequest, SeasonResponse,
    SubscriptionInfo, SubtitleGroupRequest, SubtitleGroupResponse, UpdateAnimeRequest,
};
use crate::models::{Anime, AnimeWork, Download, Season};
use crate::schema::{
    anime_cover_images, anime_links, anime_works, animes, downloads, raw_anime_items, seasons,
    subscriptions,
};
use crate::state::AppState;

// ============ AnimeWork Handlers ============

/// Create a new anime work
#[utoipa::path(
    post,
    path = "/api/core/anime-works",
    tag = "AnimeWorks",
    request_body = AnimeWorkRequest,
    responses(
        (status = 201, description = "Created successfully", body = AnimeWorkResponse),
        (status = 500, description = "Database error")
    )
)]
pub async fn create_anime_work(
    State(state): State<AppState>,
    Json(payload): Json<AnimeWorkRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.anime_work.create(payload.title).await {
        Ok(anime_work) => {
            tracing::info!("Created anime work: {}", anime_work.work_id);
            // 背景取得封面圖
            let db_clone = state.db.clone();
            let wid = anime_work.work_id;
            tokio::spawn(async move {
                fetch_and_store_covers(db_clone, wid).await;
            });
            let response = AnimeWorkResponse {
                anime_id: anime_work.work_id,
                title: anime_work.title,
                created_at: anime_work.created_at,
                updated_at: anime_work.updated_at,
            };
            (StatusCode::CREATED, Json(json!(response)))
        }
        Err(e) => {
            tracing::error!("Failed to create anime work: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to create anime work: {:?}", e)
                })),
            )
        }
    }
}

#[derive(Debug, serde::Deserialize, Default, utoipa::IntoParams)]
pub struct AnimeWorksQuery {
    /// Filter to anime works that have at least one link
    pub has_links: Option<bool>,
}

/// List all anime works
#[utoipa::path(
    get,
    path = "/api/core/anime-works",
    tag = "AnimeWorks",
    params(AnimeWorksQuery),
    responses(
        (status = 200, description = "Success", body = AnimeWorksListResponse),
        (status = 500, description = "Database error")
    )
)]
pub async fn list_anime_work(
    State(state): State<AppState>,
    Query(query): Query<AnimeWorksQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    // When has_links=true, filter to anime works that have at least one anime_link
    if query.has_links.unwrap_or(false) {
        let mut conn = match state.db.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    "Failed to get DB connection for anime works (has_links): {:?}",
                    e
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "database_error",
                        "message": format!("DB pool error: {:?}", e),
                        "animes": []
                    })),
                );
            }
        };
        let work_list = anime_works::table
            .filter(exists(
                animes::table
                    .inner_join(anime_links::table.on(anime_links::anime_id.eq(animes::anime_id)))
                    .filter(animes::work_id.eq(anime_works::work_id))
                    .select(anime_links::link_id),
            ))
            .order(anime_works::work_id.asc())
            .load::<crate::models::AnimeWork>(&mut conn);

        return match work_list {
            Ok(works) => {
                let responses: Vec<AnimeWorkResponse> = works
                    .into_iter()
                    .map(|a| AnimeWorkResponse {
                        anime_id: a.work_id,
                        title: a.title,
                        created_at: a.created_at,
                        updated_at: a.updated_at,
                    })
                    .collect();
                tracing::info!("Listed {} anime works (has_links=true)", responses.len());
                (StatusCode::OK, Json(json!({ "animes": responses })))
            }
            Err(e) => {
                tracing::error!("Failed to list anime works (has_links=true): {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "database_error",
                        "message": format!("Failed to list anime works: {:?}", e),
                        "animes": []
                    })),
                )
            }
        };
    }

    match state.repos.anime_work.find_all().await {
        Ok(work_list) => {
            let responses: Vec<AnimeWorkResponse> = work_list
                .into_iter()
                .map(|a| AnimeWorkResponse {
                    anime_id: a.work_id,
                    title: a.title,
                    created_at: a.created_at,
                    updated_at: a.updated_at,
                })
                .collect();
            tracing::info!("Listed {} anime works", responses.len());
            (StatusCode::OK, Json(json!({ "animes": responses })))
        }
        Err(e) => {
            tracing::error!("Failed to list anime works: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to list anime works: {:?}", e),
                    "animes": []
                })),
            )
        }
    }
}

/// Get anime work by ID
#[utoipa::path(
    get,
    path = "/api/core/anime-works/{work_id}",
    tag = "AnimeWorks",
    params(("work_id" = i32, Path, description = "Anime work ID")),
    responses(
        (status = 200, description = "Success", body = AnimeWorkResponse),
        (status = 404, description = "Not found"),
        (status = 500, description = "Database error")
    )
)]
pub async fn get_anime_work(
    State(state): State<AppState>,
    Path(work_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.anime_work.find_by_id(work_id).await {
        Ok(Some(anime_work)) => {
            let response = AnimeWorkResponse {
                anime_id: anime_work.work_id,
                title: anime_work.title,
                created_at: anime_work.created_at,
                updated_at: anime_work.updated_at,
            };
            tracing::info!("Retrieved anime work: {}", work_id);
            (StatusCode::OK, Json(json!(response)))
        }
        Ok(None) => {
            tracing::warn!("Anime work not found: {}", work_id);
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "not_found",
                    "message": format!("Anime work {} not found", work_id)
                })),
            )
        }
        Err(e) => {
            tracing::error!("Failed to get anime work: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to get anime work: {:?}", e)
                })),
            )
        }
    }
}

/// 系列名稱：去頭尾空白，不可空白、不可超過 255 字
fn normalized_work_title(title: &str) -> Option<String> {
    let title = title.trim();
    (!title.is_empty() && title.chars().count() <= 255).then(|| title.to_string())
}

/// 手動改系列名稱；之後身分解析與回填都不再自動改它（title_source = manual）。
/// 已同步的集數不會自動搬資料夾，需要時執行 resync。
#[utoipa::path(
    put,
    path = "/api/core/anime-works/{work_id}",
    tag = "AnimeWorks",
    params(("work_id" = i32, Path, description = "Anime work ID")),
    request_body = AnimeWorkRequest,
    responses(
        (status = 200, description = "Renamed", body = AnimeWorkResponse),
        (status = 400, description = "Empty or too long title"),
        (status = 404, description = "Not found"),
        (status = 409, description = "Another work already has this title"),
        (status = 500, description = "Database error")
    )
)]
pub async fn update_anime_work(
    State(state): State<AppState>,
    Path(work_id): Path<i32>,
    Json(req): Json<AnimeWorkRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(title) = normalized_work_title(&req.title) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_title",
                "message": "Title must be 1–255 characters"
            })),
        );
    };
    let mut conn = match state.db.get() {
        Ok(conn) => conn,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database_error", "message": e.to_string() })),
            )
        }
    };
    let updated = diesel::update(anime_works::table.find(work_id))
        .set((
            anime_works::title.eq(&title),
            anime_works::title_source.eq("manual"),
            anime_works::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .get_result::<AnimeWork>(&mut conn);
    match updated {
        Ok(work) => {
            tracing::info!(
                "Anime work {} renamed to '{}' (manual)",
                work_id,
                work.title
            );
            (
                StatusCode::OK,
                Json(json!(AnimeWorkResponse {
                    anime_id: work.work_id,
                    title: work.title,
                    created_at: work.created_at,
                    updated_at: work.updated_at,
                })),
            )
        }
        Err(diesel::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "not_found",
                "message": format!("Anime work {} not found", work_id)
            })),
        ),
        Err(diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            _,
        )) => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "conflict",
                "message": format!("Another anime work is already titled '{}'", title)
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "database_error", "message": e.to_string() })),
        ),
    }
}

/// Delete anime work by ID
#[utoipa::path(
    delete,
    path = "/api/core/anime-works/{work_id}",
    tag = "AnimeWorks",
    params(("work_id" = i32, Path, description = "Anime work ID")),
    responses(
        (status = 200, description = "Deleted successfully"),
        (status = 404, description = "Not found"),
        (status = 500, description = "Database error")
    )
)]
pub async fn delete_anime_work(
    State(state): State<AppState>,
    Path(work_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.anime_work.delete(work_id).await {
        Ok(true) => {
            tracing::info!("Deleted anime work: {}", work_id);
            (StatusCode::OK, Json(json!({ "deleted": true })))
        }
        Ok(false) => {
            tracing::warn!("Anime work not found for deletion: {}", work_id);
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "not_found",
                    "message": format!("Anime work {} not found", work_id)
                })),
            )
        }
        Err(e) => {
            tracing::error!("Failed to delete anime work: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to delete anime work: {:?}", e)
                })),
            )
        }
    }
}

// ============ Season Handlers ============

/// Create a new season
#[utoipa::path(
    post,
    path = "/api/core/seasons",
    tag = "Seasons",
    request_body = SeasonRequest,
    responses(
        (status = 201, description = "Created or found existing season", body = SeasonResponse),
        (status = 500, description = "Database error")
    )
)]
pub async fn create_season(
    State(state): State<AppState>,
    Json(payload): Json<SeasonRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state
        .repos
        .season
        .find_or_create(payload.year, payload.season)
        .await
    {
        Ok(season) => {
            tracing::info!("Created season: {}", season.season_id);
            let response = SeasonResponse {
                season_id: season.season_id,
                year: season.year,
                season: season.season,
                created_at: season.created_at,
            };
            (StatusCode::CREATED, Json(json!(response)))
        }
        Err(e) => {
            tracing::error!("Failed to create season: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to create season: {:?}", e)
                })),
            )
        }
    }
}

/// List all seasons
#[utoipa::path(
    get,
    path = "/api/core/seasons",
    tag = "Seasons",
    responses(
        (status = 200, description = "Success"),
        (status = 500, description = "Database error")
    )
)]
pub async fn list_seasons(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.season.find_all().await {
        Ok(season_list) => {
            let responses: Vec<SeasonResponse> = season_list
                .into_iter()
                .map(|s| SeasonResponse {
                    season_id: s.season_id,
                    year: s.year,
                    season: s.season,
                    created_at: s.created_at,
                })
                .collect();
            tracing::info!("Listed {} seasons", responses.len());
            (StatusCode::OK, Json(json!({ "seasons": responses })))
        }
        Err(e) => {
            tracing::error!("Failed to list seasons: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to list seasons: {:?}", e),
                    "seasons": []
                })),
            )
        }
    }
}

// ============ Anime Handlers (formerly AnimeSeries) ============

#[derive(Debug, serde::Deserialize, Default)]
pub struct ExcludeEmptyParams {
    #[serde(default)]
    exclude_empty: bool,
}

/// List all anime with enriched data (work_title, season, episode counts, subscriptions)
#[utoipa::path(
    get,
    path = "/api/core/anime",
    tag = "Anime",
    responses(
        (status = 200, description = "Success", body = Vec<AnimeRichResponse>),
        (status = 500, description = "Database error")
    )
)]
pub async fn list_all_anime(
    State(state): State<AppState>,
    Query(params): Query<ExcludeEmptyParams>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string(), "series": [] })),
            );
        }
    };

    // Load all anime with joined anime_work and season
    let anime_with_joins: Vec<(Anime, AnimeWork, Season)> = match animes::table
        .inner_join(anime_works::table.on(anime_works::work_id.eq(animes::work_id)))
        .inner_join(seasons::table.on(seasons::season_id.eq(animes::season_id)))
        .select((
            Anime::as_select(),
            AnimeWork::as_select(),
            Season::as_select(),
        ))
        .order(animes::anime_id.desc())
        .load::<(Anime, AnimeWork, Season)>(&mut conn)
    {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string(), "series": [] })),
            );
        }
    };

    let mut results = Vec::new();

    // Batch-fetch default cover images to avoid N+1 queries
    let cover_map: std::collections::HashMap<i32, String> = {
        match anime_cover_images::table
            .filter(anime_cover_images::is_default.eq(true))
            .select((anime_cover_images::work_id, anime_cover_images::image_url))
            .load::<(i32, String)>(&mut conn)
        {
            Ok(rows) => rows.into_iter().collect(),
            Err(_) => std::collections::HashMap::new(),
        }
    };

    for (anime, anime_work, season) in &anime_with_joins {
        // episode_found: distinct episode_no where filtered_flag = false
        let episode_found: i64 = anime_links::table
            .filter(anime_links::anime_id.eq(anime.anime_id))
            .filter(anime_links::filtered_flag.eq(false))
            .select(count_distinct(anime_links::episode_no))
            .first(&mut conn)
            .unwrap_or(0);

        // episode_downloaded: distinct episode_no where filtered_flag = false AND download completed
        let episode_downloaded: i64 = anime_links::table
            .inner_join(downloads::table.on(downloads::link_id.eq(anime_links::link_id)))
            .filter(anime_links::anime_id.eq(anime.anime_id))
            .filter(anime_links::filtered_flag.eq(false))
            .filter(downloads::status.eq_any(["completed", "synced"]))
            .select(count_distinct(anime_links::episode_no))
            .first(&mut conn)
            .unwrap_or(0);

        // subscriptions: via anime_links → raw_anime_items → subscriptions
        let sub_infos: Vec<SubscriptionInfo> = match anime_links::table
            .inner_join(
                raw_anime_items::table.on(raw_anime_items::item_id
                    .nullable()
                    .eq(anime_links::raw_item_id)),
            )
            .inner_join(
                subscriptions::table
                    .on(subscriptions::subscription_id.eq(raw_anime_items::subscription_id)),
            )
            .filter(anime_links::anime_id.eq(anime.anime_id))
            .select((subscriptions::subscription_id, subscriptions::name))
            .distinct()
            .load::<(i32, Option<String>)>(&mut conn)
        {
            Ok(subs) => subs
                .into_iter()
                .map(|(id, name)| SubscriptionInfo {
                    subscription_id: id,
                    name,
                })
                .collect(),
            Err(_) => vec![],
        };

        results.push(AnimeRichResponse {
            series_id: anime.anime_id,
            anime_id: anime.work_id,
            anime_title: anime_work.title.clone(),
            series_no: anime.series_no,
            season: SeasonInfo {
                year: season.year,
                season: season.season.clone(),
            },
            episode_downloaded,
            episode_found,
            subscriptions: sub_infos,
            description: anime.description.clone(),
            aired_date: anime.aired_date,
            end_date: anime.end_date,
            created_at: anime.created_at,
            updated_at: anime.updated_at,
            cover_image_url: cover_map.get(&anime.work_id).cloned(),
        });
    }

    if params.exclude_empty {
        results.retain(|r| r.episode_found > 0);
    }

    // Backfill missing cover images in background (debounced)
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static COVER_FETCH_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

        let work_ids_with_covers: std::collections::HashSet<i32> =
            cover_map.keys().cloned().collect();
        let missing: std::collections::BTreeSet<i32> = results
            .iter()
            .filter(|r| !work_ids_with_covers.contains(&r.anime_id))
            .map(|r| r.anime_id)
            .collect();

        if !missing.is_empty()
            && COVER_FETCH_IN_PROGRESS
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            let db_clone = state.db.clone();
            tokio::spawn(async move {
                for id in missing {
                    fetch_and_store_covers(db_clone.clone(), id).await;
                }
                COVER_FETCH_IN_PROGRESS.store(false, Ordering::SeqCst);
            });
        }
    }

    (StatusCode::OK, Json(json!({ "series": results })))
}

/// Create a new anime (formerly anime series)
#[utoipa::path(
    post,
    path = "/api/core/anime",
    tag = "Anime",
    request_body = AnimeRequest,
    responses(
        (status = 201, description = "Created successfully", body = AnimeResponse),
        (status = 500, description = "Database error")
    )
)]
pub async fn create_anime(
    State(state): State<AppState>,
    Json(payload): Json<AnimeRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let params = CreateAnimeParams {
        work_id: payload.anime_id,
        series_no: payload.series_no,
        season_id: payload.season_id,
        description: payload.description,
        aired_date: payload.aired_date,
        end_date: payload.end_date,
    };

    match state.repos.anime.create(params).await {
        Ok(anime) => {
            tracing::info!("Created anime: {}", anime.anime_id);
            let response = AnimeResponse {
                series_id: anime.anime_id,
                anime_id: anime.work_id,
                series_no: anime.series_no,
                season_id: anime.season_id,
                description: anime.description,
                aired_date: anime.aired_date,
                end_date: anime.end_date,
                created_at: anime.created_at,
                updated_at: anime.updated_at,
            };
            (StatusCode::CREATED, Json(json!(response)))
        }
        Err(e) => {
            tracing::error!("Failed to create anime: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to create anime: {:?}", e)
                })),
            )
        }
    }
}

/// Get anime by ID
#[utoipa::path(
    get,
    path = "/api/core/anime/{anime_id}",
    tag = "Anime",
    params(("anime_id" = i32, Path, description = "Anime series ID")),
    responses(
        (status = 200, description = "Success", body = AnimeResponse),
        (status = 404, description = "Not found"),
        (status = 500, description = "Database error")
    )
)]
/// Get anime by ID (formerly anime series)
pub async fn get_anime(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.anime.find_by_id(anime_id).await {
        Ok(Some(anime)) => {
            let response = AnimeResponse {
                series_id: anime.anime_id,
                anime_id: anime.work_id,
                series_no: anime.series_no,
                season_id: anime.season_id,
                description: anime.description,
                aired_date: anime.aired_date,
                end_date: anime.end_date,
                created_at: anime.created_at,
                updated_at: anime.updated_at,
            };
            tracing::info!("Retrieved anime: {}", anime_id);
            (StatusCode::OK, Json(json!(response)))
        }
        Ok(None) => {
            tracing::warn!("Anime not found: {}", anime_id);
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "not_found",
                    "message": format!("Anime {} not found", anime_id)
                })),
            )
        }
        Err(e) => {
            tracing::error!("Failed to get anime: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to get anime: {:?}", e)
                })),
            )
        }
    }
}

/// Update anime by ID (partial update: description, aired_date, end_date)
#[utoipa::path(
    put,
    path = "/api/core/anime/{anime_id}",
    tag = "Anime",
    params(("anime_id" = i32, Path, description = "Anime series ID")),
    request_body = UpdateAnimeRequest,
    responses(
        (status = 200, description = "Updated successfully", body = AnimeResponse),
        (status = 404, description = "Not found"),
        (status = 500, description = "Database error")
    )
)]
pub async fn update_anime(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
    Json(payload): Json<UpdateAnimeRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };

    use diesel::prelude::*;
    let mut query = diesel::update(animes::table.find(anime_id))
        .set((
            animes::description.eq(payload.description),
            animes::aired_date.eq(payload.aired_date),
            animes::end_date.eq(payload.end_date),
            animes::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .get_result::<crate::models::Anime>(&mut conn);

    // If season_id provided, do a separate update for it
    if let Some(sid) = payload.season_id {
        let _ = diesel::update(animes::table.find(anime_id))
            .set(animes::season_id.eq(sid))
            .execute(&mut conn);
        // Re-fetch after both updates
        query = animes::table
            .find(anime_id)
            .first::<crate::models::Anime>(&mut conn);
    }

    match query {
        Ok(anime) => {
            tracing::info!("Updated anime: {}", anime_id);
            let response = AnimeResponse {
                series_id: anime.anime_id,
                anime_id: anime.work_id,
                series_no: anime.series_no,
                season_id: anime.season_id,
                description: anime.description,
                aired_date: anime.aired_date,
                end_date: anime.end_date,
                created_at: anime.created_at,
                updated_at: anime.updated_at,
            };
            (StatusCode::OK, Json(json!(response)))
        }
        Err(diesel::result::Error::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "not_found",
                "message": format!("Anime {} not found", anime_id)
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "database_error",
                "message": format!("Failed to update anime: {:?}", e)
            })),
        ),
    }
}

/// List anime by work_id
#[utoipa::path(
    get,
    path = "/api/core/anime-works/{work_id}/anime",
    tag = "Anime",
    params(("work_id" = i32, Path, description = "Anime work ID")),
    responses(
        (status = 200, description = "Success"),
        (status = 500, description = "Database error")
    )
)]
/// List anime by work_id (formerly list_anime_series by anime_id)
pub async fn list_anime(
    State(state): State<AppState>,
    Path(work_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.anime.find_by_work_id(work_id).await {
        Ok(anime_list) => {
            let responses: Vec<AnimeResponse> = anime_list
                .into_iter()
                .map(|a| AnimeResponse {
                    series_id: a.anime_id,
                    anime_id: a.work_id,
                    series_no: a.series_no,
                    season_id: a.season_id,
                    description: a.description,
                    aired_date: a.aired_date,
                    end_date: a.end_date,
                    created_at: a.created_at,
                    updated_at: a.updated_at,
                })
                .collect();
            tracing::info!("Listed {} anime for work_id {}", responses.len(), work_id);
            (StatusCode::OK, Json(json!({ "series": responses })))
        }
        Err(e) => {
            tracing::error!("Failed to list anime: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to list anime: {:?}", e),
                    "series": []
                })),
            )
        }
    }
}

// ============ SubtitleGroup Handlers ============

/// Create a new subtitle group
#[utoipa::path(
    post,
    path = "/api/core/subtitle-groups",
    tag = "SubtitleGroups",
    request_body = SubtitleGroupRequest,
    responses(
        (status = 201, description = "Created successfully", body = SubtitleGroupResponse),
        (status = 500, description = "Database error")
    )
)]
pub async fn create_subtitle_group(
    State(state): State<AppState>,
    Json(payload): Json<SubtitleGroupRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.subtitle_group.create(payload.group_name).await {
        Ok(group) => {
            tracing::info!("Created subtitle group: {}", group.group_id);
            let response = SubtitleGroupResponse {
                group_id: group.group_id,
                group_name: group.group_name,
                created_at: group.created_at,
            };
            (StatusCode::CREATED, Json(json!(response)))
        }
        Err(e) => {
            tracing::error!("Failed to create subtitle group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to create subtitle group: {:?}", e)
                })),
            )
        }
    }
}

/// List all subtitle groups
#[utoipa::path(
    get,
    path = "/api/core/subtitle-groups",
    tag = "SubtitleGroups",
    responses(
        (status = 200, description = "Success"),
        (status = 500, description = "Database error")
    )
)]
pub async fn list_subtitle_groups(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.subtitle_group.find_all().await {
        Ok(group_list) => {
            let responses: Vec<SubtitleGroupResponse> = group_list
                .into_iter()
                .map(|g| SubtitleGroupResponse {
                    group_id: g.group_id,
                    group_name: g.group_name,
                    created_at: g.created_at,
                })
                .collect();
            tracing::info!("Listed {} subtitle groups", responses.len());
            (StatusCode::OK, Json(json!({ "groups": responses })))
        }
        Err(e) => {
            tracing::error!("Failed to list subtitle groups: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to list subtitle groups: {:?}", e),
                    "groups": []
                })),
            )
        }
    }
}

/// Delete subtitle group by ID
#[utoipa::path(
    delete,
    path = "/api/core/subtitle-groups/{group_id}",
    tag = "SubtitleGroups",
    params(("group_id" = i32, Path, description = "Subtitle group ID")),
    responses(
        (status = 200, description = "Deleted successfully"),
        (status = 404, description = "Not found"),
        (status = 500, description = "Database error")
    )
)]
pub async fn delete_subtitle_group(
    State(state): State<AppState>,
    Path(group_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.subtitle_group.delete(group_id).await {
        Ok(true) => {
            tracing::info!("Deleted subtitle group: {}", group_id);
            (StatusCode::OK, Json(json!({ "deleted": true })))
        }
        Ok(false) => {
            tracing::warn!("Subtitle group not found for deletion: {}", group_id);
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "not_found",
                    "message": format!("Subtitle group {} not found", group_id)
                })),
            )
        }
        Err(e) => {
            tracing::error!("Failed to delete subtitle group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to delete subtitle group: {:?}", e)
                })),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_work_title_is_trimmed_and_must_not_be_blank() {
        assert_eq!(
            normalized_work_title("  藥師少女的獨語 "),
            Some("藥師少女的獨語".to_string())
        );
        assert_eq!(normalized_work_title("   "), None);
        assert_eq!(normalized_work_title(&"字".repeat(256)), None);
        assert!(normalized_work_title(&"字".repeat(255)).is_some());
    }
    use crate::db::repository::anime::mock::MockAnimeRepository;
    use crate::db::repository::anime::AnimeRepository;
    use crate::db::repository::anime_work::mock::MockAnimeWorkRepository;
    use crate::db::repository::anime_work::AnimeWorkRepository;
    use crate::db::repository::season::mock::MockSeasonRepository;
    use crate::db::repository::season::SeasonRepository;
    use crate::db::repository::subtitle_group::mock::MockSubtitleGroupRepository;
    use crate::db::repository::subtitle_group::SubtitleGroupRepository;
    use crate::models::{Anime, AnimeWork, Season, SubtitleGroup};
    use chrono::Utc;

    // ============ AnimeWork Repository Tests ============
    #[tokio::test]
    async fn test_anime_work_repository_create() {
        let repo = MockAnimeWorkRepository::new();
        let work = repo.create("Test Anime".to_string()).await.unwrap();
        assert_eq!(work.title, "Test Anime");

        let ops = repo.get_operations();
        assert!(ops.contains(&"create:Test Anime".to_string()));
    }

    #[tokio::test]
    async fn test_anime_work_repository_find_by_id() {
        let work = AnimeWork {
            work_id: 1,
            title: "Test Anime".to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            series_namespace: None,
            series_external_id: None,
            title_source: "parser".to_string(),
            description: None,
            premiered: None,
            is_active: true,
            soft_deleted_at: None,
        };
        let repo = MockAnimeWorkRepository::with_data(vec![work]);

        let found = repo.find_by_id(1).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Test Anime");

        let not_found = repo.find_by_id(999).await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_anime_work_repository_find_all() {
        let work1 = AnimeWork {
            work_id: 1,
            title: "Anime 1".to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            series_namespace: None,
            series_external_id: None,
            title_source: "parser".to_string(),
            description: None,
            premiered: None,
            is_active: true,
            soft_deleted_at: None,
        };
        let work2 = AnimeWork {
            work_id: 2,
            title: "Anime 2".to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            series_namespace: None,
            series_external_id: None,
            title_source: "parser".to_string(),
            description: None,
            premiered: None,
            is_active: true,
            soft_deleted_at: None,
        };
        let repo = MockAnimeWorkRepository::with_data(vec![work1, work2]);

        let all = repo.find_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_anime_work_repository_delete() {
        let work = AnimeWork {
            work_id: 1,
            title: "To Delete".to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            series_namespace: None,
            series_external_id: None,
            title_source: "parser".to_string(),
            description: None,
            premiered: None,
            is_active: true,
            soft_deleted_at: None,
        };
        let repo = MockAnimeWorkRepository::with_data(vec![work]);

        let deleted = repo.delete(1).await.unwrap();
        assert!(deleted);

        let not_deleted = repo.delete(999).await.unwrap();
        assert!(!not_deleted);
    }

    // ============ Season Repository Tests ============
    #[tokio::test]
    async fn test_season_repository_create() {
        let repo = MockSeasonRepository::new();
        let season = repo.create(2024, "Winter".to_string()).await.unwrap();
        assert_eq!(season.year, 2024);
        assert_eq!(season.season, "Winter");
    }

    #[tokio::test]
    async fn test_season_repository_find_all() {
        let season = Season {
            season_id: 1,
            year: 2024,
            season: "Spring".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let repo = MockSeasonRepository::with_data(vec![season]);

        let all = repo.find_all().await.unwrap();
        assert_eq!(all.len(), 1);
    }

    // ============ Anime Repository Tests ============
    #[tokio::test]
    async fn test_anime_repository_create() {
        let repo = MockAnimeRepository::new();
        let params = CreateAnimeParams {
            work_id: 1,
            series_no: 1,
            season_id: 1,
            description: Some("Test".to_string()),
            aired_date: None,
            end_date: None,
        };
        let anime = repo.create(params).await.unwrap();
        assert_eq!(anime.work_id, 1);
        assert_eq!(anime.series_no, 1);
    }

    #[tokio::test]
    async fn test_anime_repository_find_by_work_id() {
        let anime = Anime {
            anime_id: 1,
            work_id: 1,
            series_no: 1,
            season_id: 1,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            title: None,
            original_title: None,
        };
        let repo = MockAnimeRepository::with_data(vec![anime]);

        let found = repo.find_by_work_id(1).await.unwrap();
        assert_eq!(found.len(), 1);
    }

    // ============ SubtitleGroup Repository Tests ============
    #[tokio::test]
    async fn test_subtitle_group_repository_create() {
        let repo = MockSubtitleGroupRepository::new();
        let group = repo.create("Test Group".to_string()).await.unwrap();
        assert_eq!(group.group_name, "Test Group");
    }

    #[tokio::test]
    async fn test_subtitle_group_repository_find_all() {
        let group = SubtitleGroup {
            group_id: 1,
            group_name: "Fansub".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let repo = MockSubtitleGroupRepository::with_data(vec![group]);

        let all = repo.find_all().await.unwrap();
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn test_subtitle_group_repository_delete() {
        let group = SubtitleGroup {
            group_id: 1,
            group_name: "To Delete".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let repo = MockSubtitleGroupRepository::with_data(vec![group]);

        let deleted = repo.delete(1).await.unwrap();
        assert!(deleted);

        let not_deleted = repo.delete(999).await.unwrap();
        assert!(!not_deleted);
    }
}

// ============ Background Metadata Fetch ============

/// 依作品各季的身分在背景補抓封面；沒有身分的作品不抓（不再用作品名模糊搜尋）。
pub async fn fetch_and_store_covers(db: crate::db::DbPool, work_id: i32) {
    let lookup = crate::services::identity::HttpMetadataLookup::new(db.clone());
    if let Err(e) = crate::services::identity::fetch_covers_for_work(&db, &lookup, work_id).await {
        tracing::warn!("Cover fetch for work {} failed: {}", work_id, e);
    }
}
