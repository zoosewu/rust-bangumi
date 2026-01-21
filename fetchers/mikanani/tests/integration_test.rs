use fetcher_mikanani::RssParser;

#[test]
fn test_parse_title_standard_format() {
    let parser = RssParser::new();

    let title = "[SubGroup] Test Anime [01][1080p]";
    let result = parser.parse_title_public(title);

    assert!(result.is_some());
    let (anime_title, group, episode) = result.unwrap();
    assert_eq!(anime_title, "Test Anime");
    assert_eq!(group, "SubGroup");
    assert_eq!(episode, 1);
}

#[test]
fn test_rss_parser_methods_exist() {
    let parser = RssParser::new();

    // This test verifies that RssParser is a real, usable struct
    let hash = parser.generate_hash_public("test_url");
    assert!(!hash.is_empty());
    assert_eq!(hash.len(), 64);
}
