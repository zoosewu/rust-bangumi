use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

// ============ Anime DTO ============
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnimeRequest {
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnimeResponse {
    pub anime_id: i32,
    pub title: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ============ Season DTO ============
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SeasonRequest {
    pub year: i32,
    pub season: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SeasonResponse {
    pub season_id: i32,
    pub year: i32,
    pub season: String,
    pub created_at: NaiveDateTime,
}

// ============ AnimeSeries DTO ============
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnimeSeriesRequest {
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnimeSeriesResponse {
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UpdateAnimeSeriesRequest {
    pub season_id: Option<i32>,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

// ============ SubtitleGroup DTO ============
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SubtitleGroupRequest {
    pub group_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubtitleGroupResponse {
    pub group_id: i32,
    pub group_name: String,
    pub created_at: NaiveDateTime,
}

// ============ FilterRule DTO ============
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FilterRuleRequest {
    pub target_type: String,
    pub target_id: Option<i32>,
    pub rule_order: i32,
    pub is_positive: bool,
    pub regex_pattern: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilterRuleResponse {
    pub rule_id: i32,
    pub target_type: String,
    pub target_id: Option<i32>,
    pub rule_order: i32,
    pub is_positive: bool,
    pub regex_pattern: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ============ AnimeSeriesRich DTO (for list_all_anime_series) ============
#[derive(Debug, Serialize, Clone)]
pub struct SeasonInfo {
    pub year: i32,
    pub season: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SubscriptionInfo {
    pub subscription_id: i32,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AnimeSeriesRichResponse {
    pub series_id: i32,
    pub anime_id: i32,
    pub anime_title: String,
    pub series_no: i32,
    pub season: SeasonInfo,
    pub episode_downloaded: i64,
    pub episode_found: i64,
    pub subscriptions: Vec<SubscriptionInfo>,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ============ AnimeLinkRich DTO (for get_anime_links) ============
#[derive(Debug, Serialize, Clone)]
pub struct DownloadInfo {
    pub download_id: i32,
    pub status: String,
    pub progress: Option<f32>,
    pub torrent_hash: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AnimeLinkRichResponse {
    pub link_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub group_name: String,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub conflict_flag: bool,
    pub conflicting_link_ids: Vec<i32>,
    pub download: Option<DownloadInfo>,
    pub created_at: NaiveDateTime,
}

// ============ DashboardStats DTO ============
#[derive(Debug, Serialize, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub module_type: String,
    pub is_healthy: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct DashboardStats {
    pub total_anime: i64,
    pub total_series: i64,
    pub active_subscriptions: i64,
    pub total_downloads: i64,
    pub downloading: i64,
    pub completed: i64,
    pub failed: i64,
    pub pending_raw_items: i64,
    pub pending_conflicts: i64,
    pub services: Vec<ServiceInfo>,
}

// ============ AnimeLink DTO ============
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnimeLinkRequest {
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnimeLinkResponse {
    pub link_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub created_at: NaiveDateTime,
}

// ============ AnimeLinkConflict DTOs ============

#[derive(Debug, Serialize, Clone)]
pub struct AnimeLinkConflictLink {
    pub link_id: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub conflict_flag: bool,
    pub link_status: String,
    pub download: Option<DownloadInfo>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Clone)]
pub struct AnimeLinkConflictInfo {
    pub conflict_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub anime_title: String,
    pub group_name: String,
    pub resolution_status: String,
    pub chosen_link_id: Option<i32>,
    pub links: Vec<AnimeLinkConflictLink>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Deserialize)]
pub struct ResolveAnimeLinkConflictRequest {
    pub chosen_link_id: i32,
}
