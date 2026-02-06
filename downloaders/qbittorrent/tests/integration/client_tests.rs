// tests/integration/client_tests.rs
use anyhow::anyhow;
use downloader_qbittorrent::{DownloaderClient, MockDownloaderClient};
use shared::{CancelResultItem, DownloadRequestItem, DownloadResultItem, DownloadStatusItem};

// ============ Login Tests ============

#[tokio::test]
async fn test_login_success() {
    let mock = MockDownloaderClient::new().with_login_result(Ok(()));

    let result = mock.login("admin", "password").await;

    assert!(result.is_ok());
    assert_eq!(mock.login_calls.borrow().len(), 1);
    assert_eq!(
        mock.login_calls.borrow()[0],
        ("admin".to_string(), "password".to_string())
    );
}

#[tokio::test]
async fn test_login_wrong_credentials_returns_error() {
    let mock = MockDownloaderClient::new().with_login_result(Err(anyhow!("Invalid credentials")));

    let result = mock.login("admin", "wrong").await;

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid credentials"));
}

#[tokio::test]
async fn test_login_connection_failed_returns_error() {
    let mock = MockDownloaderClient::new().with_login_result(Err(anyhow!("Connection refused")));

    let result = mock.login("admin", "password").await;

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Connection refused"));
}

// ============ Add Torrents (Batch) Tests ============

#[tokio::test]
async fn test_add_torrents_success_returns_results() {
    let expected = vec![DownloadResultItem {
        url: "magnet:?xt=urn:btih:abc123def456".to_string(),
        hash: Some("abc123def456".to_string()),
        status: "accepted".to_string(),
        reason: None,
    }];

    let mock = MockDownloaderClient::new().with_add_torrents_result(Ok(expected.clone()));

    let items = vec![DownloadRequestItem {
        url: "magnet:?xt=urn:btih:abc123def456".to_string(),
        save_path: "/downloads".to_string(),
    }];

    let result = mock.add_torrents(items).await;

    assert!(result.is_ok());
    let results = result.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, "accepted");
    assert_eq!(results[0].hash, Some("abc123def456".to_string()));
}

#[tokio::test]
async fn test_add_torrents_records_call_parameters() {
    let mock = MockDownloaderClient::new();

    let items = vec![
        DownloadRequestItem {
            url: "magnet:?xt=urn:btih:hash1".to_string(),
            save_path: "/path1".to_string(),
        },
        DownloadRequestItem {
            url: "magnet:?xt=urn:btih:hash2".to_string(),
            save_path: "/path2".to_string(),
        },
    ];

    let _ = mock.add_torrents(items).await;

    let calls = mock.add_torrents_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].len(), 2);
    assert_eq!(calls[0][0].url, "magnet:?xt=urn:btih:hash1");
    assert_eq!(calls[0][1].save_path, "/path2");
}

#[tokio::test]
async fn test_add_torrents_error_propagates() {
    let mock = MockDownloaderClient::new()
        .with_add_torrents_result(Err(anyhow!("Connection timeout")));

    let items = vec![DownloadRequestItem {
        url: "magnet:?xt=urn:btih:hash1".to_string(),
        save_path: "/downloads".to_string(),
    }];

    let result = mock.add_torrents(items).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Connection timeout"));
}

// ============ Cancel Torrents (Batch) Tests ============

#[tokio::test]
async fn test_cancel_torrents_success() {
    let expected = vec![CancelResultItem {
        hash: "abc123".to_string(),
        status: "cancelled".to_string(),
    }];

    let mock = MockDownloaderClient::new().with_cancel_torrents_result(Ok(expected));

    let result = mock.cancel_torrents(vec!["abc123".to_string()]).await;

    assert!(result.is_ok());
    let results = result.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, "cancelled");
}

#[tokio::test]
async fn test_cancel_torrents_records_hashes() {
    let mock = MockDownloaderClient::new();

    let _ = mock
        .cancel_torrents(vec!["hash1".to_string(), "hash2".to_string()])
        .await;

    let calls = mock.cancel_torrents_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], vec!["hash1".to_string(), "hash2".to_string()]);
}

// ============ Query Status Tests ============

#[tokio::test]
async fn test_query_status_returns_statuses() {
    let expected = vec![DownloadStatusItem {
        hash: "hash1".to_string(),
        status: "downloading".to_string(),
        progress: 0.5,
        size: 1000000,
    }];

    let mock = MockDownloaderClient::new().with_query_status_result(Ok(expected));

    let result = mock.query_status(vec!["hash1".to_string()]).await;

    assert!(result.is_ok());
    let statuses = result.unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].hash, "hash1");
    assert_eq!(statuses[0].progress, 0.5);
}

#[tokio::test]
async fn test_query_status_records_hashes() {
    let mock = MockDownloaderClient::new();

    let _ = mock
        .query_status(vec!["h1".to_string(), "h2".to_string()])
        .await;

    let calls = mock.query_status_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], vec!["h1".to_string(), "h2".to_string()]);
}

// ============ Pause / Resume / Delete Tests ============

#[tokio::test]
async fn test_pause_torrent_success() {
    let mock = MockDownloaderClient::new().with_pause_result(Ok(()));

    let result = mock.pause_torrent("pausehash").await;

    assert!(result.is_ok());
    assert_eq!(mock.pause_calls.borrow()[0], "pausehash");
}

#[tokio::test]
async fn test_pause_torrent_not_found_error() {
    let mock = MockDownloaderClient::new().with_pause_result(Err(anyhow!("Torrent not found")));

    let result = mock.pause_torrent("nonexistent").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_resume_torrent_success() {
    let mock = MockDownloaderClient::new().with_resume_result(Ok(()));

    let result = mock.resume_torrent("resumehash").await;

    assert!(result.is_ok());
    assert_eq!(mock.resume_calls.borrow()[0], "resumehash");
}

#[tokio::test]
async fn test_delete_torrent_with_files() {
    let mock = MockDownloaderClient::new().with_delete_result(Ok(()));

    let result = mock.delete_torrent("deletehash", true).await;

    assert!(result.is_ok());
    let calls = mock.delete_calls.borrow();
    assert_eq!(calls[0], ("deletehash".to_string(), true));
}

#[tokio::test]
async fn test_delete_torrent_without_files() {
    let mock = MockDownloaderClient::new().with_delete_result(Ok(()));

    let result = mock.delete_torrent("deletehash", false).await;

    assert!(result.is_ok());
    let calls = mock.delete_calls.borrow();
    assert_eq!(calls[0], ("deletehash".to_string(), false));
}
