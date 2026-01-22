use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// 動畫元數據
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeMetadata {
    pub anime_id: i64,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 季度資訊
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SeasonInfo {
    pub season_id: i64,
    pub year: i32,
    pub season: String, // 冬/春/夏/秋
}

/// 動畫系列
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
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

/// 字幕組
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SubtitleGroup {
    pub group_id: i64,
    pub group_name: String,
    pub created_at: DateTime<Utc>,
}

/// 動畫連結
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

/// 過濾規則
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

/// 過濾規則類型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum FilterType {
    Positive,
    Negative,
}

/// 創建過濾規則請求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFilterRuleRequest {
    pub series_id: i64,
    pub group_id: i64,
    pub rule_type: FilterType,
    pub regex_pattern: String,
}

/// 服務資訊
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredService {
    pub service_id: String,
    pub service_type: String,
    pub service_name: String,
    pub host: String,
    pub port: u16,
    pub is_healthy: bool,
    pub last_heartbeat: DateTime<Utc>,
}

/// 下載狀態
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DownloadProgress {
    pub link_id: String,
    pub downloader_type: String,
    pub status: String,
    pub progress: f64,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub error_message: Option<String>,
}

/// 列表響應
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub total: Option<i64>,
}

/// 簡單成功響應
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessResponse {
    pub message: String,
}

/// RSS 訂閱請求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub rss_url: String,
    pub fetcher: String,
}

/// 下載請求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub link_id: i64,
    pub downloader: Option<String>,
}
