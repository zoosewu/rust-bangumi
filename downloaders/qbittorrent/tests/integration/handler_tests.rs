// tests/integration/handler_tests.rs
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{delete, get, post},
    Router,
};
use downloader_qbittorrent::MockDownloaderClient;
use http_body_util::BodyExt;
use shared::{
    BatchCancelResponse, BatchDownloadResponse, CancelResultItem, DownloadResultItem,
    DownloadStatusItem, StatusQueryResponse,
};
use std::sync::Arc;
use tower::ServiceExt;

// Import the handlers module - we need to reference it from the binary crate
// For now, we'll test through a duplicated copy of handlers (binary crate handlers can't be imported)

mod handlers {
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
        pub hashes: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct DeleteParams {
        pub delete_files: Option<bool>,
    }

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

    pub async fn pause<C: DownloaderClient + 'static>(
        State(client): State<Arc<C>>,
        Path(hash): Path<String>,
    ) -> StatusCode {
        match client.pause_torrent(&hash).await {
            Ok(()) => StatusCode::OK,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub async fn resume<C: DownloaderClient + 'static>(
        State(client): State<Arc<C>>,
        Path(hash): Path<String>,
    ) -> StatusCode {
        match client.resume_torrent(&hash).await {
            Ok(()) => StatusCode::OK,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub async fn delete_download<C: DownloaderClient + 'static>(
        State(client): State<Arc<C>>,
        Path(hash): Path<String>,
        Query(params): Query<DeleteParams>,
    ) -> StatusCode {
        let delete_files = params.delete_files.unwrap_or(false);
        match client.delete_torrent(&hash, delete_files).await {
            Ok(()) => StatusCode::OK,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub async fn health_check() -> StatusCode {
        StatusCode::OK
    }
}

fn create_test_app(mock: MockDownloaderClient) -> Router {
    let state = Arc::new(mock);
    Router::new()
        .route(
            "/downloads",
            post(handlers::batch_download::<MockDownloaderClient>),
        )
        .route(
            "/downloads",
            get(handlers::query_download_status::<MockDownloaderClient>),
        )
        .route(
            "/downloads/cancel",
            post(handlers::batch_cancel::<MockDownloaderClient>),
        )
        .route(
            "/downloads/:hash/pause",
            post(handlers::pause::<MockDownloaderClient>),
        )
        .route(
            "/downloads/:hash/resume",
            post(handlers::resume::<MockDownloaderClient>),
        )
        .route(
            "/downloads/:hash",
            delete(handlers::delete_download::<MockDownloaderClient>),
        )
        .route("/health", get(handlers::health_check))
        .with_state(state)
}

// ============ Batch Download Tests ============

#[tokio::test]
async fn test_batch_download_returns_200() {
    let mock = MockDownloaderClient::new().with_add_torrents_result(Ok(vec![DownloadResultItem {
        url: "magnet:?xt=urn:btih:testhash123456789012345678901234&dn=test".to_string(),
        hash: Some("testhash123456789012345678901234".to_string()),
        status: "accepted".to_string(),
        reason: None,
    }]));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"items":[{"url":"magnet:?xt=urn:btih:testhash123456789012345678901234&dn=test","save_path":"/downloads"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body();
    let bytes = body.collect().await.unwrap().to_bytes();
    let resp: BatchDownloadResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(resp.results.len(), 1);
    assert_eq!(resp.results[0].status, "accepted");
}

#[tokio::test]
async fn test_batch_download_empty_items_returns_400() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"items":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_batch_download_invalid_json_returns_error() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"invalid": json"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_batch_download_client_error_returns_500() {
    let mock = MockDownloaderClient::new()
        .with_add_torrents_result(Err(anyhow::anyhow!("Internal client error")));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"items":[{"url":"magnet:?xt=urn:btih:abc12345678901234567890123456789a","save_path":"/downloads"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// ============ Batch Cancel Tests ============

#[tokio::test]
async fn test_batch_cancel_returns_200() {
    let mock =
        MockDownloaderClient::new().with_cancel_torrents_result(Ok(vec![CancelResultItem {
            hash: "abc123".to_string(),
            status: "cancelled".to_string(),
        }]));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads/cancel")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"hashes":["abc123"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body();
    let bytes = body.collect().await.unwrap().to_bytes();
    let resp: BatchCancelResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(resp.results.len(), 1);
    assert_eq!(resp.results[0].status, "cancelled");
}

#[tokio::test]
async fn test_batch_cancel_empty_hashes_returns_400() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads/cancel")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"hashes":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============ Status Query Tests ============

#[tokio::test]
async fn test_query_status_returns_200() {
    let mock = MockDownloaderClient::new().with_query_status_result(Ok(vec![DownloadStatusItem {
        hash: "hash1".to_string(),
        status: "downloading".to_string(),
        progress: 0.5,
        size: 1000000,
        content_path: None,
    }]));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/downloads?hashes=hash1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body();
    let bytes = body.collect().await.unwrap().to_bytes();
    let resp: StatusQueryResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(resp.statuses.len(), 1);
    assert_eq!(resp.statuses[0].hash, "hash1");
    assert_eq!(resp.statuses[0].progress, 0.5);
}

#[tokio::test]
async fn test_query_status_missing_hashes_returns_400() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/downloads?hashes=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============ Health Check Test ============

#[tokio::test]
async fn test_health_check_returns_200() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
