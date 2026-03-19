use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ============ Anime DTO ============
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct AnimeWorkRequest {
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct AnimeWorkResponse {
    pub anime_id: i32,
    pub title: String,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub updated_at: NaiveDateTime,
}

/// 用於 list_anime_works 的包裝響應
#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct AnimeWorksListResponse {
    pub animes: Vec<AnimeWorkResponse>,
}

// ============ Season DTO ============
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct SeasonRequest {
    pub year: i32,
    pub season: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct SeasonResponse {
    pub season_id: i32,
    pub year: i32,
    pub season: String,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
}

// ============ AnimeSeries DTO ============
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct AnimeRequest {
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    #[schema(value_type = Option<String>, format = Date)]
    pub aired_date: Option<NaiveDate>,
    #[schema(value_type = Option<String>, format = Date)]
    pub end_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct AnimeResponse {
    pub series_id: i32,
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    #[schema(value_type = Option<String>, format = Date)]
    pub aired_date: Option<NaiveDate>,
    #[schema(value_type = Option<String>, format = Date)]
    pub end_date: Option<NaiveDate>,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct UpdateAnimeRequest {
    pub season_id: Option<i32>,
    pub description: Option<String>,
    #[schema(value_type = Option<String>, format = Date)]
    pub aired_date: Option<NaiveDate>,
    #[schema(value_type = Option<String>, format = Date)]
    pub end_date: Option<NaiveDate>,
}

// ============ SubtitleGroup DTO ============
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct SubtitleGroupRequest {
    pub group_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct SubtitleGroupResponse {
    pub group_id: i32,
    pub group_name: String,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
}

// ============ FilterRule DTO ============
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct FilterRuleRequest {
    pub target_type: String,
    pub target_id: Option<i32>,
    pub rule_order: i32,
    pub is_positive: bool,
    pub regex_pattern: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct FilterRuleResponse {
    pub rule_id: i32,
    pub target_type: String,
    pub target_id: Option<i32>,
    pub target_name: Option<String>,
    pub rule_order: i32,
    pub is_positive: bool,
    pub regex_pattern: String,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub updated_at: NaiveDateTime,
}

// ============ AnimeSeriesRich DTO (for list_all_anime_series) ============
#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct SeasonInfo {
    pub year: i32,
    pub season: String,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct SubscriptionInfo {
    pub subscription_id: i32,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct AnimeRichResponse {
    pub series_id: i32,
    pub anime_id: i32,
    pub anime_title: String,
    pub series_no: i32,
    pub season: SeasonInfo,
    pub episode_downloaded: i64,
    pub episode_found: i64,
    pub subscriptions: Vec<SubscriptionInfo>,
    pub description: Option<String>,
    #[schema(value_type = Option<String>, format = Date)]
    pub aired_date: Option<NaiveDate>,
    #[schema(value_type = Option<String>, format = Date)]
    pub end_date: Option<NaiveDate>,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub updated_at: NaiveDateTime,
    pub cover_image_url: Option<String>,
}

// ============ AnimeLinkRich DTO (for get_anime_links) ============
#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct DownloadInfo {
    pub download_id: i32,
    pub status: String,
    pub progress: Option<f32>,
    pub torrent_hash: Option<String>,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
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
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
}

// ============ ConflictingLink DTO (for list_conflicting_links) ============
#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct ConflictingLinkResponse {
    pub link_id: i32,
    pub episode_no: i32,
    pub group_name: String,
    pub url: String,
    pub conflicting_link_ids: Vec<i32>,
    pub series_id: i32,
    pub series_no: i32,
    pub anime_work_id: i32,
    pub anime_work_title: String,
    pub subscription_id: Option<i32>,
    pub subscription_name: Option<String>,
}

// ============ DashboardStats DTO ============
#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct ServiceInfo {
    pub name: String,
    pub module_type: String,
    pub is_healthy: bool,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
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
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct AnimeLinkRequest {
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct AnimeLinkResponse {
    pub link_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
}

// ============ AnimeLinkConflict DTOs ============

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct AnimeLinkConflictLink {
    pub link_id: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub conflict_flag: bool,
    pub link_status: String,
    pub download: Option<DownloadInfo>,
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
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
    #[schema(value_type = String, format = DateTime)]
    #[serde(serialize_with = "crate::serde_utils::naive_datetime_utc::serialize")]
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResolveAnimeLinkConflictRequest {
    pub chosen_link_id: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anime_work_request_json_has_title_field() {
        let req = AnimeWorkRequest { title: "test".to_string() };
        let v = serde_json::to_value(&req).unwrap();
        assert_eq!(v["title"], "test");  // JSON key "title" 不變
    }

    #[test]
    fn anime_request_json_fields_are_api_contract() {
        let req = AnimeRequest {
            anime_id: 1, series_no: 2, season_id: 3,
            description: None, aired_date: None, end_date: None,
        };
        let v = serde_json::to_value(&req).unwrap();
        assert_eq!(v["anime_id"], 1);   // JSON key "anime_id" 不變
        assert_eq!(v["series_no"], 2);  // JSON key "series_no" 不變
        assert_eq!(v["season_id"], 3);  // JSON key "season_id" 不變
    }

    #[test]
    fn anime_rich_json_has_series_id_and_anime_id() {
        use chrono::Utc;
        let resp = AnimeRichResponse {
            series_id: 42, anime_id: 1,
            anime_title: "Test".to_string(), series_no: 1,
            season: SeasonInfo { year: 2024, season: "spring".to_string() },
            episode_downloaded: 0, episode_found: 0,
            subscriptions: vec![],
            description: None, aired_date: None, end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            cover_image_url: None,
        };
        let v = serde_json::to_value(&resp).unwrap();
        assert_eq!(v["series_id"], 42);  // JSON key 不變
        assert_eq!(v["anime_id"], 1);    // JSON key 不變
    }
}
