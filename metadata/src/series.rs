//! 系列的判定與命名：沿 bgm 前傳鏈找到第一季，以第一季的中文名作為各季共用的系列名稱。

use regex::Regex;
use std::sync::OnceLock;

pub use crate::bangumi_client::Subject;

/// 只有 TV / WEB 動畫沿前傳鏈歸入同一系列；劇場版、OVA 自成一部。
pub fn walks_prequels(platform: &str) -> bool {
    matches!(platform, "TV" | "WEB")
}

/// 去掉名稱結尾的季別標記（`第三季`、`第2期`、`Season 2`、`3rd Season`、`第2部分`），可重複出現。
/// 去完變成空字串時保留原名。
pub fn strip_season_suffix(name: &str) -> String {
    static SUFFIX: OnceLock<Regex> = OnceLock::new();
    let suffix = SUFFIX.get_or_init(|| {
        Regex::new(
            r"(?i)\s*(第[一二三四五六七八九十百\d]+(季|期|部分|部)|season\s*\d+|\d+(st|nd|rd|th)\s+season|part\s*\d+)\s*$",
        )
        .unwrap()
    });
    let mut current = name.trim().to_string();
    loop {
        let stripped = suffix.replace(&current, "").trim().to_string();
        if stripped == current || stripped.is_empty() {
            return current;
        }
        current = stripped;
    }
}

/// 系列名稱（spec 第 3 節）：從第一季往後找第一個非空的中文名並去掉季別標記；
/// 都沒有中文名時用第一季的原名。`chain` 由第一季排到目前這一季。
pub fn series_title(chain: &[Subject]) -> String {
    chain
        .iter()
        .map(|s| strip_season_suffix(&s.name_cn))
        .find(|name| !name.is_empty())
        .or_else(|| chain.first().map(|root| strip_season_suffix(&root.name)))
        .unwrap_or_default()
}

/// bgm infobox 的中文日期（`2026年3月21日`）轉 `YYYY-MM-DD`。
pub fn parse_cn_date(value: &str) -> Option<String> {
    static DATE: OnceLock<Regex> = OnceLock::new();
    let date = DATE.get_or_init(|| Regex::new(r"(\d{4})年(\d{1,2})月(\d{1,2})日").unwrap());
    let caps = date.captures(value)?;
    let part = |i: usize| caps[i].parse::<u32>().ok();
    Some(format!("{}-{:02}-{:02}", &caps[1], part(2)?, part(3)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(id: &str, name: &str, name_cn: &str, platform: &str) -> Subject {
        Subject {
            id: id.to_string(),
            name: name.to_string(),
            name_cn: name_cn.to_string(),
            platform: platform.to_string(),
            ..Subject::default()
        }
    }

    #[test]
    fn strips_season_markers_from_the_end_of_a_name() {
        assert_eq!(
            strip_season_suffix("药屋少女的呢喃 第三季"),
            "药屋少女的呢喃"
        );
        assert_eq!(
            strip_season_suffix("药屋少女的呢喃 第三季 第2部分"),
            "药屋少女的呢喃"
        );
        assert_eq!(strip_season_suffix("葬送的芙莉蓮第一季"), "葬送的芙莉蓮");
        assert_eq!(strip_season_suffix("メダリスト 第2期"), "メダリスト");
        assert_eq!(strip_season_suffix("Medalist Season 2"), "Medalist");
        assert_eq!(
            strip_season_suffix("Kusuriya no Hitorigoto 3rd Season"),
            "Kusuriya no Hitorigoto"
        );
        assert_eq!(strip_season_suffix("金牌得主"), "金牌得主");
    }

    #[test]
    fn does_not_strip_a_name_down_to_nothing() {
        assert_eq!(strip_season_suffix("第二季"), "第二季");
    }

    #[test]
    fn series_title_is_the_first_season_chinese_name() {
        let root = subject("430699", "メダリスト", "金牌得主", "TV");
        let season = subject("548818", "メダリスト 第2期", "金牌得主 第二季", "TV");
        assert_eq!(series_title(&[root, season]), "金牌得主");
    }

    #[test]
    fn series_title_falls_back_to_a_later_season_chinese_name() {
        let root = subject("1", "薬屋のひとりごと", "", "TV");
        let season = subject("3", "薬屋のひとりごと 第3期", "药屋少女的呢喃 第三季", "TV");
        assert_eq!(series_title(&[root, season]), "药屋少女的呢喃");
    }

    #[test]
    fn series_title_falls_back_to_the_original_name() {
        let root = subject("1", "LIAR GAME", "", "TV");
        assert_eq!(series_title(&[root]), "LIAR GAME");
    }

    #[test]
    fn parses_the_broadcast_end_date() {
        assert_eq!(
            parse_cn_date("2026年3月21日"),
            Some("2026-03-21".to_string())
        );
        assert_eq!(parse_cn_date("2026年3月"), None);
    }

    #[test]
    fn only_tv_and_web_seasons_belong_to_a_series_chain() {
        assert!(walks_prequels("TV"));
        assert!(walks_prequels("WEB"));
        assert!(!walks_prequels("剧场版"));
        assert!(!walks_prequels("OVA"));
    }
}
