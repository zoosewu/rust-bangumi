//! 標題解析服務
//!
//! 負責使用 title_parsers 表中的解析器解析原始標題

use chrono::Utc;
use diesel::prelude::*;
use regex::Regex;

use crate::models::{NewRawAnimeItem, ParserSourceType, RawAnimeItem, TitleParser};
use crate::schema::{raw_anime_items, title_parsers};

/// 解析結果
#[derive(Debug, Clone)]
pub struct ParsedResult {
    pub anime_title: String,
    pub episode_no: i32,
    pub episode_end: Option<i32>,  // None = single episode; Some(n) = batch end
    pub series_no: i32,
    pub subtitle_group: Option<String>,
    pub resolution: Option<String>,
    pub season: Option<String>,
    pub year: Option<String>,
    pub parser_id: i32,
}

/// 解析狀態
#[derive(Debug, Clone, PartialEq)]
pub enum ParseStatus {
    Pending,
    Parsed,
    Partial,
    Failed,
    NoMatch,
    Skipped,
}

impl ParseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseStatus::Pending => "pending",
            ParseStatus::Parsed => "parsed",
            ParseStatus::Partial => "partial",
            ParseStatus::Failed => "failed",
            ParseStatus::NoMatch => "no_match",
            ParseStatus::Skipped => "skipped",
        }
    }
}

/// 標題解析服務
pub struct TitleParserService;

impl TitleParserService {
    /// 取得所有已啟用且已確認（非 pending）的解析器（按 priority 降序）
    pub fn get_enabled_parsers(conn: &mut PgConnection) -> Result<Vec<TitleParser>, String> {
        title_parsers::table
            .filter(title_parsers::is_enabled.eq(true))
            .filter(title_parsers::pending_result_id.is_null())
            .order(title_parsers::priority.desc())
            .load::<TitleParser>(conn)
            .map_err(|e| format!("Failed to load title parsers: {}", e))
    }

    /// 嘗試使用所有解析器解析標題
    pub fn parse_title(
        conn: &mut PgConnection,
        title: &str,
    ) -> Result<Option<ParsedResult>, String> {
        let parsers = Self::get_enabled_parsers(conn)?;

        for parser in parsers {
            if let Some(result) = Self::try_parser(&parser, title)? {
                return Ok(Some(result));
            }
        }

        Ok(None)
    }

    /// 解析標題，失敗時背景觸發 AI 生成（非同步）
    pub fn parse_title_with_ai_fallback(
        conn: &mut PgConnection,
        pool: std::sync::Arc<crate::db::DbPool>,
        title: &str,
        raw_item_id: Option<i32>,
    ) -> Result<Option<ParsedResult>, String> {
        let result = Self::parse_title(conn, title)?;

        if result.is_none() {
            use crate::schema::pending_ai_results;
            let already_pending: bool = pending_ai_results::table
                .filter(pending_ai_results::result_type.eq("parser"))
                .filter(pending_ai_results::source_title.eq(title))
                .filter(
                    pending_ai_results::status.eq_any(vec!["generating", "pending"]),
                )
                .count()
                .get_result::<i64>(conn)
                .unwrap_or(0)
                > 0;

            if !already_pending {
                let pool_clone = pool.clone();
                let title_owned = title.to_string();
                tokio::spawn(async move {
                    if let Err(e) = crate::ai::parser_generator::generate_parser_for_title(
                        pool_clone,
                        title_owned,
                        raw_item_id,
                        None,
                        None,
                    )
                    .await
                    {
                        tracing::warn!("AI parser 觸發失敗: {}", e);
                    }
                });
            }
        }

        Ok(result)
    }

    /// 嘗試使用單一解析器解析標題
    pub fn try_parser(parser: &TitleParser, title: &str) -> Result<Option<ParsedResult>, String> {
        // 檢查 condition_regex 是否匹配
        let condition_regex = Regex::new(&parser.condition_regex).map_err(|e| {
            format!(
                "Invalid condition_regex for parser {}: {}",
                parser.parser_id, e
            )
        })?;

        if !condition_regex.is_match(title) {
            return Ok(None);
        }

        // 執行 parse_regex
        let parse_regex = Regex::new(&parser.parse_regex)
            .map_err(|e| format!("Invalid parse_regex for parser {}: {}", parser.parser_id, e))?;

        let captures = match parse_regex.captures(title) {
            Some(c) => c,
            None => return Ok(None),
        };

        // 提取必要欄位
        let anime_title = Self::extract_value(
            &parser.anime_title_source,
            &parser.anime_title_value,
            &captures,
        )?;
        let episode_str = Self::extract_value(
            &parser.episode_no_source,
            &parser.episode_no_value,
            &captures,
        )?;
        let episode_no: i32 = episode_str
            .parse()
            .map_err(|_| format!("Failed to parse episode_no '{}' as integer", episode_str))?;

        // 提取 series_no（預設為 1）
        let series_no = match (&parser.series_no_source, &parser.series_no_value) {
            (Some(source), Some(value)) => {
                let s = Self::extract_value(source, value, &captures)?;
                s.parse().unwrap_or(1)
            }
            _ => 1,
        };

        // 提取非必要欄位
        let subtitle_group = Self::extract_optional_value(
            &parser.subtitle_group_source,
            &parser.subtitle_group_value,
            &captures,
        );

        let resolution = Self::extract_optional_value(
            &parser.resolution_source,
            &parser.resolution_value,
            &captures,
        );

        let season =
            Self::extract_optional_value(&parser.season_source, &parser.season_value, &captures);

        let year = Self::extract_optional_value(&parser.year_source, &parser.year_value, &captures);

        // Extract episode_end (optional range end for batch torrents)
        let episode_end = match Self::extract_optional_value(
            &parser.episode_end_source,
            &parser.episode_end_value,
            &captures,
        ) {
            Some(v) => v.parse::<i32>().ok(),
            None => None,
        };

        Ok(Some(ParsedResult {
            anime_title,
            episode_no,
            episode_end,
            series_no,
            subtitle_group,
            resolution,
            season,
            year,
            parser_id: parser.parser_id,
        }))
    }

    /// 從捕獲組或靜態值提取欄位值
    fn extract_value(
        source: &ParserSourceType,
        value: &str,
        captures: &regex::Captures,
    ) -> Result<String, String> {
        match source {
            ParserSourceType::Regex => {
                // Support both "$1" and "1" formats
                let index_str = value.strip_prefix('$').unwrap_or(value);
                let index: usize = index_str
                    .parse()
                    .map_err(|_| format!("Invalid capture group index: {}", value))?;
                captures
                    .get(index)
                    .map(|m| m.as_str().trim().to_string())
                    .ok_or_else(|| format!("Capture group {} not found", index))
            }
            ParserSourceType::Static => Ok(value.to_string()),
        }
    }

    /// 提取非必要欄位（可能為 None）
    fn extract_optional_value(
        source: &Option<ParserSourceType>,
        value: &Option<String>,
        captures: &regex::Captures,
    ) -> Option<String> {
        match (source, value) {
            (Some(s), Some(v)) => Self::extract_value(s, v, captures).ok(),
            _ => None,
        }
    }

    /// 儲存原始項目到資料庫
    pub fn save_raw_item(
        conn: &mut PgConnection,
        title: &str,
        description: Option<&str>,
        download_url: &str,
        pub_date: Option<chrono::NaiveDateTime>,
        subscription_id: i32,
    ) -> Result<RawAnimeItem, String> {
        let now = Utc::now().naive_utc();

        let new_item = NewRawAnimeItem {
            title: title.to_string(),
            description: description.map(|s| s.to_string()),
            download_url: download_url.to_string(),
            pub_date,
            subscription_id,
            status: ParseStatus::Pending.as_str().to_string(),
            parser_id: None,
            error_message: None,
            parsed_at: None,
            created_at: now,
        };

        diesel::insert_into(raw_anime_items::table)
            .values(&new_item)
            .on_conflict(raw_anime_items::download_url)
            .do_nothing()
            .get_result::<RawAnimeItem>(conn)
            .map_err(|e| format!("Failed to save raw item: {}", e))
    }

    /// 更新原始項目的解析狀態
    pub fn update_raw_item_status(
        conn: &mut PgConnection,
        item_id: i32,
        status: ParseStatus,
        parser_id: Option<i32>,
        error_message: Option<&str>,
    ) -> Result<(), String> {
        let now = Utc::now().naive_utc();

        diesel::update(raw_anime_items::table.filter(raw_anime_items::item_id.eq(item_id)))
            .set((
                raw_anime_items::status.eq(status.as_str()),
                raw_anime_items::parser_id.eq(parser_id),
                raw_anime_items::error_message.eq(error_message),
                raw_anime_items::parsed_at.eq(Some(now)),
            ))
            .execute(conn)
            .map_err(|e| format!("Failed to update raw item status: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::db::{TitleParser, ParserSourceType};
    use chrono::Utc;

    #[test]
    fn test_parse_status_as_str() {
        assert_eq!(ParseStatus::Pending.as_str(), "pending");
        assert_eq!(ParseStatus::Parsed.as_str(), "parsed");
        assert_eq!(ParseStatus::Partial.as_str(), "partial");
        assert_eq!(ParseStatus::Failed.as_str(), "failed");
        assert_eq!(ParseStatus::NoMatch.as_str(), "no_match");
        assert_eq!(ParseStatus::Skipped.as_str(), "skipped");
    }

    fn make_parser(id: i32, condition: &str, parse: &str) -> TitleParser {
        let now = Utc::now().naive_utc();
        TitleParser {
            parser_id: id,
            name: format!("test_{}", id),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: condition.to_string(),
            parse_regex: parse.to_string(),
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "$1".to_string(),
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "$2".to_string(),
            episode_end_source: None,
            episode_end_value: None,
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
            pending_result_id: None,
        }
    }

    fn make_batch_parser() -> TitleParser {
        let now = Utc::now().naive_utc();
        TitleParser {
            parser_id: 1,
            name: "batch_test".to_string(),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: r"\d+-\d+".to_string(),
            parse_regex: r"^(.+?)\s+(\d+)-(\d+)".to_string(),
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "$1".to_string(),
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "$2".to_string(),
            episode_end_source: Some(ParserSourceType::Regex),
            episode_end_value: Some("$3".to_string()),
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
            pending_result_id: None,
        }
    }

    #[test]
    fn test_try_parser_extracts_episode_end() {
        let parser = make_batch_parser();
        let title = "動畫名 01-12 [1080p]";
        let result = TitleParserService::try_parser(&parser, title).unwrap().unwrap();

        assert_eq!(result.episode_no, 1);
        assert_eq!(result.episode_end, Some(12));
        assert_eq!(result.anime_title, "動畫名");
    }

    #[test]
    fn test_try_parser_episode_end_none_for_single_episode() {
        let now = Utc::now().naive_utc();
        let parser = TitleParser {
            parser_id: 2,
            name: "single_test".to_string(),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: ".*".to_string(),
            parse_regex: r"^(.+?)\s+(\d+)".to_string(),
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "$1".to_string(),
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "$2".to_string(),
            episode_end_source: None,
            episode_end_value: None,
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
            pending_result_id: None,
        };
        let title = "動畫名 05 [1080p]";
        let result = TitleParserService::try_parser(&parser, title).unwrap().unwrap();

        assert_eq!(result.episode_no, 5);
        assert_eq!(result.episode_end, None);
    }

    /// condition_regex 不匹配時回傳 None
    #[test]
    fn test_try_parser_condition_no_match_returns_none() {
        let parser = make_parser(3, r"^\[.+\]", r"^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)");
        let title = "Title without brackets - 01";
        let result = TitleParserService::try_parser(&parser, title).unwrap();
        assert!(result.is_none(), "Should return None when condition_regex does not match");
    }

    /// condition_regex 匹配但 parse_regex 不匹配時回傳 None
    #[test]
    fn test_try_parser_parse_regex_no_match_returns_none() {
        let parser = make_parser(4, r"^\[.+\]", r"^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)\s*\[.*?(\d{3,4}p)");
        // 有 [Group] 和 - 數字，但缺少解析度資訊
        let title = "[Group] Title - 01";
        let result = TitleParserService::try_parser(&parser, title).unwrap();
        assert!(result.is_none(), "Should return None when parse_regex does not capture");
    }

    /// 使用 Static source type 提取 anime_title
    #[test]
    fn test_try_parser_static_source_type() {
        let now = Utc::now().naive_utc();
        let parser = TitleParser {
            parser_id: 5,
            name: "static_test".to_string(),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: ".*".to_string(),
            parse_regex: r"^.*?\s+(\d+)".to_string(),
            anime_title_source: ParserSourceType::Static,
            anime_title_value: "固定動畫名".to_string(),
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "$1".to_string(),
            episode_end_source: None,
            episode_end_value: None,
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
            pending_result_id: None,
        };
        let title = "任意標題 07 [1080p]";
        let result = TitleParserService::try_parser(&parser, title).unwrap().unwrap();
        assert_eq!(result.anime_title, "固定動畫名");
        assert_eq!(result.episode_no, 7);
    }

    /// capture group index 使用無前綴數字（"1" 而非 "$1"）
    #[test]
    fn test_try_parser_numeric_capture_index_without_dollar() {
        let now = Utc::now().naive_utc();
        let parser = TitleParser {
            parser_id: 6,
            name: "numeric_index_test".to_string(),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: ".*".to_string(),
            parse_regex: r"^(.+?)\s+-\s*(\d+)".to_string(),
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "1".to_string(), // 不帶 $ 前綴
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "2".to_string(), // 不帶 $ 前綴
            episode_end_source: None,
            episode_end_value: None,
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
            pending_result_id: None,
        };
        let title = "進擊的巨人 - 25 [1080p]";
        let result = TitleParserService::try_parser(&parser, title).unwrap().unwrap();
        assert_eq!(result.anime_title, "進擊的巨人");
        assert_eq!(result.episode_no, 25);
    }

    /// 解析所有可選欄位：subtitle_group、resolution、season、year
    #[test]
    fn test_try_parser_all_optional_fields() {
        let now = Utc::now().naive_utc();
        let parser = TitleParser {
            parser_id: 7,
            name: "full_fields_test".to_string(),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: r"^\[.+\]".to_string(),
            // [字幕組] 標題 - 集數 [解析度]
            parse_regex: r"^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)\s*\[(\d{3,4}p)\]".to_string(),
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "$2".to_string(),
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "$3".to_string(),
            episode_end_source: None,
            episode_end_value: None,
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: Some(ParserSourceType::Regex),
            subtitle_group_value: Some("$1".to_string()),
            resolution_source: Some(ParserSourceType::Regex),
            resolution_value: Some("$4".to_string()),
            season_source: Some(ParserSourceType::Static),
            season_value: Some("春".to_string()),
            year_source: Some(ParserSourceType::Static),
            year_value: Some("2025".to_string()),
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
            pending_result_id: None,
        };
        let title = "[LoliHouse] 進擊的巨人 - 25 [1080p]";
        let result = TitleParserService::try_parser(&parser, title).unwrap().unwrap();

        assert_eq!(result.anime_title, "進擊的巨人");
        assert_eq!(result.episode_no, 25);
        assert_eq!(result.subtitle_group, Some("LoliHouse".to_string()));
        assert_eq!(result.resolution, Some("1080p".to_string()));
        assert_eq!(result.season, Some("春".to_string()));
        assert_eq!(result.year, Some("2025".to_string()));
        assert_eq!(result.parser_id, 7);
    }

    /// series_no 從 capture group 提取
    #[test]
    fn test_try_parser_series_no_from_regex() {
        let now = Utc::now().naive_utc();
        let parser = TitleParser {
            parser_id: 8,
            name: "series_no_test".to_string(),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: ".*".to_string(),
            parse_regex: r"^(.+?)\s+S(\d+)E(\d+)".to_string(),
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "$1".to_string(),
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "$3".to_string(),
            episode_end_source: None,
            episode_end_value: None,
            series_no_source: Some(ParserSourceType::Regex),
            series_no_value: Some("$2".to_string()),
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
            pending_result_id: None,
        };
        let title = "鬼滅之刃 S03E08";
        let result = TitleParserService::try_parser(&parser, title).unwrap().unwrap();
        assert_eq!(result.anime_title, "鬼滅之刃");
        assert_eq!(result.series_no, 3);
        assert_eq!(result.episode_no, 8);
    }

    /// series_no 未設定時預設為 1
    #[test]
    fn test_try_parser_series_no_defaults_to_one() {
        let parser = make_parser(9, ".*", r"^(.+?)\s+-\s*(\d+)");
        let title = "進擊的巨人 - 10";
        let result = TitleParserService::try_parser(&parser, title).unwrap().unwrap();
        assert_eq!(result.series_no, 1);
    }

    /// episode_no 無法轉換為 i32 時回傳 Err
    #[test]
    fn test_try_parser_episode_no_parse_error() {
        let now = Utc::now().naive_utc();
        let parser = TitleParser {
            parser_id: 10,
            name: "err_test".to_string(),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: ".*".to_string(),
            parse_regex: r"^(.+?)\s+-\s*([A-Za-z]+)".to_string(), // 集數是字母，不能轉 i32
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "$1".to_string(),
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "$2".to_string(),
            episode_end_source: None,
            episode_end_value: None,
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
            pending_result_id: None,
        };
        let title = "タイトル - SP";
        let result = TitleParserService::try_parser(&parser, title);
        assert!(result.is_err(), "Should return Err when episode_no is not numeric");
    }

    /// invalid condition_regex 回傳 Err
    #[test]
    fn test_try_parser_invalid_condition_regex_returns_err() {
        let now = Utc::now().naive_utc();
        let parser = TitleParser {
            parser_id: 11,
            name: "invalid_regex_test".to_string(),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: r"[invalid(regex".to_string(), // 無效 regex
            parse_regex: r"^(.+?)\s+-\s*(\d+)".to_string(),
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "$1".to_string(),
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "$2".to_string(),
            episode_end_source: None,
            episode_end_value: None,
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
            pending_result_id: None,
        };
        let result = TitleParserService::try_parser(&parser, "some title - 01");
        assert!(result.is_err(), "Should return Err for invalid condition_regex");
    }

    /// episode_end 無效時（end < start）視為單集
    #[test]
    fn test_batch_parser_episode_end_invalid_when_end_less_than_start() {
        let parser = make_batch_parser();
        // 05-01 不合理，end < start → 視為單集（解析成功但 episode_end 仍回傳 Some(1)）
        // try_parser 只做 regex 解析，不做業務驗證，所以 episode_end = Some(1) 是正確的
        let title = "動畫名 05-01 [1080p]";
        let result = TitleParserService::try_parser(&parser, title).unwrap().unwrap();
        assert_eq!(result.episode_no, 5);
        assert_eq!(result.episode_end, Some(1));
    }
}
