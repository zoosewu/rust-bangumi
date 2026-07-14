// tests/integration/relogin_tests.rs
//
// 403 自動重登整合測試：以 axum 模擬 qBittorrent WebUI，
// 驗證真實 QBittorrentClient 在啟動登入失敗（憑證已注入但 session 不存在）時，
// 收到 403 能自動重登並重試成功。

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Router,
};
use downloader_qbittorrent::{DownloaderClient, QBittorrentClient};
use shared::DownloadRequestItem;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

struct MockQbState {
    logged_in: AtomicBool,
    login_calls: AtomicU32,
    add_calls: AtomicU32,
}

async fn mock_login(State(state): State<Arc<MockQbState>>) -> (StatusCode, &'static str) {
    state.login_calls.fetch_add(1, Ordering::SeqCst);
    state.logged_in.store(true, Ordering::SeqCst);
    (StatusCode::OK, "Ok.")
}

async fn mock_add(State(state): State<Arc<MockQbState>>) -> (StatusCode, &'static str) {
    state.add_calls.fetch_add(1, Ordering::SeqCst);
    if state.logged_in.load(Ordering::SeqCst) {
        (StatusCode::OK, "Ok.")
    } else {
        (StatusCode::FORBIDDEN, "Forbidden")
    }
}

async fn mock_info(State(state): State<Arc<MockQbState>>) -> (StatusCode, &'static str) {
    if state.logged_in.load(Ordering::SeqCst) {
        (StatusCode::OK, "[]")
    } else {
        (StatusCode::FORBIDDEN, "Forbidden")
    }
}

/// 啟動 mock qBittorrent，回傳 (base_url, state)
async fn spawn_mock_qbittorrent() -> (String, Arc<MockQbState>) {
    let state = Arc::new(MockQbState {
        logged_in: AtomicBool::new(false),
        login_calls: AtomicU32::new(0),
        add_calls: AtomicU32::new(0),
    });

    let app = Router::new()
        .route("/api/v2/auth/login", post(mock_login))
        .route("/api/v2/torrents/add", post(mock_add))
        .route("/api/v2/torrents/info", get(mock_info))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock server");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock server");
    });

    (format!("http://{}", addr), state)
}

const MAGNET: &str = "magnet:?xt=urn:btih:217e6da069a8ee782fb6d2cabe8e438f6293780c";

#[tokio::test]
async fn add_torrents_relogins_after_403_with_injected_credentials() {
    let (base_url, state) = spawn_mock_qbittorrent().await;
    let client = QBittorrentClient::new(base_url);

    // 模擬生產事故情境：帳密已注入，但啟動登入從未成功（無 session）
    client.set_credentials("admin", "password").await;

    let results = client
        .add_torrents(vec![DownloadRequestItem {
            url: MAGNET.to_string(),
            save_path: "/downloads".to_string(),
        }])
        .await
        .expect("add_torrents should succeed");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, "accepted");
    assert_eq!(state.login_calls.load(Ordering::SeqCst), 1);
    // 第一次 403 + 重登後重試一次
    assert_eq!(state.add_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn add_torrents_fails_without_credentials() {
    let (base_url, state) = spawn_mock_qbittorrent().await;
    let client = QBittorrentClient::new(base_url);

    let results = client
        .add_torrents(vec![DownloadRequestItem {
            url: MAGNET.to_string(),
            save_path: "/downloads".to_string(),
        }])
        .await
        .expect("add_torrents returns per-item results");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, "failed");
    assert!(
        results[0]
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("403"),
        "失敗原因應包含 403，實際: {:?}",
        results[0].reason
    );
    assert_eq!(state.login_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn query_status_relogins_after_403() {
    let (base_url, state) = spawn_mock_qbittorrent().await;
    let client = QBittorrentClient::new(base_url);
    client.set_credentials("admin", "password").await;

    let hash = "217e6da069a8ee782fb6d2cabe8e438f6293780c".to_string();
    let results = client
        .query_status(vec![hash.clone()])
        .await
        .expect("query_status should succeed after re-login");

    assert_eq!(state.login_calls.load(Ordering::SeqCst), 1);
    // mock 回傳空清單，未回報的 hash 應標記 not_found
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].hash, hash);
    assert_eq!(results[0].status, "not_found");
}
