// tests/integration/client_tests.rs
use anyhow::anyhow;
use downloader_qbittorrent::{DownloaderClient, MockDownloaderClient, TorrentInfo};

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

// ============ Add Magnet Tests ============

#[tokio::test]
async fn test_add_magnet_success_returns_hash() {
    let mock = MockDownloaderClient::new().with_add_magnet_result(Ok("abc123def456".to_string()));

    let result = mock
        .add_magnet("magnet:?xt=urn:btih:abc123def456", None)
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "abc123def456");
}

#[tokio::test]
async fn test_add_magnet_with_save_path() {
    let mock = MockDownloaderClient::new().with_add_magnet_result(Ok("hash123".to_string()));

    let result = mock
        .add_magnet("magnet:?xt=urn:btih:hash123", Some("/downloads"))
        .await;

    assert!(result.is_ok());
    let calls = mock.add_magnet_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, Some("/downloads".to_string()));
}

#[tokio::test]
async fn test_add_magnet_duplicate_torrent_error() {
    let mock =
        MockDownloaderClient::new().with_add_magnet_result(Err(anyhow!("Torrent already exists")));

    let result = mock.add_magnet("magnet:?xt=urn:btih:existing", None).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[tokio::test]
async fn test_add_magnet_records_call_parameters() {
    let mock = MockDownloaderClient::new();
    let magnet = "magnet:?xt=urn:btih:recordtest123456789012345678901234";

    let _ = mock.add_magnet(magnet, Some("/path")).await;

    let calls = mock.add_magnet_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, magnet);
    assert_eq!(calls[0].1, Some("/path".to_string()));
}

// ============ Get Torrent Info Tests ============

#[tokio::test]
async fn test_get_torrent_info_found() {
    let info = TorrentInfo {
        hash: "testhash123".to_string(),
        name: "Test Torrent".to_string(),
        state: "downloading".to_string(),
        progress: 0.5,
        dlspeed: 1024000,
        size: 1000000000,
        downloaded: 500000000,
    };

    let mock = MockDownloaderClient::new().with_get_torrent_info_result(Ok(Some(info.clone())));

    let result = mock.get_torrent_info("testhash123").await;

    assert!(result.is_ok());
    let returned_info = result.unwrap().unwrap();
    assert_eq!(returned_info.hash, "testhash123");
    assert_eq!(returned_info.progress, 0.5);
}

#[tokio::test]
async fn test_get_torrent_info_not_found_returns_none() {
    let mock = MockDownloaderClient::new().with_get_torrent_info_result(Ok(None));

    let result = mock.get_torrent_info("nonexistent").await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_get_torrent_info_records_hash() {
    let mock = MockDownloaderClient::new();

    let _ = mock.get_torrent_info("queryhash").await;

    assert_eq!(mock.get_torrent_info_calls.borrow()[0], "queryhash");
}

// ============ Get All Torrents Tests ============

#[tokio::test]
async fn test_get_all_torrents_returns_list() {
    let torrents = vec![
        TorrentInfo {
            hash: "hash1".to_string(),
            name: "Torrent 1".to_string(),
            state: "downloading".to_string(),
            progress: 0.3,
            dlspeed: 500000,
            size: 500000000,
            downloaded: 150000000,
        },
        TorrentInfo {
            hash: "hash2".to_string(),
            name: "Torrent 2".to_string(),
            state: "completed".to_string(),
            progress: 1.0,
            dlspeed: 0,
            size: 200000000,
            downloaded: 200000000,
        },
    ];

    let mock = MockDownloaderClient::new().with_get_all_torrents_result(Ok(torrents));

    let result = mock.get_all_torrents().await;

    assert!(result.is_ok());
    let list = result.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].hash, "hash1");
    assert_eq!(list[1].hash, "hash2");
}

#[tokio::test]
async fn test_get_all_torrents_empty_list() {
    let mock = MockDownloaderClient::new().with_get_all_torrents_result(Ok(vec![]));

    let result = mock.get_all_torrents().await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
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

// ============ Extract Hash Tests ============

#[tokio::test]
async fn test_extract_hash_from_valid_magnet() {
    let mock = MockDownloaderClient::new();
    let magnet = "magnet:?xt=urn:btih:1234567890abcdef1234567890abcdef&dn=test";

    let result = mock.extract_hash_from_magnet(magnet);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "1234567890abcdef1234567890abcdef");
}

#[tokio::test]
async fn test_extract_hash_invalid_url() {
    let mock = MockDownloaderClient::new();

    let result = mock.extract_hash_from_magnet("not_a_magnet");

    assert!(result.is_err());
}
