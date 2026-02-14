use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============ Download Types ============

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadType {
    Magnet,
    Torrent,
    Http,
}

impl std::fmt::Display for DownloadType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadType::Magnet => write!(f, "magnet"),
            DownloadType::Torrent => write!(f, "torrent"),
            DownloadType::Http => write!(f, "http"),
        }
    }
}

impl std::str::FromStr for DownloadType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "magnet" => Ok(DownloadType::Magnet),
            "torrent" => Ok(DownloadType::Torrent),
            "http" => Ok(DownloadType::Http),
            _ => Err(format!("Unknown download type: {}", s)),
        }
    }
}

// ============ Service Registration ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRegistration {
    pub service_type: ServiceType,
    pub service_name: String,
    pub host: String,
    pub port: u16,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ServiceType {
    Fetcher,
    Downloader,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_endpoint: Option<String>,
    #[serde(default)]
    pub supported_download_types: Vec<DownloadType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRegistrationResponse {
    pub service_id: Uuid,
    pub registered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredService {
    pub service_id: Uuid,
    pub service_type: ServiceType,
    pub service_name: String,
    pub host: String,
    pub port: u16,
    pub capabilities: Capabilities,
    pub is_healthy: bool,
    pub last_heartbeat: DateTime<Utc>,
}

// ============ Anime & Links ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResponse {
    pub animes: Vec<FetchedAnime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedAnime {
    pub title: String,
    pub description: String,
    pub season: String, // 冬/春/夏/秋
    pub year: i32,
    pub series_no: i32,
    pub links: Vec<FetchedLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedLink {
    pub episode_no: i32,
    pub subtitle_group: String,
    pub title: String,
    pub url: String, // magnet/torrent/http 等格式
    pub source_hash: String,
    pub source_rss_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeMetadata {
    pub anime_id: i64,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeSeriesMetadata {
    pub series_id: i64,
    pub anime_id: i64,
    pub series_no: i32,
    pub season_id: i64,
    pub description: Option<String>,
    pub aired_date: Option<String>,
    pub end_date: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleGroup {
    pub group_id: i64,
    pub group_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeLink {
    pub link_id: i64,
    pub series_id: i64,
    pub group_id: i64,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============ Download ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub link_id: Uuid,
    pub url: String,
    pub callback_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Accepted,
    Unsupported,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResponse {
    pub status: DownloadStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub link_id: Uuid,
    pub downloader_type: String,
    pub status: String, // downloading/completed/failed
    pub progress: f64,  // 0-1
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub error_message: Option<String>,
}

// ============ Viewer/Sync ============

#[deprecated(note = "Use ViewerSyncRequest instead")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    pub link_id: Uuid,
    pub anime_title: String,
    pub series_no: i32,
    pub episode_no: i32,
    pub subtitle_group: String,
    pub file_path: String,
    pub file_size: u64,
}

#[deprecated(note = "Use ViewerSyncCallback instead")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub status: String, // synced/failed
    pub target_path: String,
    pub message: String,
}

/// Core → Viewer: request to sync a completed download
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerSyncRequest {
    pub download_id: i32,
    pub series_id: i32,
    pub anime_title: String,
    pub series_no: i32,
    pub episode_no: i32,
    pub subtitle_group: String,
    pub file_path: String,
    pub callback_url: String,
}

/// Core → Viewer: request to resync a previously synced download with updated metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerResyncRequest {
    pub download_id: i32,
    pub series_id: i32,
    pub anime_title: String,
    pub series_no: i32,
    pub episode_no: i32,
    pub subtitle_group: String,
    pub old_target_path: String,
    pub callback_url: String,
}

/// Viewer → Core: callback after sync processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerSyncCallback {
    pub download_id: i32,
    pub status: String, // "synced" | "failed"
    pub target_path: Option<String>,
    pub error_message: Option<String>,
}

// ============ Filter Rules ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterRule {
    pub rule_id: i64,
    pub series_id: i64,
    pub group_id: i64,
    pub rule_order: i32,
    pub rule_type: FilterType,
    pub regex_pattern: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum FilterType {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFilterRuleRequest {
    pub series_id: i64,
    pub group_id: i64,
    pub rule_type: FilterType,
    pub regex_pattern: String,
}

// ============ Cron ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub subscription_id: Uuid,
    pub fetcher_type: String,
    pub cron_expression: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronStatus {
    pub job_id: Uuid,
    pub status: String,
    pub last_execution: Option<DateTime<Utc>>,
    pub next_execution: Option<DateTime<Utc>>,
}

// ============ Fetch Trigger (Core -> Fetcher) ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchTriggerRequest {
    pub subscription_id: i32,
    pub rss_url: String,
    pub callback_url: String, // Core 的 /fetcher-results endpoint
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchTriggerResponse {
    pub accepted: bool,
    pub message: String,
}

// ============ Raw Anime Item (New Architecture) ============

/// 原始動畫項目（單集）- 來自 Fetcher 的原始資料
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAnimeItem {
    pub title: String,                   // RSS <title>
    pub description: Option<String>,     // RSS <description>
    pub download_url: String,            // RSS <enclosure> url
    pub pub_date: Option<DateTime<Utc>>, // RSS <pubDate>
}

/// Fetcher 回傳的原始結果（新架構）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFetcherResultsPayload {
    pub subscription_id: i32,
    pub items: Vec<RawAnimeItem>,
    pub fetcher_source: String,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Core 處理原始結果的回應
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFetcherResultsResponse {
    pub success: bool,
    pub items_received: usize,
    pub items_parsed: usize,
    pub items_failed: usize,
    pub message: String,
}

// ============ Batch Download API DTOs ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDownloadRequest {
    pub items: Vec<DownloadRequestItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequestItem {
    pub url: String,
    pub save_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDownloadResponse {
    pub results: Vec<DownloadResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResultItem {
    pub url: String,
    pub hash: Option<String>,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCancelRequest {
    pub hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCancelResponse {
    pub results: Vec<CancelResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelResultItem {
    pub hash: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusQueryResponse {
    pub statuses: Vec<DownloadStatusItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStatusItem {
    pub hash: String,
    pub status: String,
    pub progress: f64,
    pub size: u64,
    pub content_path: Option<String>,
}
