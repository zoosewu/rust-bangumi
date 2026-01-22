use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use downloader_qbittorrent::{QBittorrentClient, retry_with_backoff};

#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    pub link_id: i32,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct DownloadResponse {
    pub status: String,
    pub hash: Option<String>,
    pub error: Option<String>,
}

pub async fn download(
    State(client): State<Arc<QBittorrentClient>>,
    Json(req): Json<DownloadRequest>,
) -> (StatusCode, Json<DownloadResponse>) {
    if !req.url.starts_with("magnet:") {
        return (StatusCode::BAD_REQUEST, Json(DownloadResponse {
            status: "unsupported".to_string(),
            hash: None,
            error: Some("Only magnet links supported".to_string()),
        }));
    }

    // Use retry logic for download with exponential backoff
    let result = retry_with_backoff(3, Duration::from_secs(1), || {
        let client = client.clone();
        let url = req.url.clone();
        async move {
            client.add_magnet(&url, None).await
        }
    }).await;

    match result {
        Ok(hash) => {
            tracing::info!("Download started: link_id={}, hash={}", req.link_id, hash);
            (StatusCode::CREATED, Json(DownloadResponse {
                status: "accepted".to_string(),
                hash: Some(hash),
                error: None,
            }))
        }
        Err(e) => {
            tracing::error!("Download failed after retries: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(DownloadResponse {
                status: "error".to_string(),
                hash: None,
                error: Some(e.to_string()),
            }))
        }
    }
}

pub async fn health_check() -> StatusCode {
    StatusCode::OK
}
