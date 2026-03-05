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
    /// 取得所有啟用的解析器（按 priority 降序）
    pub fn get_enabled_parsers(conn: &mut PgConnection) -> Result<Vec<TitleParser>, String> {
        title_parsers::table
            .filter(title_parsers::is_enabled.eq(true))
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
        assert_eq!(ParseStatus::NoMatch.as_str(), "no_match");
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
}
