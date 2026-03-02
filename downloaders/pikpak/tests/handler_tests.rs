// tests/handler_tests.rs
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{delete, get, post},
    Router,
};
use downloader_pikpak::{handlers, MockPikPakClient};
use http_body_util::BodyExt;
use shared::{
    BatchCancelRequest, BatchDownloadRequest, BatchDownloadResponse, CancelResultItem,
    DownloadRequestItem, DownloadResultItem, DownloadStatusItem, StatusQueryResponse,
};
use std::sync::Arc;
use tower::ServiceExt;

fn make_app(client: MockPikPakClient) -> Router {
    Router::new()
        .route(
            "/downloads",
            post(handlers::batch_download::<MockPikPakClient>),
        )
        .route(
            "/downloads",
            get(handlers::query_download_status::<MockPikPakClient>),
        )
        .route(
            "/downloads/cancel",
            post(handlers::batch_cancel::<MockPikPakClient>),
        )
        .route(
            "/downloads/:hash/pause",
            post(handlers::pause::<MockPikPakClient>),
        )
        .route(
            "/downloads/:hash/resume",
            post(handlers::resume::<MockPikPakClient>),
        )
        .route(
            "/downloads/:hash",
            delete(handlers::delete_download::<MockPikPakClient>),
        )
        .route("/health", get(handlers::health_check))
        .with_state(Arc::new(client))
}

#[tokio::test]
async fn test_health_check() {
    let app = make_app(MockPikPakClient::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_batch_download_empty_returns_400() {
    let app = make_app(MockPikPakClient::new());
    let body = serde_json::to_vec(&BatchDownloadRequest { items: vec![] }).unwrap();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_batch_download_success() {
    let result = vec![DownloadResultItem {
        url: "magnet:test".to_string(),
        hash: Some("abc123".to_string()),
        status: "accepted".to_string(),
        reason: None,
    }];
    let client = MockPikPakClient::new().with_add_torrents_result(Ok(result));
    let app = make_app(client);
    let req_body = serde_json::to_vec(&BatchDownloadRequest {
        items: vec![DownloadRequestItem {
            url: "magnet:test".to_string(),
            save_path: "/downloads".to_string(),
        }],
    })
    .unwrap();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads")
                .header("content-type", "application/json")
                .body(Body::from(req_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: BatchDownloadResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.results.len(), 1);
    assert_eq!(parsed.results[0].hash.as_deref(), Some("abc123"));
}

#[tokio::test]
async fn test_query_status_empty_hashes_returns_400() {
    let app = make_app(MockPikPakClient::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/downloads?hashes=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_query_status_success() {
    let statuses = vec![DownloadStatusItem {
        hash: "hash1".to_string(),
        status: "downloading".to_string(),
        progress: 0.5,
        size: 1000000,
        content_path: None,
        files: vec![],
    }];
    let client = MockPikPakClient::new().with_query_status_result(Ok(statuses));
    let app = make_app(client);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/downloads?hashes=hash1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: StatusQueryResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.statuses.len(), 1);
    assert_eq!(parsed.statuses[0].hash, "hash1");
}

#[tokio::test]
async fn test_cancel_empty_returns_400() {
    let app = make_app(MockPikPakClient::new());
    let body = serde_json::to_vec(&BatchCancelRequest { hashes: vec![] }).unwrap();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads/cancel")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_cancel_success() {
    let results = vec![CancelResultItem {
        hash: "abc123".to_string(),
        status: "cancelled".to_string(),
    }];
    let client = MockPikPakClient::new().with_cancel_torrents_result(Ok(results));
    let app = make_app(client);
    let body = serde_json::to_vec(&BatchCancelRequest {
        hashes: vec!["abc123".to_string()],
    })
    .unwrap();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads/cancel")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_pause_success() {
    let app = make_app(MockPikPakClient::new());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads/abc123/pause")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_resume_success() {
    let app = make_app(MockPikPakClient::new());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/downloads/abc123/resume")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_delete_success() {
    let app = make_app(MockPikPakClient::new());
    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/downloads/abc123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
