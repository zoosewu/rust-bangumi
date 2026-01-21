use diesel::prelude::*;
use chrono::{DateTime, NaiveDate, Utc};

// ============ Seasons ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::seasons)]
pub struct Season {
    pub season_id: i32,
    pub year: i32,
    pub season: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::seasons)]
pub struct NewSeason {
    pub year: i32,
    pub season: String,
}

// ============ Animes ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::animes)]
pub struct Anime {
    pub anime_id: i32,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::animes)]
pub struct NewAnime {
    pub title: String,
}

// ============ AnimeSeries ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::anime_series)]
pub struct AnimeSeries {
    pub series_id: i32,
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::anime_series)]
pub struct NewAnimeSeries {
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

// ============ SubtitleGroups ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::subtitle_groups)]
pub struct SubtitleGroup {
    pub group_id: i32,
    pub group_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::subtitle_groups)]
pub struct NewSubtitleGroup {
    pub group_name: String,
}

// ============ AnimeLinks ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::anime_links)]
pub struct AnimeLink {
    pub link_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::anime_links)]
pub struct NewAnimeLink {
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
}

// ============ FilterRules ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::filter_rules)]
pub struct FilterRule {
    pub rule_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub rule_order: i32,
    pub rule_type: String,
    pub regex_pattern: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::filter_rules)]
pub struct NewFilterRule {
    pub series_id: i32,
    pub group_id: i32,
    pub rule_order: i32,
    pub rule_type: String,
    pub regex_pattern: String,
}

// ============ Downloads ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::downloads)]
pub struct Download {
    pub download_id: i32,
    pub link_id: i32,
    pub downloader_type: String,
    pub status: String,
    pub progress: Option<f64>,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::downloads)]
pub struct NewDownload {
    pub link_id: i32,
    pub downloader_type: String,
}

// ============ CronLogs ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::cron_logs)]
pub struct CronLog {
    pub log_id: i32,
    pub fetcher_type: String,
    pub status: String,
    pub error_message: Option<String>,
    pub attempt_count: i32,
    pub executed_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::cron_logs)]
pub struct NewCronLog {
    pub fetcher_type: String,
    pub status: String,
    pub error_message: Option<String>,
    pub attempt_count: i32,
}
