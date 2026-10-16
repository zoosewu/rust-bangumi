// tests/unit/add_confirm_tests.rs
//
// 加入後確認：只有 torrent 真的出現在 qBittorrent 才算 accepted（稽核 E4）。

use downloader_qbittorrent::reject_unregistered;
use shared::DownloadResultItem;
use std::collections::HashSet;
use std::time::Duration;

fn accepted(hash: &str) -> DownloadResultItem {
    DownloadResultItem {
        url: format!("https://example.com/{hash}.torrent"),
        hash: Some(hash.to_string()),
        status: "accepted".to_string(),
        reason: None,
    }
}

#[test]
fn unregistered_accepted_items_become_failed() {
    let unregistered: HashSet<String> = ["bbb".to_string()].into();
    let results = reject_unregistered(
        vec![accepted("aaa"), accepted("bbb")],
        &unregistered,
        Duration::from_secs(15),
    );

    assert_eq!(results[0].status, "accepted");
    assert_eq!(results[1].status, "failed");
    assert_eq!(results[1].hash.as_deref(), Some("bbb"));
    assert_eq!(
        results[1].reason.as_deref(),
        Some("qBittorrent did not register the torrent within 15s")
    );
}

#[test]
fn already_failed_items_keep_their_reason() {
    let failed = DownloadResultItem {
        url: "https://example.com/x.torrent".to_string(),
        hash: None,
        status: "failed".to_string(),
        reason: Some("qBittorrent returned 403".to_string()),
    };
    let results = reject_unregistered(
        vec![failed],
        &HashSet::from(["x".to_string()]),
        Duration::from_secs(15),
    );

    assert_eq!(
        results[0].reason.as_deref(),
        Some("qBittorrent returned 403")
    );
}
