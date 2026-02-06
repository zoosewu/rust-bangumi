use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::json;

use crate::db::CreateAnimeSeriesParams;
use crate::dto::{
    AnimeRequest, AnimeResponse, AnimeSeriesRequest, AnimeSeriesResponse, SeasonRequest,
    SeasonResponse, SubtitleGroupRequest, SubtitleGroupResponse,
};
use crate::state::AppState;

// ============ Anime Handlers ============

/// Create a new anime
pub async fn create_anime(
    State(state): State<AppState>,
    Json(payload): Json<AnimeRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.anime.create(payload.title).await {
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

/// List all animes
pub async fn list_anime(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.anime.find_all().await {
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
            tracing::error!("Failed to list animes: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to list animes: {:?}", e),
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
    match state.repos.anime.find_by_id(anime_id).await {
        Ok(Some(anime)) => {
            let response = AnimeResponse {
                anime_id: anime.anime_id,
                title: anime.title,
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

/// Delete anime by ID
pub async fn delete_anime(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.repos.anime.delete(anime_id).await {
        Ok(true) => {
            tracing::info!("Deleted anime: {}", anime_id);
            (StatusCode::OK, Json(json!({ "deleted": true })))
        }
        Ok(false) => {
            tracing::warn!("Anime not found for deletion: {}", anime_id);
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "not_found",
                    "message": format!("Anime {} not found", anime_id)
                })),
            )
        }
        Err(e) => {
            tracing::error!("Failed to delete anime: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to delete anime: {:?}", e)
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
    match state
        .repos
        .season
        .create(payload.year, payload.season)
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

// ============ AnimeSeries Handlers ============

/// Create a new anime series
pub async fn create_anime_series(
    State(state): State<AppState>,
    Json(payload): Json<AnimeSeriesRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let params = CreateAnimeSeriesParams {
        anime_id: payload.anime_id,
        series_no: payload.series_no,
        season_id: payload.season_id,
        description: payload.description,
        aired_date: payload.aired_date,
        end_date: payload.end_date,
    };

    match state.repos.anime_series.create(params).await {
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
            tracing::error!("Failed to create anime series: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to create anime series: {:?}", e)
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
    match state.repos.anime_series.find_by_id(series_id).await {
        Ok(Some(series)) => {
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
        Ok(None) => {
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
            tracing::error!("Failed to get anime series: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to get anime series: {:?}", e)
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
    match state.repos.anime_series.find_by_anime_id(anime_id).await {
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
            tracing::info!(
                "Listed {} anime series for anime {}",
                responses.len(),
                anime_id
            );
            (StatusCode::OK, Json(json!({ "series": responses })))
        }
        Err(e) => {
            tracing::error!("Failed to list anime series: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "database_error",
                    "message": format!("Failed to list anime series: {:?}", e),
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
    use crate::db::repository::anime::mock::MockAnimeRepository;
    use crate::db::repository::anime::AnimeRepository;
    use crate::db::repository::anime_series::mock::MockAnimeSeriesRepository;
    use crate::db::repository::anime_series::AnimeSeriesRepository;
    use crate::db::repository::season::mock::MockSeasonRepository;
    use crate::db::repository::season::SeasonRepository;
    use crate::db::repository::subtitle_group::mock::MockSubtitleGroupRepository;
    use crate::db::repository::subtitle_group::SubtitleGroupRepository;
    use crate::models::{Anime, AnimeSeries, Season, SubtitleGroup};
    use chrono::Utc;

    // ============ Anime Repository Tests ============
    #[tokio::test]
    async fn test_anime_repository_create() {
        let repo = MockAnimeRepository::new();
        let anime = repo.create("Test Anime".to_string()).await.unwrap();
        assert_eq!(anime.title, "Test Anime");

        let ops = repo.get_operations();
        assert!(ops.contains(&"create:Test Anime".to_string()));
    }

    #[tokio::test]
    async fn test_anime_repository_find_by_id() {
        let anime = Anime {
            anime_id: 1,
            title: "Test Anime".to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let repo = MockAnimeRepository::with_data(vec![anime]);

        let found = repo.find_by_id(1).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Test Anime");

        let not_found = repo.find_by_id(999).await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_anime_repository_find_all() {
        let anime1 = Anime {
            anime_id: 1,
            title: "Anime 1".to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let anime2 = Anime {
            anime_id: 2,
            title: "Anime 2".to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let repo = MockAnimeRepository::with_data(vec![anime1, anime2]);

        let all = repo.find_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_anime_repository_delete() {
        let anime = Anime {
            anime_id: 1,
            title: "To Delete".to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let repo = MockAnimeRepository::with_data(vec![anime]);

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

    // ============ AnimeSeries Repository Tests ============
    #[tokio::test]
    async fn test_anime_series_repository_create() {
        let repo = MockAnimeSeriesRepository::new();
        let params = CreateAnimeSeriesParams {
            anime_id: 1,
            series_no: 1,
            season_id: 1,
            description: Some("Test".to_string()),
            aired_date: None,
            end_date: None,
        };
        let series = repo.create(params).await.unwrap();
        assert_eq!(series.anime_id, 1);
        assert_eq!(series.series_no, 1);
    }

    #[tokio::test]
    async fn test_anime_series_repository_find_by_anime_id() {
        let series = AnimeSeries {
            series_id: 1,
            anime_id: 1,
            series_no: 1,
            season_id: 1,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let repo = MockAnimeSeriesRepository::with_data(vec![series]);

        let found = repo.find_by_anime_id(1).await.unwrap();
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
