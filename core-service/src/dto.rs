use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

// ============ Error Response ============
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

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
