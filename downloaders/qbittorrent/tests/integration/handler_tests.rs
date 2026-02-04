// tests/integration/handler_tests.rs
use anyhow::anyhow;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use downloader_qbittorrent::MockDownloaderClient;
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

// Import the handlers module - we need to reference it from the binary crate
// For now, we'll test through the library's public API

mod handlers {
    use axum::{extract::State, http::StatusCode, Json};
    use downloader_qbittorrent::{retry_with_backoff, DownloaderClient};
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Debug, Deserialize)]
    pub struct DownloadRequest {
        pub link_id: i32,
        pub url: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct DownloadResponse {
        pub status: String,
        pub hash: Option<String>,
        pub error: Option<String>,
    }

    pub async fn download<C: DownloaderClient + 'static>(
        State(client): State<Arc<C>>,
        Json(req): Json<DownloadRequest>,
    ) -> (StatusCode, Json<DownloadResponse>) {
        if !req.url.starts_with("magnet:") {
            return (
                StatusCode::BAD_REQUEST,
                Json(DownloadResponse {
                    status: "unsupported".to_string(),
                    hash: None,
                    error: Some("Only magnet links supported".to_string()),
                }),
            );
        }

        let result = retry_with_backoff(3, Duration::from_millis(10), || {
            let client = client.clone();
            let url = req.url.clone();
            async move { client.add_magnet(&url, None).await }
        })
        .await;

        match result {
            Ok(hash) => (
                StatusCode::CREATED,
                Json(DownloadResponse {
                    status: "accepted".to_string(),
                    hash: Some(hash),
                    error: None,
                }),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DownloadResponse {
                    status: "error".to_string(),
                    hash: None,
                    error: Some(e.to_string()),
                }),
            ),
        }
    }
}

fn create_test_app(mock: MockDownloaderClient) -> Router {
    Router::new()
        .route("/download", post(handlers::download::<MockDownloaderClient>))
        .with_state(Arc::new(mock))
}

async fn parse_response(response: axum::response::Response) -> handlers::DownloadResponse {
    let body = response.into_body();
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ============ Request Validation Tests ============

#[tokio::test]
async fn test_download_valid_magnet_returns_201() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Ok("testhash123456789012345678901234".to_string()));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"magnet:?xt=urn:btih:testhash123456789012345678901234&dn=test"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_download_non_magnet_returns_400() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"http://example.com/file.torrent"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_download_invalid_json_returns_error() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"invalid": json"#))
                .unwrap(),
        )
        .await
        .unwrap();

    // Axum returns 400 Bad Request for JSON parsing errors
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============ Response Format Tests ============

#[tokio::test]
async fn test_download_success_response_has_hash() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Ok("responsehash12345678901234567890".to_string()));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"magnet:?xt=urn:btih:responsehash12345678901234567890"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = parse_response(response).await;
    assert_eq!(body.status, "accepted");
    assert_eq!(body.hash, Some("responsehash12345678901234567890".to_string()));
    assert!(body.error.is_none());
}

#[tokio::test]
async fn test_download_error_response_has_message() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Err(anyhow!("Connection timeout")));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"magnet:?xt=urn:btih:errorhash1234567890123456789012"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = parse_response(response).await;
    assert_eq!(body.status, "error");
    assert!(body.hash.is_none());
    assert!(body.error.is_some());
}

#[tokio::test]
async fn test_download_unsupported_response_format() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"https://not-a-magnet.com"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = parse_response(response).await;
    assert_eq!(body.status, "unsupported");
    assert!(body.error.is_some());
    assert!(body.error.unwrap().contains("magnet"));
}

// ============ Error Handling Tests ============

#[tokio::test]
async fn test_download_client_error_returns_500() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Err(anyhow!("Internal client error")));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":99,"url":"magnet:?xt=urn:btih:clienterror123456789012345678"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
