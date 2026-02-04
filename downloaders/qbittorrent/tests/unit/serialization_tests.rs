// tests/unit/serialization_tests.rs
use downloader_qbittorrent::TorrentInfo;

// ============ TorrentInfo Tests ============

#[test]
fn test_torrent_info_serialize_json() {
    let info = TorrentInfo {
        hash: "abc123".to_string(),
        name: "Test".to_string(),
        state: "downloading".to_string(),
        progress: 0.5,
        dlspeed: 1024000,
        size: 1000000000,
        downloaded: 500000000,
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"hash\":\"abc123\""));
    assert!(json.contains("\"progress\":0.5"));
}

#[test]
fn test_torrent_info_deserialize_json() {
    let json = r#"{
        "hash": "def456",
        "name": "Deserialized",
        "state": "completed",
        "progress": 1.0,
        "dlspeed": 0,
        "size": 500000000,
        "downloaded": 500000000
    }"#;

    let info: TorrentInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.hash, "def456");
    assert_eq!(info.state, "completed");
    assert_eq!(info.progress, 1.0);
}

#[test]
fn test_torrent_info_all_states() {
    let states = vec![
        "downloading",
        "uploading",
        "paused",
        "completed",
        "error",
        "stalledDL",
        "stalledUP",
    ];

    for state in states {
        let info = TorrentInfo {
            hash: "test".to_string(),
            name: "Torrent".to_string(),
            state: state.to_string(),
            progress: 1.0,
            dlspeed: 0,
            size: 100000,
            downloaded: 100000,
        };

        assert_eq!(info.state, state);
    }
}

#[test]
fn test_torrent_info_progress_boundaries() {
    // Test 0.0
    let info_zero = TorrentInfo {
        hash: "zero".to_string(),
        name: "Zero Progress".to_string(),
        state: "downloading".to_string(),
        progress: 0.0,
        dlspeed: 1024,
        size: 1000000,
        downloaded: 0,
    };
    assert_eq!(info_zero.progress, 0.0);

    // Test 1.0
    let info_full = TorrentInfo {
        hash: "full".to_string(),
        name: "Full Progress".to_string(),
        state: "completed".to_string(),
        progress: 1.0,
        dlspeed: 0,
        size: 1000000,
        downloaded: 1000000,
    };
    assert_eq!(info_full.progress, 1.0);
}

// ============ DownloadRequest Tests ============

#[test]
fn test_download_request_deserialize() {
    #[derive(serde::Deserialize)]
    struct DownloadRequest {
        link_id: i32,
        url: String,
    }

    let json = r#"{"link_id": 123, "url": "magnet:?xt=urn:btih:abc123"}"#;
    let req: DownloadRequest = serde_json::from_str(json).unwrap();

    assert_eq!(req.link_id, 123);
    assert_eq!(req.url, "magnet:?xt=urn:btih:abc123");
}

#[test]
fn test_download_request_missing_field_error() {
    #[derive(serde::Deserialize)]
    struct DownloadRequest {
        link_id: i32,
        url: String,
    }

    let json = r#"{"link_id": 123}"#;
    let result: Result<DownloadRequest, _> = serde_json::from_str(json);

    assert!(result.is_err());
}

// ============ DownloadResponse Tests ============

#[test]
fn test_download_response_accepted() {
    #[derive(serde::Serialize)]
    struct DownloadResponse {
        status: String,
        hash: Option<String>,
        error: Option<String>,
    }

    let response = DownloadResponse {
        status: "accepted".to_string(),
        hash: Some("def456".to_string()),
        error: None,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"status\":\"accepted\""));
    assert!(json.contains("\"hash\":\"def456\""));
    assert!(json.contains("\"error\":null"));
}

#[test]
fn test_download_response_error() {
    #[derive(serde::Serialize)]
    struct DownloadResponse {
        status: String,
        hash: Option<String>,
        error: Option<String>,
    }

    let response = DownloadResponse {
        status: "error".to_string(),
        hash: None,
        error: Some("Connection failed".to_string()),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"status\":\"error\""));
    assert!(json.contains("\"hash\":null"));
    assert!(json.contains("\"error\":\"Connection failed\""));
}

#[test]
fn test_download_response_unsupported() {
    #[derive(serde::Serialize)]
    struct DownloadResponse {
        status: String,
        hash: Option<String>,
        error: Option<String>,
    }

    let response = DownloadResponse {
        status: "unsupported".to_string(),
        hash: None,
        error: Some("Only magnet links supported".to_string()),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"status\":\"unsupported\""));
}
