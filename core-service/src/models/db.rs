use diesel::prelude::*;
use chrono::{NaiveDate, NaiveDateTime, Utc};

// ============ Seasons ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::seasons)]
pub struct Season {
    pub season_id: i32,
    pub year: i32,
    pub season: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::seasons)]
pub struct NewSeason {
    pub year: i32,
    pub season: String,
    pub created_at: NaiveDateTime,
}

// ============ Animes ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::animes)]
pub struct Anime {
    pub anime_id: i32,
    pub title: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::animes)]
pub struct NewAnime {
    pub title: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ============ SubtitleGroups ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::subtitle_groups)]
pub struct SubtitleGroup {
    pub group_id: i32,
    pub group_name: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::subtitle_groups)]
pub struct NewSubtitleGroup {
    pub group_name: String,
    pub created_at: NaiveDateTime,
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
    pub created_at: NaiveDateTime,
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
    pub created_at: NaiveDateTime,
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
    pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::filter_rules)]
pub struct NewFilterRule {
    pub series_id: i32,
    pub group_id: i32,
    pub rule_order: i32,
    pub rule_type: String,
    pub regex_pattern: String,
    pub created_at: NaiveDateTime,
}

// ============ Downloads ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::downloads)]
pub struct Download {
    pub download_id: i32,
    pub link_id: i32,
    pub downloader_type: String,
    pub status: String,
    pub progress: Option<f32>,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::downloads)]
pub struct NewDownload {
    pub link_id: i32,
    pub downloader_type: String,
    pub status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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
    pub executed_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::cron_logs)]
pub struct NewCronLog {
    pub fetcher_type: String,
    pub status: String,
    pub error_message: Option<String>,
    pub attempt_count: i32,
    pub executed_at: NaiveDateTime,
}

// ============ FetcherModules ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::fetcher_modules)]
pub struct FetcherModule {
    pub fetcher_id: i32,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub config_schema: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub priority: i32,
    pub base_url: String,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::fetcher_modules)]
pub struct NewFetcherModule {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub config_schema: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub priority: i32,
    pub base_url: String,
}

// ============ Subscriptions (formerly RssSubscriptions) ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::subscriptions)]
pub struct Subscription {
    pub subscription_id: i32,
    pub fetcher_id: i32,
    pub source_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub last_fetched_at: Option<NaiveDateTime>,
    pub next_fetch_at: Option<NaiveDateTime>,
    pub fetch_interval_minutes: i32,
    pub is_active: bool,
    pub config: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub source_type: String,
    pub assignment_status: String,
    pub assigned_at: Option<NaiveDateTime>,
    pub auto_selected: bool,
}

// For manual inserts, use sql_query with bind parameters instead
#[derive(Insertable)]
#[diesel(table_name = super::super::schema::subscriptions)]
pub struct NewSubscription {
    pub fetcher_id: i32,
    pub source_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub last_fetched_at: Option<NaiveDateTime>,
    pub next_fetch_at: Option<NaiveDateTime>,
    pub fetch_interval_minutes: i32,
    pub is_active: bool,
    pub config: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub source_type: String,
    pub assignment_status: String,
    pub assigned_at: Option<NaiveDateTime>,
    pub auto_selected: bool,
}

// Compatibility alias for existing code
pub type RssSubscription = Subscription;
pub type NewRssSubscription = NewSubscription;

// ============ SubscriptionConflicts ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::subscription_conflicts)]
pub struct SubscriptionConflict {
    pub conflict_id: i32,
    pub subscription_id: i32,
    pub conflict_type: String,
    pub affected_item_id: Option<String>,
    pub conflict_data: String,
    pub resolution_status: String,
    pub resolution_data: Option<String>,
    pub created_at: NaiveDateTime,
    pub resolved_at: Option<NaiveDateTime>,
}

// For manual inserts, use sql_query with bind parameters instead
pub struct NewSubscriptionConflict {
    pub subscription_id: i32,
    pub conflict_type: String,
    pub affected_item_id: Option<String>,
    pub conflict_data: String,
    pub resolution_status: String,
    pub resolution_data: Option<String>,
    pub created_at: NaiveDateTime,
    pub resolved_at: Option<NaiveDateTime>,
}
