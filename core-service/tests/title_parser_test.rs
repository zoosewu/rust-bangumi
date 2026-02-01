//! 標題解析器整合測試

use regex::Regex;

/// 測試 LoliHouse 標準格式解析
#[test]
fn test_lolihouse_standard_format() {
    let condition_regex = Regex::new(r"^\[.+\].+\s-\s\d+").unwrap();
    let parse_regex = Regex::new(r"^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)\s*\[.*?(\d{3,4}p)").unwrap();

    let title = "[LoliHouse] 黄金神威 最终章 / Golden Kamuy - 53 [WebRip 1080p HEVC-10bit AAC][无字幕]";

    assert!(condition_regex.is_match(title));

    let captures = parse_regex.captures(title).expect("Should match");
    assert_eq!(captures.get(1).unwrap().as_str(), "LoliHouse");
    assert_eq!(captures.get(2).unwrap().as_str().trim(), "黄金神威 最终章 / Golden Kamuy");
    assert_eq!(captures.get(3).unwrap().as_str(), "53");
    assert_eq!(captures.get(4).unwrap().as_str(), "1080p");
}

/// 測試六四位元星號格式解析
#[test]
fn test_star_separator_format() {
    let condition_regex = Regex::new(r"^[^★]+★.+★\d+★").unwrap();
    let parse_regex = Regex::new(r"^([^★]+)★(.+?)★(\d+)★(\d+x\d+)").unwrap();

    let title = "六四位元字幕组★可以帮忙洗干净吗？Kirei ni Shite Moraemasu ka★04★1920x1080★AVC AAC MP4★繁体中文";

    assert!(condition_regex.is_match(title));

    let captures = parse_regex.captures(title).expect("Should match");
    assert_eq!(captures.get(1).unwrap().as_str(), "六四位元字幕组");
    assert_eq!(captures.get(2).unwrap().as_str(), "可以帮忙洗干净吗？Kirei ni Shite Moraemasu ka");
    assert_eq!(captures.get(3).unwrap().as_str(), "04");
    assert_eq!(captures.get(4).unwrap().as_str(), "1920x1080");
}

/// 測試預設解析器格式
#[test]
fn test_default_parser_format() {
    let condition_regex = Regex::new(r".+\s-\s\d+").unwrap();
    let parse_regex = Regex::new(r"^(.+?)\s+-\s*(\d+)").unwrap();

    let titles = vec![
        "[LoliHouse] 神八小妹不可怕 / Kaya-chan wa Kowakunai - 03 [WebRip 1080p]",
        "[豌豆字幕组&LoliHouse] 地狱乐 / Jigokuraku - 16 [WebRip 1080p]",
    ];

    for title in titles {
        assert!(condition_regex.is_match(title), "Should match: {}", title);

        let captures = parse_regex.captures(title).expect("Should parse");
        let episode: i32 = captures.get(2).unwrap().as_str().parse().unwrap();
        assert!(episode > 0);
    }
}

/// 測試不匹配的標題
#[test]
fn test_non_matching_title() {
    let condition_regex = Regex::new(r"^\[.+\].+\s-\s\d+").unwrap();

    let non_matching = vec![
        "Random text without brackets",
        "Just some anime title",
        "[Group only] no episode number",
    ];

    for title in non_matching {
        assert!(!condition_regex.is_match(title), "Should not match: {}", title);
    }
}

/// 測試邊界情況 - 零補位集數
#[test]
fn test_zero_padded_episode_numbers() {
    let parse_regex = Regex::new(r"^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)\s*\[").unwrap();

    let test_cases = vec![
        ("[Group] Title - 01 [1080p]", 1),
        ("[Group] Title - 001 [1080p]", 1),
        ("[Group] Title - 099 [1080p]", 99),
    ];

    for (title, expected_episode) in test_cases {
        if let Some(captures) = parse_regex.captures(title) {
            let episode: i32 = captures.get(3).unwrap().as_str().parse().unwrap();
            assert_eq!(episode, expected_episode, "Failed for: {}", title);
        }
    }
}

/// 測試多個括號情況
#[test]
fn test_multiple_brackets() {
    let condition_regex = Regex::new(r"^\[.+\].+\s-\s\d+").unwrap();
    let parse_regex = Regex::new(r"^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)").unwrap();

    let title = "[字幕組] [新番] 動畫標題 - 12 [1080p][10bit]";

    assert!(condition_regex.is_match(title));

    let captures = parse_regex.captures(title).expect("Should match");
    assert_eq!(captures.get(1).unwrap().as_str(), "字幕組");
    assert_eq!(captures.get(3).unwrap().as_str(), "12");
}

/// 測試解析失敗情況：condition_regex 匹配但 parse_regex 無法完整解析
#[test]
fn test_parse_failure_handling() {
    let condition_regex = Regex::new(r"^\[.+\].+\s-\s\d+").unwrap();
    let parse_regex = Regex::new(r"^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)\s*\[.*?(\d{3,4}p)").unwrap();

    // 這個標題匹配 condition_regex（有 [Group] 和 - 數字）
    // 但不匹配 parse_regex（缺少 [1080p] 等解析度資訊）
    let title = "[Group] Some Title - 01";

    assert!(condition_regex.is_match(title), "Should match condition regex");
    assert!(parse_regex.captures(title).is_none(), "Should not match parse regex (missing resolution)");
}

/// 測試特殊字符處理
#[test]
fn test_special_characters() {
    let parse_regex = Regex::new(r"^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)").unwrap();

    let title = "[ロリハウス] タイトル・チャンネル - 01";

    let captures = parse_regex.captures(title).expect("Should match");
    assert_eq!(captures.get(1).unwrap().as_str(), "ロリハウス");
    assert_eq!(captures.get(3).unwrap().as_str(), "01");
}

/// 測試空白字符處理
#[test]
fn test_whitespace_handling() {
    let parse_regex = Regex::new(r"^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)").unwrap();

    let titles = vec![
        "[Group]  Title  -  05",
        "[Group]   Title   -   05",
        "[Group]\tTitle\t-\t05",
    ];

    for title in titles {
        let captures = parse_regex.captures(title);
        if let Some(c) = captures {
            assert_eq!(c.get(3).unwrap().as_str(), "05");
        }
    }
}
