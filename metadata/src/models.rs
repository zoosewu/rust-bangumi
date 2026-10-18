use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct EnrichAnimeRequest {
    pub namespace: String,
    pub external_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CoverImageInfo {
    pub url: String,
    pub source: String,
}

/// 系列（沿前傳鏈找到的第一季）的內容；`title` 是各季共用的系列名稱
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SeriesInfo {
    pub namespace: String,
    pub external_id: String,
    pub title: String,
    pub original_title: String,
    pub summary: Option<String>,
    pub air_date: Option<String>,
    pub cover_images: Vec<CoverImageInfo>,
}

/// 一季的內容
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct EnrichAnimeResponse {
    pub namespace: String,
    pub external_id: String,
    pub title: String,
    pub title_cn: String,
    pub summary: Option<String>,
    pub air_date: Option<String>,
    pub end_date: Option<String>,
    pub total_episodes: Option<i32>,
    pub platform: String,
    pub cover_images: Vec<CoverImageInfo>,
    pub series: SeriesInfo,
}

#[derive(Debug, Deserialize)]
pub struct EnrichEpisodesRequest {
    pub namespace: String,
    pub external_id: String,
    pub episode_no: i32,
}

#[derive(Debug, Serialize)]
pub struct EnrichEpisodesResponse {
    pub episode_no: i32,
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<String>,
    pub summary: Option<String>,
}
