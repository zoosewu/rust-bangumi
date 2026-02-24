use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct EnrichAnimeRequest {
    pub title: String,
}

#[derive(Debug, Serialize, Default)]
pub struct CoverImageInfo {
    pub url: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct EnrichAnimeResponse {
    pub bangumi_id: Option<i32>,
    pub cover_images: Vec<CoverImageInfo>,
    pub summary: Option<String>,
    pub air_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EnrichEpisodesRequest {
    pub bangumi_id: i32,
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
