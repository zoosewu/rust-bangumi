use fetcher_mikanani::RssParser;
use shared::models::{FetchedAnime, FetchedLink};
use std::sync::Arc;

// ============ Parser Object Creation Tests ============

#[test]
fn test_parser_creates_valid_anime_objects() {
    let parser = RssParser::new();

    // Verify parser can parse titles into anime objects
    let title = "[SubGroup] Test Anime [01][1080p]";
    let result = parser.parse_title_public(title);

    assert!(result.is_some());
    let (anime_title, group, episode) = result.unwrap();
    assert!(!anime_title.is_empty());
    assert!(!group.is_empty());
    assert!(episode > 0);

    // Verify the parsed values match expected format
    assert_eq!(anime_title, "Test Anime");
    assert_eq!(group, "SubGroup");
    assert_eq!(episode, 1);
}

#[test]
fn test_parser_creates_multiple_fetched_link_objects() {
    let parser = RssParser::new();

    let test_cases = vec![
        "[SubGroup] Anime Title [01][1080p]",
        "[SubGroup] Anime Title [02][1080p]",
        "[SubGroup] Anime Title [03][1080p]",
    ];

    let mut links = Vec::new();
    for title in test_cases {
        if let Some((_anime_title, subtitle_group, episode_no)) = parser.parse_title_public(title) {
            let source_hash = parser.generate_hash_public("magnet:?xt=test");

            let link = FetchedLink {
                episode_no,
                subtitle_group,
                title: title.to_string(),
                url: "magnet:?xt=test".to_string(),
                source_hash,
            };

            links.push(link);
        }
    }

    assert_eq!(links.len(), 3);
    assert_eq!(links[0].episode_no, 1);
    assert_eq!(links[1].episode_no, 2);
    assert_eq!(links[2].episode_no, 3);
}

#[test]
fn test_parser_creates_fetched_anime_structure() {
    let parser = RssParser::new();

    let title = "[SubGroup] Test Series [05][1080p]";
    if let Some((anime_title, subtitle_group, episode_no)) = parser.parse_title_public(title) {
        let source_hash = parser.generate_hash_public("magnet:?xt=test");

        let link = FetchedLink {
            episode_no,
            subtitle_group,
            title: title.to_string(),
            url: "magnet:?xt=test".to_string(),
            source_hash,
        };

        let anime = FetchedAnime {
            title: anime_title,
            description: "Test description".to_string(),
            season: "spring".to_string(),
            year: 2025,
            series_no: 1,
            links: vec![link],
        };

        assert_eq!(anime.title, "Test Series");
        assert_eq!(anime.links.len(), 1);
        assert_eq!(anime.links[0].episode_no, 5);
    }
}

// ============ Retry Logic Tests ============

#[test]
fn test_hash_generation_consistency() {
    let parser = RssParser::new();

    // Same URL should always produce same hash
    for _ in 0..100 {
        let hash1 = parser.generate_hash_public("magnet:?xt=test");
        let hash2 = parser.generate_hash_public("magnet:?xt=test");
        assert_eq!(hash1, hash2);
    }
}

#[test]
fn test_hash_generation_uniqueness() {
    let parser = RssParser::new();

    // Different URLs should produce different hashes
    let hash1 = parser.generate_hash_public("magnet:?xt=abc123");
    let hash2 = parser.generate_hash_public("magnet:?xt=def456");
    let hash3 = parser.generate_hash_public("http://example.com/torrent");

    assert_ne!(hash1, hash2);
    assert_ne!(hash2, hash3);
    assert_ne!(hash1, hash3);
}

#[test]
fn test_hash_length_is_sha256() {
    let parser = RssParser::new();
    let hash = parser.generate_hash_public("test_url");

    // SHA256 produces 64 character hex strings
    assert_eq!(hash.len(), 64);
}

// ============ Thread Safety Tests ============

#[test]
fn test_parser_is_thread_safe() {
    let parser = Arc::new(RssParser::new());
    let mut handles = vec![];

    for i in 0..5 {
        let parser_clone = parser.clone();
        let handle = std::thread::spawn(move || {
            let title = format!("[Group{}] Anime [{}][1080p]", i, i);
            parser_clone.parse_title_public(&title)
        });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.join();
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
}

#[test]
fn test_parser_hash_generation_thread_safe() {
    let parser = Arc::new(RssParser::new());
    let mut handles = vec![];

    for i in 0..10 {
        let parser_clone = parser.clone();
        let handle = std::thread::spawn(move || {
            let url = format!("magnet:?xt=test{}", i);
            parser_clone.generate_hash_public(&url)
        });
        handles.push(handle);
    }

    let mut hashes = Vec::new();
    for handle in handles {
        let result = handle.join();
        assert!(result.is_ok());
        hashes.push(result.unwrap());
    }

    // All hashes should be unique (different inputs)
    for i in 0..hashes.len() {
        for j in (i + 1)..hashes.len() {
            assert_ne!(hashes[i], hashes[j]);
        }
    }
}

#[test]
fn test_parser_concurrent_title_parsing() {
    let parser = Arc::new(RssParser::new());
    let mut handles = vec![];

    let titles = vec![
        "[SubGroup] Title1 [01][1080p]",
        "[SubGroup] Title2 [02][1080p]",
        "[SubGroup] Title3 [03][1080p]",
        "[SubGroup] Title4 [04][1080p]",
        "[SubGroup] Title5 [05][1080p]",
    ];

    for title in titles {
        let parser_clone = parser.clone();
        let handle = std::thread::spawn(move || {
            parser_clone.parse_title_public(title)
        });
        handles.push(handle);
    }

    for (idx, handle) in handles.into_iter().enumerate() {
        let result = handle.join();
        assert!(result.is_ok());
        let parse_result = result.unwrap();
        assert!(parse_result.is_some());
        let (_, _, episode) = parse_result.unwrap();
        assert_eq!(episode as usize, idx + 1);
    }
}

// ============ Multiple Title Format Tests ============

#[test]
fn test_parser_handles_multiple_title_formats() {
    let parser = RssParser::new();

    let test_cases = vec![
        "[SubGroup] Anime [01][1080p]",
        "[字幕組] 動畫 第01話 [1080p]",
        "[Group] Title EP01 [720p]",
    ];

    for title in test_cases {
        let result = parser.parse_title_public(title);
        assert!(result.is_some(), "Failed to parse: {}", title);
    }
}

#[test]
fn test_parser_handles_different_resolution_formats() {
    let parser = RssParser::new();

    let test_cases = vec![
        "[SubGroup] Anime [01][1080p]",
        "[SubGroup] Anime [01][720p]",
        "[SubGroup] Anime [01][480p]",
        "[SubGroup] Anime [01][2160p]",
    ];

    for title in test_cases {
        let result = parser.parse_title_public(title);
        assert!(result.is_some(), "Failed to parse: {}", title);
        let (_, _, episode) = result.unwrap();
        assert_eq!(episode, 1);
    }
}

#[test]
fn test_parser_handles_different_subtitle_groups() {
    let parser = RssParser::new();

    let test_cases = vec![
        ("[SubGroup1] Anime [01][1080p]", "SubGroup1"),
        ("[字幕組] Anime [01][1080p]", "字幕組"),
        ("[KeepSubGroup] Anime [01][1080p]", "KeepSubGroup"),
        ("[A-Z_123] Anime [01][1080p]", "A-Z_123"),
    ];

    for (title, expected_group) in test_cases {
        let result = parser.parse_title_public(title);
        assert!(result.is_some(), "Failed to parse: {}", title);
        let (_, group, _) = result.unwrap();
        assert_eq!(group, expected_group);
    }
}

#[test]
fn test_parser_handles_different_episode_formats() {
    let parser = RssParser::new();

    let test_cases = vec![
        ("[SubGroup] Anime [01][1080p]", 1),
        ("[SubGroup] Anime 第05話 [1080p]", 5),
        ("[SubGroup] Anime EP12 [720p]", 12),
        ("[SubGroup] Anime [99][1080p]", 99),
    ];

    for (title, expected_episode) in test_cases {
        let result = parser.parse_title_public(title);
        assert!(result.is_some(), "Failed to parse: {}", title);
        let (_, _, episode) = result.unwrap();
        assert_eq!(episode, expected_episode);
    }
}

// ============ Error Handling Tests ============

#[test]
fn test_parser_handles_invalid_titles() {
    let parser = RssParser::new();

    let invalid_cases = vec![
        "No brackets here",
        "[Group] Title without episode",
        "[] Empty brackets [01]",
    ];

    for title in invalid_cases {
        let _result = parser.parse_title_public(title);
        // Parser might return None for invalid formats, that's acceptable
        // We just verify it doesn't panic
    }
}

#[test]
fn test_parser_handles_malformed_episode_numbers() {
    let parser = RssParser::new();

    // These might fail to parse or parse partially, but should not panic
    let test_cases = vec![
        "[Group] Anime [abc][1080p]",
        "[Group] Anime [01x02][1080p]",
        "[Group] Anime [][1080p]",
    ];

    for title in test_cases {
        // Should not panic regardless
        let _ = parser.parse_title_public(title);
    }
}

// ============ Pipeline Integration Tests ============

#[test]
fn test_full_pipeline_parse_to_fetched_anime() {
    let parser = RssParser::new();

    // Simulate parsing multiple entries
    let entries = vec![
        "[SubGroup] Fantasy Anime [01][1080p]",
        "[SubGroup] Fantasy Anime [02][1080p]",
        "[SubGroup] Fantasy Anime [03][1080p]",
    ];

    let mut anime_map: std::collections::HashMap<String, FetchedAnime> = std::collections::HashMap::new();

    for title in entries {
        if let Some((anime_title, subtitle_group, episode_no)) = parser.parse_title_public(title) {
            let source_hash = parser.generate_hash_public("magnet:?xt=test");

            let fetched_link = FetchedLink {
                episode_no,
                subtitle_group: subtitle_group.clone(),
                title: title.to_string(),
                url: "magnet:?xt=test".to_string(),
                source_hash,
            };

            anime_map
                .entry(anime_title.clone())
                .or_insert_with(|| FetchedAnime {
                    title: anime_title.clone(),
                    description: String::new(),
                    season: "unknown".to_string(),
                    year: 2025,
                    series_no: 1,
                    links: Vec::new(),
                })
                .links
                .push(fetched_link);
        }
    }

    assert_eq!(anime_map.len(), 1);
    let anime = anime_map.values().next().unwrap();
    assert_eq!(anime.links.len(), 3);
    assert_eq!(anime.title, "Fantasy Anime");
}

#[test]
fn test_full_pipeline_multiple_series() {
    let parser = RssParser::new();

    // Simulate parsing entries from multiple series
    let entries = vec![
        "[Group1] Series A [01][1080p]",
        "[Group2] Series B [01][1080p]",
        "[Group1] Series A [02][1080p]",
        "[Group2] Series B [02][1080p]",
        "[Group1] Series A [03][1080p]",
    ];

    let mut anime_map: std::collections::HashMap<String, FetchedAnime> = std::collections::HashMap::new();

    for title in entries {
        if let Some((anime_title, subtitle_group, episode_no)) = parser.parse_title_public(title) {
            let source_hash = parser.generate_hash_public("magnet:?xt=test");

            let fetched_link = FetchedLink {
                episode_no,
                subtitle_group: subtitle_group.clone(),
                title: title.to_string(),
                url: format!("magnet:?xt=test{}", episode_no),
                source_hash,
            };

            anime_map
                .entry(anime_title.clone())
                .or_insert_with(|| FetchedAnime {
                    title: anime_title.clone(),
                    description: String::new(),
                    season: "unknown".to_string(),
                    year: 2025,
                    series_no: 1,
                    links: Vec::new(),
                })
                .links
                .push(fetched_link);
        }
    }

    assert_eq!(anime_map.len(), 2);

    // Verify Series A has 3 episodes
    let series_a = anime_map.get("Series A").unwrap();
    assert_eq!(series_a.links.len(), 3);

    // Verify Series B has 2 episodes
    let series_b = anime_map.get("Series B").unwrap();
    assert_eq!(series_b.links.len(), 2);
}

#[test]
fn test_fetched_link_object_completeness() {
    let parser = RssParser::new();

    let title = "[MyGroup] Great Anime [15][1080p]";
    if let Some((_anime_title, subtitle_group, episode_no)) = parser.parse_title_public(title) {
        let url = "magnet:?xt=urn:btih:abc123def456";
        let source_hash = parser.generate_hash_public(url);

        let link = FetchedLink {
            episode_no,
            subtitle_group: subtitle_group.clone(),
            title: title.to_string(),
            url: url.to_string(),
            source_hash: source_hash.clone(),
        };

        // Verify all fields are populated
        assert_eq!(link.episode_no, 15);
        assert_eq!(link.subtitle_group, "MyGroup");
        assert_eq!(link.title, title);
        assert_eq!(link.url, url);
        assert_eq!(link.source_hash.len(), 64); // SHA256 hex length
    }
}

#[test]
fn test_hash_consistency_in_pipeline() {
    let parser = RssParser::new();

    let url = "magnet:?xt=urn:btih:consistent";

    // Parse title
    let title = "[Group] Anime [05][1080p]";
    if let Some((_, _, episode_no)) = parser.parse_title_public(title) {
        // Generate hash multiple times for same URL
        let hash1 = parser.generate_hash_public(url);
        let hash2 = parser.generate_hash_public(url);
        let hash3 = parser.generate_hash_public(url);

        // All hashes should be identical
        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);

        // Create multiple links with same URL, verify hashes match
        let link1 = FetchedLink {
            episode_no,
            subtitle_group: "Group".to_string(),
            title: title.to_string(),
            url: url.to_string(),
            source_hash: hash1.clone(),
        };

        let link2 = FetchedLink {
            episode_no,
            subtitle_group: "Group".to_string(),
            title: title.to_string(),
            url: url.to_string(),
            source_hash: hash2.clone(),
        };

        assert_eq!(link1.source_hash, link2.source_hash);
    }
}
