/// Parser 固定 Prompt 預設值（revert 時使用）
pub const DEFAULT_FIXED_PARSER_PROMPT: &str = r#"你是一個動畫資料解析專家。根據提供的動畫標題，生成一個正則表達式解析器設定。
返回 JSON 格式，包含以下欄位：
- name: 解析器名稱（字串）
- condition_regex: 標題匹配條件（正則表達式字串）
- parse_regex: 解析用正則表達式，使用命名群組（字串）
- anime_title_source: "regex" 或 "static"
- anime_title_value: 如果是 regex，填命名群組名稱；如果是 static，填固定值
- episode_no_source: "regex" 或 "static"
- episode_no_value: 集數來源
- subtitle_group_source: "regex" 或 "static" 或 null
- subtitle_group_value: 字幕組來源或 null
- resolution_source: "regex" 或 "static" 或 null
- resolution_value: 解析度來源或 null
確保 parse_regex 的命名群組與對應的 *_value 欄位匹配。"#;

/// Filter 固定 Prompt 預設值
pub const DEFAULT_FIXED_FILTER_PROMPT: &str = r#"你是一個動畫過濾規則專家。根據提供的衝突動畫標題列表，生成過濾規則。
返回 JSON 格式，包含 rules 陣列，每個規則包含：
- regex_pattern: 過濾用正則表達式（字串）
- is_positive: true 表示保留匹配項，false 表示排除匹配項（布林值）
- rule_order: 規則順序，從 1 開始（整數）
目標是讓每個訂閱只保留最符合的集數。"#;

/// 組裝最終的 system prompt
pub fn build_system_prompt(fixed: Option<&str>) -> String {
    fixed.unwrap_or("").to_string()
}

/// 組裝 parser 的 user prompt
pub fn build_parser_user_prompt(title: &str, custom: Option<&str>) -> String {
    let mut s = format!("動畫標題：{}", title);
    if let Some(c) = custom {
        if !c.is_empty() {
            s.push_str("\n\n");
            s.push_str(c);
        }
    }
    s
}

/// 組裝 filter 的 user prompt（多個衝突標題）
pub fn build_filter_user_prompt(titles: &[String], custom: Option<&str>) -> String {
    let titles_str = titles
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{}. {}", i + 1, t))
        .collect::<Vec<_>>()
        .join("\n");
    let mut s = format!("衝突的動畫標題列表：\n{}", titles_str);
    if let Some(c) = custom {
        if !c.is_empty() {
            s.push_str("\n\n");
            s.push_str(c);
        }
    }
    s
}
