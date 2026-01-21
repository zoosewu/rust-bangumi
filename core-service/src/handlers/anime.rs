use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde_json::json;
use diesel::prelude::*;

use crate::state::AppState;
use crate::dto::{
    AnimeRequest, AnimeResponse, SeasonRequest, SeasonResponse, AnimeSeriesRequest,
    AnimeSeriesResponse, SubtitleGroupRequest, SubtitleGroupResponse, ErrorResponse,
};
use crate::models::{NewAnime, NewSeason, NewAnimeSeries, NewSubtitleGroup};
use crate::schema::{animes, seasons, anime_series, subtitle_groups};

// ============ Anime Handlers ============

/// Create a new anime
pub async fn create_anime(
    State(state): State<AppState>,
    Json(payload): Json<AnimeRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let now = Utc::now().naive_utc();
    let new_anime = NewAnime {
        title: payload.title,
        created_at: now,
        updated_at: now,
    };

    match state.db.get() {
        Ok(mut conn) => {
            match diesel::insert_into(animes::table)
                .values(&new_anime)
                .get_result::<crate::models::Anime>(&mut conn)
            {
                Ok(anime) => {
                    tracing::info!("Created anime: {}", anime.anime_id);
                    let response = AnimeResponse {
                        anime_id: anime.anime_id,
                        title: anime.title,
                        created_at: anime.created_at,
                        updated_at: anime.updated_at,
                    };
                    (StatusCode::CREATED, Json(json!(response)))
                }
                Err(e) => {
                    tracing::error!("Failed to create anime: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to create anime: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}

/// List all animes
pub async fn list_anime(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match animes::table.load::<crate::models::Anime>(&mut conn) {
                Ok(anime_list) => {
                    let responses: Vec<AnimeResponse> = anime_list
                        .into_iter()
                        .map(|a| AnimeResponse {
                            anime_id: a.anime_id,
                            title: a.title,
                            created_at: a.created_at,
                            updated_at: a.updated_at,
                        })
                        .collect();
                    tracing::info!("Listed {} animes", responses.len());
                    (StatusCode::OK, Json(json!({ "animes": responses })))
                }
                Err(e) => {
                    tracing::error!("Failed to list animes: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to list animes: {}", e),
                            "animes": []
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e),
                    "animes": []
                })),
            )
        }
    }
}

/// Get anime by ID
pub async fn get_anime(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match animes::table
                .find(anime_id)
                .first::<crate::models::Anime>(&mut conn)
            {
                Ok(anime) => {
                    let response = AnimeResponse {
                        anime_id: anime.anime_id,
                        title: anime.title,
                        created_at: anime.created_at,
                        updated_at: anime.updated_at,
                    };
                    tracing::info!("Retrieved anime: {}", anime_id);
                    (StatusCode::OK, Json(json!(response)))
                }
                Err(diesel::NotFound) => {
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
                    tracing::error!("Failed to get anime: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to get anime: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}

/// Delete anime by ID
pub async fn delete_anime(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match diesel::delete(animes::table.find(anime_id)).execute(&mut conn) {
                Ok(deleted_count) => {
                    if deleted_count > 0 {
                        tracing::info!("Deleted anime: {}", anime_id);
                        (StatusCode::OK, Json(json!({ "deleted": true })))
                    } else {
                        tracing::warn!("Anime not found for deletion: {}", anime_id);
                        (
                            StatusCode::NOT_FOUND,
                            Json(json!({
                                "error": "not_found",
                                "message": format!("Anime {} not found", anime_id)
                            })),
                        )
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to delete anime: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to delete anime: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}

// ============ Season Handlers ============

/// Create a new season
pub async fn create_season(
    State(state): State<AppState>,
    Json(payload): Json<SeasonRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let now = Utc::now().naive_utc();
    let new_season = NewSeason {
        year: payload.year,
        season: payload.season,
        created_at: now,
    };

    match state.db.get() {
        Ok(mut conn) => {
            match diesel::insert_into(seasons::table)
                .values(&new_season)
                .get_result::<crate::models::Season>(&mut conn)
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
                    tracing::error!("Failed to create season: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to create season: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}

/// List all seasons
pub async fn list_seasons(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match seasons::table.load::<crate::models::Season>(&mut conn) {
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
                    tracing::error!("Failed to list seasons: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to list seasons: {}", e),
                            "seasons": []
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e),
                    "seasons": []
                })),
            )
        }
    }
}

// ============ AnimeSeries Handlers ============

/// Create a new anime series
pub async fn create_anime_series(
    State(state): State<AppState>,
    Json(payload): Json<AnimeSeriesRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let now = Utc::now().naive_utc();
    let new_series = NewAnimeSeries {
        anime_id: payload.anime_id,
        series_no: payload.series_no,
        season_id: payload.season_id,
        description: payload.description,
        aired_date: payload.aired_date,
        end_date: payload.end_date,
        created_at: now,
        updated_at: now,
    };

    match state.db.get() {
        Ok(mut conn) => {
            match diesel::insert_into(anime_series::table)
                .values(&new_series)
                .get_result::<crate::models::AnimeSeries>(&mut conn)
            {
                Ok(series) => {
                    tracing::info!("Created anime series: {}", series.series_id);
                    let response = AnimeSeriesResponse {
                        series_id: series.series_id,
                        anime_id: series.anime_id,
                        series_no: series.series_no,
                        season_id: series.season_id,
                        description: series.description,
                        aired_date: series.aired_date,
                        end_date: series.end_date,
                        created_at: series.created_at,
                        updated_at: series.updated_at,
                    };
                    (StatusCode::CREATED, Json(json!(response)))
                }
                Err(e) => {
                    tracing::error!("Failed to create anime series: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to create anime series: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}

/// Get anime series by ID
pub async fn get_anime_series(
    State(state): State<AppState>,
    Path(series_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match anime_series::table
                .find(series_id)
                .first::<crate::models::AnimeSeries>(&mut conn)
            {
                Ok(series) => {
                    let response = AnimeSeriesResponse {
                        series_id: series.series_id,
                        anime_id: series.anime_id,
                        series_no: series.series_no,
                        season_id: series.season_id,
                        description: series.description,
                        aired_date: series.aired_date,
                        end_date: series.end_date,
                        created_at: series.created_at,
                        updated_at: series.updated_at,
                    };
                    tracing::info!("Retrieved anime series: {}", series_id);
                    (StatusCode::OK, Json(json!(response)))
                }
                Err(diesel::NotFound) => {
                    tracing::warn!("Anime series not found: {}", series_id);
                    (
                        StatusCode::NOT_FOUND,
                        Json(json!({
                            "error": "not_found",
                            "message": format!("Anime series {} not found", series_id)
                        })),
                    )
                }
                Err(e) => {
                    tracing::error!("Failed to get anime series: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to get anime series: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}

/// List anime series by anime ID
pub async fn list_anime_series(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match anime_series::table
                .filter(anime_series::anime_id.eq(anime_id))
                .load::<crate::models::AnimeSeries>(&mut conn)
            {
                Ok(series_list) => {
                    let responses: Vec<AnimeSeriesResponse> = series_list
                        .into_iter()
                        .map(|s| AnimeSeriesResponse {
                            series_id: s.series_id,
                            anime_id: s.anime_id,
                            series_no: s.series_no,
                            season_id: s.season_id,
                            description: s.description,
                            aired_date: s.aired_date,
                            end_date: s.end_date,
                            created_at: s.created_at,
                            updated_at: s.updated_at,
                        })
                        .collect();
                    tracing::info!("Listed {} anime series for anime {}", responses.len(), anime_id);
                    (StatusCode::OK, Json(json!({ "series": responses })))
                }
                Err(e) => {
                    tracing::error!("Failed to list anime series: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to list anime series: {}", e),
                            "series": []
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e),
                    "series": []
                })),
            )
        }
    }
}

// ============ SubtitleGroup Handlers ============

/// Create a new subtitle group
pub async fn create_subtitle_group(
    State(state): State<AppState>,
    Json(payload): Json<SubtitleGroupRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let now = Utc::now().naive_utc();
    let new_group = NewSubtitleGroup {
        group_name: payload.group_name,
        created_at: now,
    };

    match state.db.get() {
        Ok(mut conn) => {
            match diesel::insert_into(subtitle_groups::table)
                .values(&new_group)
                .get_result::<crate::models::SubtitleGroup>(&mut conn)
            {
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
                    tracing::error!("Failed to create subtitle group: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to create subtitle group: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}

/// List all subtitle groups
pub async fn list_subtitle_groups(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match subtitle_groups::table.load::<crate::models::SubtitleGroup>(&mut conn) {
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
                    tracing::error!("Failed to list subtitle groups: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to list subtitle groups: {}", e),
                            "groups": []
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e),
                    "groups": []
                })),
            )
        }
    }
}

/// Delete subtitle group by ID
pub async fn delete_subtitle_group(
    State(state): State<AppState>,
    Path(group_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match diesel::delete(subtitle_groups::table.find(group_id)).execute(&mut conn) {
                Ok(deleted_count) => {
                    if deleted_count > 0 {
                        tracing::info!("Deleted subtitle group: {}", group_id);
                        (StatusCode::OK, Json(json!({ "deleted": true })))
                    } else {
                        tracing::warn!("Subtitle group not found for deletion: {}", group_id);
                        (
                            StatusCode::NOT_FOUND,
                            Json(json!({
                                "error": "not_found",
                                "message": format!("Subtitle group {} not found", group_id)
                            })),
                        )
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to delete subtitle group: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to delete subtitle group: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}
