// tests/integration/relogin_tests.rs
//
// 403 自動重登整合測試：以 axum 模擬 qBittorrent WebUI，
// 驗證真實 QBittorrentClient 在啟動登入失敗（憑證已注入但 session 不存在）時，
// 收到 403 能自動重登並重試成功；以及加入後確認 torrent 真的出現在 qBittorrent。

use axum::{
    extract::{Form, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use downloader_qbittorrent::{DownloaderClient, QBittorrentClient};
use shared::DownloadRequestItem;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

struct MockQbState {
    logged_in: AtomicBool,
    login_calls: AtomicU32,
    add_calls: AtomicU32,
    /// 是否真的把加入的 torrent 放進清單（模擬 qB 抓 .torrent 失敗時為 false）
    registers_added: AtomicBool,
    added: AtomicBool,
    /// 收到的 /torrents/delete 表單
    delete_forms: Mutex<Vec<HashMap<String, String>>>,
    /// 登入回應的版本：qBittorrent 5.2.0 起成功時回 204 沒有內容
    login_style: Mutex<LoginStyle>,
}

#[derive(Clone, Copy)]
enum LoginStyle {
    /// 5.2.0 之前：200 + "Ok."
    Ok200,
    /// 5.2.0 起：204 沒有內容
    NoContent204,
    /// 帳密錯誤：200 + "Fails."
    WrongPassword,
}

async fn mock_login(State(state): State<Arc<MockQbState>>) -> (StatusCode, &'static str) {
    state.login_calls.fetch_add(1, Ordering::SeqCst);
    let style = *state.login_style.lock().unwrap();
    match style {
        LoginStyle::WrongPassword => (StatusCode::OK, "Fails."),
        LoginStyle::Ok200 => {
            state.logged_in.store(true, Ordering::SeqCst);
            (StatusCode::OK, "Ok.")
        }
        LoginStyle::NoContent204 => {
            state.logged_in.store(true, Ordering::SeqCst);
            (StatusCode::NO_CONTENT, "")
        }
    }
}

async fn mock_add(State(state): State<Arc<MockQbState>>) -> (StatusCode, &'static str) {
    state.add_calls.fetch_add(1, Ordering::SeqCst);
    if state.logged_in.load(Ordering::SeqCst) {
        state.added.store(true, Ordering::SeqCst);
        (StatusCode::OK, "Ok.")
    } else {
        (StatusCode::FORBIDDEN, "Forbidden")
    }
}

async fn mock_delete(
    State(state): State<Arc<MockQbState>>,
    Form(form): Form<HashMap<String, String>>,
) -> StatusCode {
    state.delete_forms.lock().unwrap().push(form);
    StatusCode::OK
}

async fn mock_info(State(state): State<Arc<MockQbState>>) -> (StatusCode, String) {
    if !state.logged_in.load(Ordering::SeqCst) {
        return (StatusCode::FORBIDDEN, "Forbidden".to_string());
    }
    if state.added.load(Ordering::SeqCst) && state.registers_added.load(Ordering::SeqCst) {
        let torrent = format!(
            r#"[{{"hash":"{}","name":"t","state":"metaDL","progress":0.0,"dlspeed":0,"size":0,"downloaded":0,"content_path":null}}]"#,
            MAGNET_HASH
        );
        (StatusCode::OK, torrent)
    } else {
        (StatusCode::OK, "[]".to_string())
    }
}

/// 啟動 mock qBittorrent，回傳 (base_url, state)
async fn spawn_mock_qbittorrent() -> (String, Arc<MockQbState>) {
    spawn_mock_qbittorrent_with(true).await
}

async fn spawn_mock_qbittorrent_with(registers_added: bool) -> (String, Arc<MockQbState>) {
    let state = Arc::new(MockQbState {
        logged_in: AtomicBool::new(false),
        login_calls: AtomicU32::new(0),
        add_calls: AtomicU32::new(0),
        registers_added: AtomicBool::new(registers_added),
        added: AtomicBool::new(false),
        delete_forms: Mutex::new(Vec::new()),
        login_style: Mutex::new(LoginStyle::Ok200),
    });

    let app = Router::new()
        .route("/api/v2/auth/login", post(mock_login))
        .route("/api/v2/torrents/add", post(mock_add))
        .route("/api/v2/torrents/info", get(mock_info))
        .route("/api/v2/torrents/delete", post(mock_delete))
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
const MAGNET_HASH: &str = "217e6da069a8ee782fb6d2cabe8e438f6293780c";

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

#[tokio::test]
async fn add_torrents_fails_when_torrent_never_appears() {
    // qB 回 `Ok.`，但 torrent 從未出現在 torrents/info（稽核 E4：555）
    let (base_url, state) = spawn_mock_qbittorrent_with(false).await;
    let client =
        QBittorrentClient::new(base_url).with_add_confirm_timeout(Duration::from_millis(600));
    client.set_credentials("admin", "password").await;

    let results = client
        .add_torrents(vec![DownloadRequestItem {
            url: MAGNET.to_string(),
            save_path: "/downloads".to_string(),
        }])
        .await
        .expect("add_torrents returns per-item results");

    assert_eq!(state.add_calls.load(Ordering::SeqCst), 2);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, "failed");
    assert_eq!(results[0].hash.as_deref(), Some(MAGNET_HASH));
    assert!(
        results[0]
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("did not register"),
        "實際: {:?}",
        results[0].reason
    );
}

#[tokio::test]
async fn cancel_torrents_removes_partial_files() {
    let (base_url, state) = spawn_mock_qbittorrent().await;
    let client = QBittorrentClient::new(base_url);
    client.set_credentials("admin", "password").await;

    let results = client
        .cancel_torrents(vec![MAGNET_HASH.to_string()])
        .await
        .expect("cancel_torrents returns per-hash results");

    assert_eq!(results[0].status, "cancelled");
    let forms = state.delete_forms.lock().unwrap();
    assert_eq!(forms.len(), 1);
    assert_eq!(forms[0]["hashes"], MAGNET_HASH);
    // 稽核 E7：取消後不能在 /downloads 留下沒下載完的檔案
    assert_eq!(forms[0]["deleteFiles"], "true");
}

#[tokio::test]
async fn relogin_works_with_the_empty_204_of_qbittorrent_5_2() {
    // 生產 2026-10-27：qB 5.2.3 重啟後 session 過期，重新登入回 204，舊程式當成失敗
    let (base_url, state) = spawn_mock_qbittorrent().await;
    *state.login_style.lock().unwrap() = LoginStyle::NoContent204;
    let client = QBittorrentClient::new(base_url);
    client.set_credentials("admin", "password").await;

    let results = client
        .add_torrents(vec![DownloadRequestItem {
            url: MAGNET.to_string(),
            save_path: "/downloads".to_string(),
        }])
        .await
        .expect("add_torrents should succeed after a 204 re-login");

    assert_eq!(results[0].status, "accepted");
    assert_eq!(state.login_calls.load(Ordering::SeqCst), 1);
    assert_eq!(state.add_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_login_with_the_204_of_qbittorrent_5_2_succeeds() {
    let (base_url, state) = spawn_mock_qbittorrent().await;
    *state.login_style.lock().unwrap() = LoginStyle::NoContent204;
    let client = QBittorrentClient::new(base_url.clone());
    assert!(client.login("admin", "password").await.is_ok());

    *state.login_style.lock().unwrap() = LoginStyle::WrongPassword;
    let err = QBittorrentClient::new(base_url)
        .login("admin", "wrong")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("wrong username or password"));
}
