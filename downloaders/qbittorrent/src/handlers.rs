use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use downloader_qbittorrent::DownloaderClient;
use serde::Deserialize;
use shared::{
    BatchCancelRequest, BatchCancelResponse, BatchDownloadRequest, BatchDownloadResponse,
    StatusQueryResponse,
};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct StatusQueryParams {
    pub hashes: String, // comma-separated
}

#[derive(Debug, Deserialize)]
pub struct DeleteParams {
    pub delete_files: Option<bool>,
}

/// POST /downloads - batch add torrents
pub async fn batch_download<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Json(req): Json<BatchDownloadRequest>,
) -> (StatusCode, Json<BatchDownloadResponse>) {
    if req.items.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(BatchDownloadResponse { results: vec![] }),
        );
    }

    match client.add_torrents(req.items).await {
        Ok(results) => (StatusCode::OK, Json(BatchDownloadResponse { results })),
        Err(e) => {
            tracing::error!("Batch download failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BatchDownloadResponse { results: vec![] }),
            )
        }
    }
}

/// POST /downloads/cancel - batch cancel
pub async fn batch_cancel<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Json(req): Json<BatchCancelRequest>,
) -> (StatusCode, Json<BatchCancelResponse>) {
    if req.hashes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(BatchCancelResponse { results: vec![] }),
        );
    }

    match client.cancel_torrents(req.hashes).await {
        Ok(results) => (StatusCode::OK, Json(BatchCancelResponse { results })),
        Err(e) => {
            tracing::error!("Batch cancel failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BatchCancelResponse { results: vec![] }),
            )
        }
    }
}

/// GET /downloads?hashes=hash1,hash2 - query status
pub async fn query_download_status<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Query(params): Query<StatusQueryParams>,
) -> (StatusCode, Json<StatusQueryResponse>) {
    let hashes: Vec<String> = params
        .hashes
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if hashes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(StatusQueryResponse { statuses: vec![] }),
        );
    }

    match client.query_status(hashes).await {
        Ok(statuses) => (StatusCode::OK, Json(StatusQueryResponse { statuses })),
        Err(e) => {
            tracing::error!("Status query failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(StatusQueryResponse { statuses: vec![] }),
            )
        }
    }
}

/// POST /downloads/:hash/pause
pub async fn pause<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Path(hash): Path<String>,
) -> StatusCode {
    match client.pause_torrent(&hash).await {
        Ok(()) => {
            tracing::info!("Torrent {} paused", hash);
            StatusCode::OK
        }
        Err(e) => {
            tracing::error!("Failed to pause torrent {}: {}", hash, e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// POST /downloads/:hash/resume
pub async fn resume<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Path(hash): Path<String>,
) -> StatusCode {
    match client.resume_torrent(&hash).await {
        Ok(()) => {
            tracing::info!("Torrent {} resumed", hash);
            StatusCode::OK
        }
        Err(e) => {
            tracing::error!("Failed to resume torrent {}: {}", hash, e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// DELETE /downloads/:hash
pub async fn delete_download<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Path(hash): Path<String>,
    Query(params): Query<DeleteParams>,
) -> StatusCode {
    let delete_files = params.delete_files.unwrap_or(false);

    match client.delete_torrent(&hash, delete_files).await {
        Ok(()) => {
            tracing::info!("Torrent {} deleted (delete_files={})", hash, delete_files);
            StatusCode::OK
        }
        Err(e) => {
            tracing::error!("Failed to delete torrent {}: {}", hash, e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// GET /health
pub async fn health_check() -> StatusCode {
    StatusCode::OK
}
