use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde_json::json;
use serde::{Deserialize, Serialize};
use diesel::prelude::*;

use crate::state::AppState;
use crate::models::{
    NewAnime, NewSeason, NewAnimeSeries, NewSubtitleGroup, NewAnimeLink,
    Anime, Season, AnimeSeries, SubtitleGroup, AnimeLink,
};
use crate::schema::{animes, seasons, anime_series, subtitle_groups, anime_links};

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
    pub season: String,  // 冬/春/夏/秋
    pub year: i32,
    pub series_no: i32,
    pub links: Vec<FetchedLinkPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherResultsPayload {
    pub animes: Vec<FetchedAnimePayload>,
    pub fetcher_source: String,  // e.g., "mikanani"
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
        "Received fetcher results from {}: {} animes",
        payload.fetcher_source,
        payload.animes.len()
    );

    let mut animes_created = 0;
    let mut links_created = 0;
    let mut errors = Vec::new();

    match state.db.get() {
        Ok(mut conn) => {
            for fetched_anime in payload.animes {
                // Create or get anime
                match create_or_get_anime(&mut conn, &fetched_anime.title) {
                    Ok(anime) => {
                        animes_created += 1;

                        // Create or get season
                        match create_or_get_season(&mut conn, fetched_anime.year, &fetched_anime.season) {
                            Ok(season) => {
                                // Create or get anime series
                                match create_or_get_series(
                                    &mut conn,
                                    anime.anime_id,
                                    fetched_anime.series_no,
                                    season.season_id,
                                    &fetched_anime.description,
                                ) {
                                    Ok(series) => {
                                        // Process each link in the anime
                                        for fetched_link in &fetched_anime.links {
                                            // Create or get subtitle group
                                            match create_or_get_subtitle_group(&mut conn, &fetched_link.subtitle_group) {
                                                Ok(group) => {
                                                    // Create anime link
                                                    match create_anime_link(
                                                        &mut conn,
                                                        series.series_id,
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
                                                                fetched_anime.title, e
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
                                            fetched_anime.title, e
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
                                    fetched_anime.year, fetched_anime.season, e
                                );
                                errors.push(format!(
                                    "Season creation failed: {}",
                                    e
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get/create anime '{}': {}",
                            fetched_anime.title, e
                        );
                        errors.push(format!("Anime creation failed for {}: {}", fetched_anime.title, e));
                    }
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
                animes_created, links_created
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

/// Create or get an anime by title
fn create_or_get_anime(conn: &mut PgConnection, title: &str) -> Result<Anime, String> {
    // Try to find existing anime
    match animes::table
        .filter(animes::title.eq(title))
        .first::<Anime>(conn)
    {
        Ok(anime) => {
            tracing::debug!("Found existing anime: {}", title);
            Ok(anime)
        }
        Err(diesel::NotFound) => {
            // Create new anime
            let now = Utc::now().naive_utc();
            let new_anime = NewAnime {
                title: title.to_string(),
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

/// Create or get a season by year and season name
fn create_or_get_season(
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

/// Create or get an anime series
fn create_or_get_series(
    conn: &mut PgConnection,
    anime_id: i32,
    series_no: i32,
    season_id: i32,
    description: &str,
) -> Result<AnimeSeries, String> {
    // Try to find existing series
    match anime_series::table
        .filter(anime_series::anime_id.eq(anime_id))
        .filter(anime_series::series_no.eq(series_no))
        .filter(anime_series::season_id.eq(season_id))
        .first::<AnimeSeries>(conn)
    {
        Ok(series) => {
            tracing::debug!(
                "Found existing anime series: anime_id={}, series_no={}",
                anime_id,
                series_no
            );
            Ok(series)
        }
        Err(diesel::NotFound) => {
            // Create new series
            let now = Utc::now().naive_utc();
            let new_series = NewAnimeSeries {
                anime_id,
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

            diesel::insert_into(anime_series::table)
                .values(&new_series)
                .get_result::<AnimeSeries>(conn)
                .map_err(|e| format!("Failed to create anime series: {}", e))
        }
        Err(e) => Err(format!("Failed to query anime series: {}", e)),
    }
}

/// Create or get a subtitle group
fn create_or_get_subtitle_group(conn: &mut PgConnection, group_name: &str) -> Result<SubtitleGroup, String> {
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
    series_id: i32,
    group_id: i32,
    fetched_link: &FetchedLinkPayload,
) -> Result<AnimeLink, String> {
    let now = Utc::now().naive_utc();
    let new_link = NewAnimeLink {
        series_id,
        group_id,
        episode_no: fetched_link.episode_no,
        title: Some(fetched_link.title.clone()),
        url: fetched_link.url.clone(),
        source_hash: fetched_link.source_hash.clone(),
        filtered_flag: false,
        created_at: now,
    };

    diesel::insert_into(anime_links::table)
        .values(&new_link)
        .get_result::<AnimeLink>(conn)
        .map_err(|e| format!("Failed to create anime link: {}", e))
}
