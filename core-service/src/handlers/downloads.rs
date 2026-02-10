//! 下載記錄查詢 API

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::Download;
use crate::schema::{anime_links, downloads};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListDownloadsQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct DownloadRow {
    pub download_id: i32,
    pub link_id: i32,
    pub title: Option<String>,
    pub downloader_type: String,
    pub status: String,
    pub progress: Option<f32>,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub torrent_hash: Option<String>,
    pub file_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// GET /downloads
pub async fn list_downloads(
    State(state): State<AppState>,
    Query(params): Query<ListDownloadsQuery>,
) -> Result<Json<Vec<DownloadRow>>, (StatusCode, String)> {
    let mut conn = state
        .db
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let mut query = downloads::table
        .order(downloads::updated_at.desc())
        .limit(limit)
        .offset(offset)
        .into_boxed();

    if let Some(ref status) = params.status {
        query = query.filter(downloads::status.eq(status));
    }

    let dl_list: Vec<Download> = query
        .load(&mut conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Batch-fetch link titles
    let link_ids: Vec<i32> = dl_list.iter().map(|d| d.link_id).collect();
    let link_titles: Vec<(i32, Option<String>)> = anime_links::table
        .filter(anime_links::link_id.eq_any(&link_ids))
        .select((anime_links::link_id, anime_links::title))
        .load(&mut conn)
        .unwrap_or_default();

    let title_map: std::collections::HashMap<i32, Option<String>> =
        link_titles.into_iter().collect();

    let rows: Vec<DownloadRow> = dl_list
        .into_iter()
        .map(|d| {
            let title = title_map.get(&d.link_id).cloned().flatten();
            DownloadRow {
                download_id: d.download_id,
                link_id: d.link_id,
                title,
                downloader_type: d.downloader_type,
                status: d.status,
                progress: d.progress,
                downloaded_bytes: d.downloaded_bytes,
                total_bytes: d.total_bytes,
                error_message: d.error_message,
                torrent_hash: d.torrent_hash,
                file_path: d.file_path,
                created_at: d.created_at.to_string(),
                updated_at: d.updated_at.to_string(),
            }
        })
        .collect();

    Ok(Json(rows))
}
