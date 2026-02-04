// tests/unit/hash_extraction_tests.rs
use downloader_qbittorrent::QBittorrentClient;

fn create_client() -> QBittorrentClient {
    QBittorrentClient::new("http://localhost:8080".to_string())
}

// ============ Valid Format Tests ============

#[test]
fn test_extract_hash_from_valid_magnet() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:1234567890abcdef1234567890abcdef&dn=test&tr=http://tracker.example.com";

    let hash = client.extract_hash_from_magnet(magnet).unwrap();
    assert_eq!(hash, "1234567890abcdef1234567890abcdef");
}

#[test]
fn test_extract_hash_with_uppercase_converts_to_lowercase() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:ABCDEFABCDEFABCDEFABCDEFABCDEFAB&dn=test";

    let hash = client.extract_hash_from_magnet(magnet).unwrap();
    assert_eq!(hash, "abcdefabcdefabcdefabcdefabcdefab");
}

#[test]
fn test_extract_hash_without_tracker_params() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:11111111111111111111111111111111";

    let hash = client.extract_hash_from_magnet(magnet).unwrap();
    assert_eq!(hash, "11111111111111111111111111111111");
}

#[test]
fn test_extract_hash_with_multiple_trackers() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:22222222222222222222222222222222&tr=http://t1.com&tr=http://t2.com&tr=udp://t3.com";

    let hash = client.extract_hash_from_magnet(magnet).unwrap();
    assert_eq!(hash, "22222222222222222222222222222222");
}

// ============ Invalid Format Tests ============

#[test]
fn test_extract_hash_invalid_url_no_btih() {
    let client = create_client();
    let result = client.extract_hash_from_magnet("magnet:?dn=test&tr=http://tracker.com");

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid magnet URL"));
}

#[test]
fn test_extract_hash_empty_string() {
    let client = create_client();
    let result = client.extract_hash_from_magnet("");

    assert!(result.is_err());
}

#[test]
fn test_extract_hash_short_hash_rejected() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:short&dn=test";
    let result = client.extract_hash_from_magnet(magnet);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid hash"));
}

#[test]
fn test_extract_hash_non_magnet_protocol() {
    let client = create_client();
    let result = client.extract_hash_from_magnet("http://example.com/file.torrent");

    assert!(result.is_err());
}

// ============ Consistency Tests ============

#[test]
fn test_extract_hash_idempotent() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:consistenthash123456789012345678&dn=test";

    let hash1 = client.extract_hash_from_magnet(magnet).unwrap();
    let hash2 = client.extract_hash_from_magnet(magnet).unwrap();

    assert_eq!(hash1, hash2);
}
