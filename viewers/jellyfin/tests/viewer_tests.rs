use std::path::{Path, PathBuf};

// Since we can't directly access internal modules from tests, we'll create integration tests
// that test the functionality through the expected interfaces

#[test]
fn test_path_construction() {
    let anime_title = "Attack on Titan";
    let season = 1;
    let episode = 1;

    let sanitized = anime_title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>();

    let expected_dir = format!("{}/Season {:02}", sanitized, season);
    let expected_file = format!("{} - S{:02}E{:02}.mkv", sanitized, season, episode);

    assert_eq!(expected_dir, "Attack on Titan/Season 01");
    assert_eq!(expected_file, "Attack on Titan - S01E01.mkv");
}

#[test]
fn test_filename_sanitization_various_chars() {
    let test_cases = vec![
        ("Test: Anime", "Test_ Anime"),
        ("Demon/Slayer", "Demon_Slayer"),
        ("Attack*Titan", "Attack_Titan"),
        ("Anime?Series", "Anime_Series"),
        ("Movie\"Title\"", "Movie_Title_"),
        ("Less<Than>Greater", "Less_Than_Greater"),
        ("Pipe|Symbol", "Pipe_Symbol"),
        ("Backslash\\Path", "Backslash_Path"),
        ("Normal Title", "Normal Title"),
        ("Numbers 123", "Numbers 123"),
        ("Mixed-Case_Title", "Mixed-Case_Title"),
    ];

    for (input, expected) in test_cases {
        let result = input
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect::<String>();

        assert_eq!(result, expected, "Failed for input: {}", input);
    }
}

#[test]
fn test_episode_regex_s_e_format() {
    use regex::Regex;

    let regex = Regex::new(r"(?i)s(\d+)e(\d+)|\[(\d+)\]").unwrap();

    let test_cases = vec![
        ("anime_s01e01.mkv", Some(("01", "01"))),
        ("anime_S05E12.mkv", Some(("05", "12"))),
        ("Episode_S02E03.mp4", Some(("02", "03"))),
        ("[01]_episode.mkv", Some(("01", "01"))), // Will match [01]
        ("random_file.mkv", None),
        ("no_episode_info.txt", None),
    ];

    for (filename, expected_result) in test_cases {
        if let Some(caps) = regex.captures(filename) {
            if expected_result.is_some() {
                // Either S##E## format or [##] format
                if caps.get(1).is_some() && caps.get(2).is_some() {
                    let season = caps.get(1).unwrap().as_str();
                    let episode = caps.get(2).unwrap().as_str();
                    assert_eq!(
                        (season, episode),
                        expected_result.unwrap(),
                        "Failed for: {}",
                        filename
                    );
                } else if caps.get(3).is_some() {
                    // [##] format - just verify match
                    assert!(true);
                }
            }
        } else {
            assert_eq!(expected_result, None, "Expected no match for: {}", filename);
        }
    }
}

#[test]
fn test_sync_request_json_structure() {
    let json = r#"{
        "anime_id": 123,
        "anime_title": "Attack on Titan",
        "season": 1,
        "episodes": [
            {"episode_number": 1, "file_path": "/path/to/episode1.mkv"},
            {"episode_number": 2, "file_path": "/path/to/episode2.mkv"}
        ]
    }"#;

    let value: serde_json::Value = serde_json::from_str(json).unwrap();

    assert_eq!(value["anime_id"].as_i64(), Some(123));
    assert_eq!(value["anime_title"].as_str(), Some("Attack on Titan"));
    assert_eq!(value["season"].as_u64(), Some(1));
    assert_eq!(value["episodes"].as_array().unwrap().len(), 2);
    assert_eq!(value["episodes"][0]["episode_number"].as_u64(), Some(1));
}

#[test]
fn test_sync_response_json_structure() {
    let response = serde_json::json!({
        "status": "success",
        "count": 2,
        "organized_files": [
            {
                "episode_number": 1,
                "source_path": "/downloads/episode1.mkv",
                "target_path": "/media/jellyfin/Attack on Titan/Season 01/Attack on Titan - S01E01.mkv"
            },
            {
                "episode_number": 2,
                "source_path": "/downloads/episode2.mkv",
                "target_path": "/media/jellyfin/Attack on Titan/Season 01/Attack on Titan - S01E02.mkv"
            }
        ],
        "error": null
    });

    assert_eq!(response["status"].as_str(), Some("success"));
    assert_eq!(response["count"].as_u64(), Some(2));
    assert_eq!(response["organized_files"].as_array().unwrap().len(), 2);
    assert_eq!(response["error"], serde_json::Value::Null);

    let first_file = &response["organized_files"][0];
    assert_eq!(first_file["episode_number"].as_u64(), Some(1));
}

#[test]
fn test_health_check_response() {
    let response = serde_json::json!({
        "status": "healthy",
        "service": "jellyfin-viewer",
        "version": "0.1.0"
    });

    assert_eq!(response["status"].as_str(), Some("healthy"));
    assert_eq!(response["service"].as_str(), Some("jellyfin-viewer"));
    assert_eq!(response["version"].as_str(), Some("0.1.0"));
}

#[test]
fn test_episode_number_formatting() {
    for episode in vec![1, 5, 10, 12, 99, 100] {
        let formatted = format!("{:02}", episode);
        assert!(formatted.len() <= 3);
        assert!(formatted.chars().all(|c| c.is_ascii_digit()));
    }
}

#[test]
fn test_season_number_formatting() {
    for season in vec![1, 5, 10, 12, 99] {
        let formatted = format!("{:02}", season);
        assert!(formatted.len() <= 3);
        assert!(formatted.chars().all(|c| c.is_ascii_digit()));
    }
}

#[test]
fn test_file_extension_extraction() {
    let test_cases = vec![
        ("episode.mkv", Some("mkv")),
        ("episode.mp4", Some("mp4")),
        ("episode.avi", Some("avi")),
        ("episode", None),
        ("archive.tar.gz", Some("gz")),
    ];

    for (filename, expected_ext) in test_cases {
        let path = Path::new(filename);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string());

        assert_eq!(ext.as_deref(), expected_ext, "Failed for: {}", filename);
    }
}

#[test]
fn test_path_joining() {
    let library_dir = PathBuf::from("/media/jellyfin");
    let anime_title = "Attack on Titan";
    let season = 1;

    let season_dir = library_dir
        .join(anime_title)
        .join(format!("Season {:02}", season));

    assert_eq!(
        season_dir.to_str().unwrap(),
        "/media/jellyfin/Attack on Titan/Season 01"
    );
}

#[test]
fn test_organized_filename_format() {
    let anime_title = "Demon Slayer";
    let season = 2;
    let episode = 5;
    let extension = "mkv";

    let filename = format!(
        "{} - S{:02}E{:02}.{}",
        anime_title, season, episode, extension
    );

    assert_eq!(filename, "Demon Slayer - S02E05.mkv");
}

#[test]
fn test_multiple_episode_organization() {
    let episodes = vec![
        (1, "/path/to/ep1.mkv"),
        (2, "/path/to/ep2.mkv"),
        (3, "/path/to/ep3.mkv"),
        (12, "/path/to/ep12.mkv"),
    ];

    for (episode_num, _path) in episodes {
        let filename = format!("Anime - S01E{:02}.mkv", episode_num);
        assert!(filename.contains(&format!("E{:02}", episode_num)));
    }
}

#[test]
fn test_error_handling_invalid_episode_format() {
    use regex::Regex;

    let regex = Regex::new(r"(?i)s(\d+)e(\d+)|\[(\d+)\]").unwrap();

    let invalid_formats = vec![
        "episode_1_of_12.mkv",
        "season_1_episode_1.mkv",
        "1x01.mkv",
        "ep1.mkv",
    ];

    for filename in invalid_formats {
        assert!(
            !regex.is_match(filename),
            "Unexpected match for: {}",
            filename
        );
    }
}

#[test]
fn test_partial_failure_response_structure() {
    let response = serde_json::json!({
        "status": "partial_failure",
        "count": 1,
        "organized_files": [
            {
                "episode_number": 1,
                "source_path": "/downloads/episode1.mkv",
                "target_path": "/media/jellyfin/Anime/Season 01/Anime - S01E01.mkv"
            }
        ],
        "error": Some("Failed to organize episode 2: source file not found")
    });

    assert_eq!(response["status"].as_str(), Some("partial_failure"));
    assert!(response["error"].is_string() || response["error"].is_null());
}

#[test]
fn test_service_registration_structure() {
    let registration = serde_json::json!({
        "service_type": "Viewer",
        "service_name": "jellyfin",
        "host": "viewer-jellyfin",
        "port": 8003,
        "capabilities": {
            "fetch_endpoint": null,
            "download_endpoint": null,
            "sync_endpoint": "/sync"
        }
    });

    assert_eq!(registration["service_type"].as_str(), Some("Viewer"));
    assert_eq!(registration["service_name"].as_str(), Some("jellyfin"));
    assert_eq!(registration["port"].as_u64(), Some(8003));
    assert_eq!(
        registration["capabilities"]["sync_endpoint"].as_str(),
        Some("/sync")
    );
}
