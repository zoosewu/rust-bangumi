use serde::{Deserialize, Serialize};

// ==============================
// Dashboard
// ==============================

#[derive(Debug, Deserialize, Serialize)]
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
    pub services: Vec<ServiceHealth>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServiceHealth {
    pub name: String,
    pub module_type: String,
    pub is_healthy: bool,
}

// ==============================
// Subscriptions
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct SubscriptionsResponse {
    pub subscriptions: Vec<SubscriptionResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SubscriptionResponse {
    pub subscription_id: i64,
    pub fetcher_id: Option<i64>,
    pub source_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub last_fetched_at: Option<String>,
    pub next_fetch_at: Option<String>,
    pub fetch_interval_minutes: i32,
    pub is_active: bool,
    pub source_type: Option<String>,
    pub assignment_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateSubscriptionRequest {
    pub source_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_interval_minutes: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct UpdateSubscriptionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_interval_minutes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
}

// ==============================
// AnimeWork (formerly Anime)
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct AnimeWorksResponse {
    pub animes: Vec<AnimeWorkResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AnimeWorkResponse {
    pub anime_id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateAnimeWorkRequest {
    pub title: String,
}

// ==============================
// Anime (formerly Series)
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct AnimesListResponse {
    pub series: Vec<AnimeRichResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AnimeRichResponse {
    pub series_id: i64,
    pub anime_id: i64,
    pub anime_title: String,
    pub series_no: i32,
    pub season: Option<SeasonInfo>,
    pub episode_downloaded: i64,
    pub episode_found: i64,
    pub subscriptions: Vec<SeriesSubscriptionRef>,
    pub description: Option<String>,
    pub aired_date: Option<String>,
    pub end_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SeasonInfo {
    pub year: i32,
    pub season: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SeriesSubscriptionRef {
    pub subscription_id: i64,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateSeriesRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aired_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
}

// ==============================
// Anime Links
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct LinksResponse {
    pub links: Vec<AnimeLinkRichResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AnimeLinkRichResponse {
    pub link_id: i64,
    pub series_id: i64,
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub conflict_flag: bool,
    pub download: Option<DownloadInfo>,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DownloadInfo {
    pub download_id: i64,
    pub status: String,
    pub progress: Option<f64>,
    pub torrent_hash: Option<String>,
}

// ==============================
// Raw Items
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct RawItemsResponse {
    pub items: Vec<RawItemResponse>,
    pub total: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawItemResponse {
    pub item_id: i64,
    pub subscription_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub download_url: String,
    pub pub_date: Option<String>,
    pub status: String,
    pub parser_id: Option<i64>,
    pub parsed_title: Option<String>,
    pub parsed_episode_no: Option<i32>,
    pub filtered_flag: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
}

// ==============================
// Downloads
// ==============================

// The backend GET /downloads returns a plain JSON array (not wrapped in an object).
pub type DownloadsResponse = Vec<DownloadResponse>;

#[derive(Debug, Deserialize, Serialize)]
pub struct DownloadResponse {
    pub download_id: i64,
    pub link_id: i64,
    pub title: Option<String>,
    pub downloader_type: Option<String>,
    pub status: String,
    pub progress: Option<f64>,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub torrent_hash: Option<String>,
    pub file_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ==============================
// Conflicts
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct ConflictsResponse {
    pub conflicts: Vec<ConflictResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConflictResponse {
    pub conflict_id: i64,
    pub rss_url: Option<String>,
    pub source_url: Option<String>,
    pub candidate_fetchers: Vec<CandidateFetcher>,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CandidateFetcher {
    pub fetcher_id: i64,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ResolveConflictRequest {
    pub fetcher_id: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LinkConflictsResponse {
    pub conflicts: Vec<LinkConflictResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LinkConflictResponse {
    pub conflict_id: i64,
    pub series_id: i64,
    pub episode_no: i32,
    pub links: Vec<ConflictingLink>,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConflictingLink {
    pub link_id: i64,
    pub group_name: Option<String>,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct ResolveLinkConflictRequest {
    pub chosen_link_id: i64,
}

// ==============================
// Filters
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct FiltersResponse {
    pub rules: Vec<FilterRuleResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FilterRuleResponse {
    pub rule_id: i64,
    pub target_type: String,
    pub target_id: Option<i64>,
    pub rule_order: i32,
    pub is_positive: bool,
    pub regex_pattern: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateFilterRuleRequest {
    pub target_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<i64>,
    pub rule_order: i32,
    pub is_positive: bool,
    pub regex_pattern: String,
}

// ==============================
// Parsers
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct ParsersResponse {
    pub parsers: Vec<ParserResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ParserResponse {
    pub parser_id: i64,
    pub name: String,
    pub priority: i32,
    pub condition_regex: Option<String>,
    pub enabled: bool,
    pub created_from_type: Option<String>,
    pub created_from_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateParserRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_from_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_from_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct UpdateParserRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

// ==============================
// Subtitle Groups
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct SubtitleGroupsResponse {
    pub groups: Vec<SubtitleGroupResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SubtitleGroupResponse {
    pub group_id: i64,
    pub group_name: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateSubtitleGroupRequest {
    pub group_name: String,
}

// ==============================
// Generic
// ==============================

#[derive(Debug, Deserialize, Serialize)]
pub struct MessageResponse {
    pub message: String,
}
