// Database query operations layer
// Full implementation of CRUD operations using Diesel ORM

use crate::db::DbPool;
use crate::models::*;
use crate::schema::*;
use diesel::prelude::*;
use chrono::{Utc, NaiveDate};

// ============ Anime CRUD ============

pub fn create_anime(pool: &DbPool, title: String) -> Result<Anime, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let now = Utc::now().naive_utc();
    let new_anime = NewAnime {
        title,
        created_at: now,
        updated_at: now,
    };

    diesel::insert_into(animes::table)
        .values(&new_anime)
        .get_result::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to create anime: {}", e))
}

pub fn get_anime_by_id(pool: &DbPool, anime_id: i32) -> Result<Anime, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    animes::table
        .find(anime_id)
        .first::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to get anime: {}", e))
}

pub fn get_anime_by_title(pool: &DbPool, title: &str) -> Result<Anime, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    animes::table
        .filter(animes::title.eq(title))
        .first::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to get anime by title: {}", e))
}

pub fn get_all_animes(pool: &DbPool) -> Result<Vec<Anime>, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    animes::table
        .load::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to load animes: {}", e))
}

pub fn update_anime(pool: &DbPool, anime_id: i32, title: String) -> Result<Anime, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    diesel::update(animes::table.find(anime_id))
        .set((
            animes::title.eq(title),
            animes::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to update anime: {}", e))
}

pub fn delete_anime(pool: &DbPool, anime_id: i32) -> Result<usize, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    diesel::delete(animes::table.find(anime_id))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to delete anime: {}", e))
}

// ============ Season CRUD ============

pub fn create_season(pool: &DbPool, year: i32, season: String) -> Result<Season, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let new_season = NewSeason {
        year,
        season,
        created_at: Utc::now().naive_utc(),
    };

    diesel::insert_into(seasons::table)
        .values(&new_season)
        .get_result::<Season>(&mut conn)
        .map_err(|e| format!("Failed to create season: {}", e))
}

pub fn get_season_by_id(pool: &DbPool, season_id: i32) -> Result<Season, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    seasons::table
        .find(season_id)
        .first::<Season>(&mut conn)
        .map_err(|e| format!("Failed to get season: {}", e))
}

pub fn get_or_create_season(
    pool: &DbPool,
    year: i32,
    season: String,
) -> Result<Season, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Try to find existing season
    let existing = seasons::table
        .filter(seasons::year.eq(year))
        .filter(seasons::season.eq(&season))
        .first::<Season>(&mut conn)
        .optional()
        .map_err(|e| format!("Failed to query season: {}", e))?;

    if let Some(s) = existing {
        Ok(s)
    } else {
        let new_season = NewSeason {
            year,
            season,
            created_at: Utc::now().naive_utc(),
        };
        diesel::insert_into(seasons::table)
            .values(&new_season)
            .get_result::<Season>(&mut conn)
            .map_err(|e| format!("Failed to create season: {}", e))
    }
}

pub fn get_all_seasons(pool: &DbPool) -> Result<Vec<Season>, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    seasons::table
        .load::<Season>(&mut conn)
        .map_err(|e| format!("Failed to load seasons: {}", e))
}

// ============ AnimeSeries CRUD ============

pub fn create_anime_series(
    pool: &DbPool,
    anime_id: i32,
    series_no: i32,
    season_id: i32,
    description: Option<String>,
    aired_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<AnimeSeries, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let now = Utc::now().naive_utc();
    let new_series = NewAnimeSeries {
        anime_id,
        series_no,
        season_id,
        description,
        aired_date,
        end_date,
        created_at: now,
        updated_at: now,
    };

    diesel::insert_into(anime_series::table)
        .values(&new_series)
        .get_result::<AnimeSeries>(&mut conn)
        .map_err(|e| format!("Failed to create anime series: {}", e))
}

pub fn get_anime_series_by_id(pool: &DbPool, series_id: i32) -> Result<AnimeSeries, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    anime_series::table
        .find(series_id)
        .first::<AnimeSeries>(&mut conn)
        .map_err(|e| format!("Failed to get anime series: {}", e))
}

pub fn get_anime_series_by_anime(pool: &DbPool, anime_id: i32) -> Result<Vec<AnimeSeries>, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    anime_series::table
        .filter(anime_series::anime_id.eq(anime_id))
        .load::<AnimeSeries>(&mut conn)
        .map_err(|e| format!("Failed to get anime series by anime: {}", e))
}

pub fn get_anime_series_by_season(pool: &DbPool, season_id: i32) -> Result<Vec<AnimeSeries>, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    anime_series::table
        .filter(anime_series::season_id.eq(season_id))
        .load::<AnimeSeries>(&mut conn)
        .map_err(|e| format!("Failed to get anime series by season: {}", e))
}

pub fn update_anime_series(
    pool: &DbPool,
    series_id: i32,
    anime_id: i32,
    series_no: i32,
    season_id: i32,
    description: Option<String>,
    aired_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<AnimeSeries, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    diesel::update(anime_series::table.find(series_id))
        .set((
            anime_series::anime_id.eq(anime_id),
            anime_series::series_no.eq(series_no),
            anime_series::season_id.eq(season_id),
            anime_series::description.eq(description),
            anime_series::aired_date.eq(aired_date),
            anime_series::end_date.eq(end_date),
            anime_series::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<AnimeSeries>(&mut conn)
        .map_err(|e| format!("Failed to update anime series: {}", e))
}

pub fn delete_anime_series(pool: &DbPool, series_id: i32) -> Result<usize, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    diesel::delete(anime_series::table.find(series_id))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to delete anime series: {}", e))
}

// ============ SubtitleGroups CRUD ============

pub fn create_subtitle_group(
    pool: &DbPool,
    group_name: String,
) -> Result<SubtitleGroup, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let new_group = NewSubtitleGroup {
        group_name,
        created_at: Utc::now().naive_utc(),
    };

    diesel::insert_into(subtitle_groups::table)
        .values(&new_group)
        .get_result::<SubtitleGroup>(&mut conn)
        .map_err(|e| format!("Failed to create subtitle group: {}", e))
}

pub fn get_subtitle_group_by_id(pool: &DbPool, group_id: i32) -> Result<SubtitleGroup, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    subtitle_groups::table
        .find(group_id)
        .first::<SubtitleGroup>(&mut conn)
        .map_err(|e| format!("Failed to get subtitle group: {}", e))
}

pub fn get_or_create_subtitle_group(
    pool: &DbPool,
    group_name: String,
) -> Result<SubtitleGroup, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Try to find existing subtitle group
    let existing = subtitle_groups::table
        .filter(subtitle_groups::group_name.eq(&group_name))
        .first::<SubtitleGroup>(&mut conn)
        .optional()
        .map_err(|e| format!("Failed to query subtitle group: {}", e))?;

    if let Some(g) = existing {
        Ok(g)
    } else {
        let new_group = NewSubtitleGroup {
            group_name,
            created_at: Utc::now().naive_utc(),
        };
        diesel::insert_into(subtitle_groups::table)
            .values(&new_group)
            .get_result::<SubtitleGroup>(&mut conn)
            .map_err(|e| format!("Failed to create subtitle group: {}", e))
    }
}

pub fn get_all_subtitle_groups(pool: &DbPool) -> Result<Vec<SubtitleGroup>, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    subtitle_groups::table
        .load::<SubtitleGroup>(&mut conn)
        .map_err(|e| format!("Failed to load subtitle groups: {}", e))
}

pub fn delete_subtitle_group(pool: &DbPool, group_id: i32) -> Result<usize, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    diesel::delete(subtitle_groups::table.find(group_id))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to delete subtitle group: {}", e))
}

// ============ AnimeLink CRUD ============

pub fn create_anime_link(
    pool: &DbPool,
    series_id: i32,
    group_id: i32,
    episode_no: i32,
    title: Option<String>,
    url: String,
    source_hash: String,
    filtered_flag: bool,
) -> Result<AnimeLink, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let now = Utc::now().naive_utc();
    let new_link = NewAnimeLink {
        series_id,
        group_id,
        episode_no,
        title,
        url,
        source_hash,
        filtered_flag,
        created_at: now,
        updated_at: now,
    };

    diesel::insert_into(anime_links::table)
        .values(&new_link)
        .get_result::<AnimeLink>(&mut conn)
        .map_err(|e| format!("Failed to create anime link: {}", e))
}

pub fn get_anime_links_by_series(
    pool: &DbPool,
    series_id: i32,
) -> Result<Vec<AnimeLink>, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    anime_links::table
        .filter(anime_links::series_id.eq(series_id))
        .load::<AnimeLink>(&mut conn)
        .map_err(|e| format!("Failed to get anime links: {}", e))
}

// ============ FilterRules CRUD ============

pub fn get_filter_rules(
    pool: &DbPool,
    series_id: i32,
    group_id: i32,
) -> Result<Vec<FilterRule>, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    filter_rules::table
        .filter(filter_rules::series_id.eq(series_id))
        .filter(filter_rules::group_id.eq(group_id))
        .load::<FilterRule>(&mut conn)
        .map_err(|e| format!("Failed to get filter rules: {}", e))
}

pub fn create_filter_rule(
    pool: &DbPool,
    series_id: i32,
    group_id: i32,
    rule_order: i32,
    rule_type: String,
    regex_pattern: String,
) -> Result<FilterRule, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let new_rule = NewFilterRule {
        series_id,
        group_id,
        rule_order,
        rule_type,
        regex_pattern,
        created_at: Utc::now().naive_utc(),
    };

    diesel::insert_into(filter_rules::table)
        .values(&new_rule)
        .get_result::<FilterRule>(&mut conn)
        .map_err(|e| format!("Failed to create filter rule: {}", e))
}

pub fn delete_filter_rule(pool: &DbPool, rule_id: i32) -> Result<usize, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    diesel::delete(filter_rules::table.find(rule_id))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to delete filter rule: {}", e))
}

// ============ Download CRUD ============

pub fn create_download(
    pool: &DbPool,
    link_id: i32,
    downloader_type: String,
) -> Result<Download, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let now = Utc::now().naive_utc();
    let new_download = NewDownload {
        link_id,
        downloader_type,
        status: "pending".to_string(),
        created_at: now,
        updated_at: now,
    };

    diesel::insert_into(downloads::table)
        .values(&new_download)
        .get_result::<Download>(&mut conn)
        .map_err(|e| format!("Failed to create download: {}", e))
}

pub fn get_download(pool: &DbPool, download_id: i32) -> Result<Download, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    downloads::table
        .find(download_id)
        .first::<Download>(&mut conn)
        .map_err(|e| format!("Failed to get download: {}", e))
}

pub fn update_download_progress(
    pool: &DbPool,
    download_id: i32,
    status: &str,
    progress: Option<f32>,
    downloaded_bytes: Option<i64>,
    total_bytes: Option<i64>,
) -> Result<Download, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    diesel::update(downloads::table.find(download_id))
        .set((
            downloads::status.eq(status),
            downloads::progress.eq(progress),
            downloads::downloaded_bytes.eq(downloaded_bytes),
            downloads::total_bytes.eq(total_bytes),
            downloads::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<Download>(&mut conn)
        .map_err(|e| format!("Failed to update download progress: {}", e))
}

// ============ CronLog CRUD ============

pub fn create_cron_log(
    pool: &DbPool,
    fetcher_type: String,
    status: String,
    error_message: Option<String>,
    attempt_count: i32,
) -> Result<CronLog, String> {
    let mut conn = pool.get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let new_log = NewCronLog {
        fetcher_type,
        status,
        error_message,
        attempt_count,
        executed_at: Utc::now().naive_utc(),
    };

    diesel::insert_into(cron_logs::table)
        .values(&new_log)
        .get_result::<CronLog>(&mut conn)
        .map_err(|e| format!("Failed to create cron log: {}", e))
}
