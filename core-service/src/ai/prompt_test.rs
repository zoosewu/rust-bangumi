//! 用設定好的真實 prompt 與某個訂閱的真實標題測試 provider：送出的內容與正式生成相同，
//! 但只送給指定的 provider、不建立待確認結果，並把 AI 產生的 regex 實際套到標題上。

use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::prompts::*;
use crate::schema::{anime_links, raw_anime_items};

pub use super::prompts::PromptKind;

/// 要送給 AI 的完整內容
#[derive(Debug, Clone, Serialize)]
pub struct PromptSample {
    pub system_prompt: String,
    pub user_prompt: String,
    #[serde(skip)]
    pub schema: Value,
    /// 送出的標題（評估結果依此順序）
    pub titles: Vec<String>,
}

/// AI 產生的規則套到某個標題上的結果
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TitleEvaluation {
    pub title: String,
    /// parser：`match` / `no_match`；filter：`keep` / `drop`
    pub outcome: String,
    /// parser：擷取到的內容（例如 `集數=05`）；filter：不使用
    pub detail: Option<String>,
}

/// 只用於這次測試的 prompt（不寫進設定）。沒給的部分沿用設定；`custom_prompt` 給空字串代表不加自訂 prompt。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromptOverrides {
    #[serde(default)]
    pub fixed_prompt: Option<String>,
    #[serde(default)]
    pub custom_prompt: Option<String>,
}

fn load_prompts(
    conn: &mut PgConnection,
    kind: PromptKind,
    overrides: &PromptOverrides,
) -> Result<(String, Option<String>), String> {
    let settings = crate::schema::ai_prompt_settings::table
        .first::<crate::models::AiPromptSettings>(conn)
        .optional()
        .map_err(|e| e.to_string())?;
    let non_empty = |s: Option<String>| s.filter(|v| !v.trim().is_empty());
    let (fixed, custom) = match kind {
        PromptKind::Parser => (
            non_empty(
                settings
                    .as_ref()
                    .and_then(|p| p.fixed_parser_prompt.clone()),
            )
            .unwrap_or_else(|| DEFAULT_FIXED_PARSER_PROMPT.to_string()),
            non_empty(settings.and_then(|p| p.custom_parser_prompt)),
        ),
        PromptKind::Filter => (
            non_empty(
                settings
                    .as_ref()
                    .and_then(|p| p.fixed_filter_prompt.clone()),
            )
            .unwrap_or_else(|| DEFAULT_FIXED_FILTER_PROMPT.to_string()),
            non_empty(settings.and_then(|p| p.custom_filter_prompt)),
        ),
    };
    Ok((
        non_empty(overrides.fixed_prompt.clone()).unwrap_or(fixed),
        match &overrides.custom_prompt {
            Some(c) => non_empty(Some(c.clone())),
            None => custom,
        },
    ))
}

/// 依正式流程組出 prompt：parser 送訂閱最早三集的所有標題；filter 送同一集有多個版本的前三集。
pub fn build_sample(
    conn: &mut PgConnection,
    kind: PromptKind,
    subscription_id: i32,
    overrides: &PromptOverrides,
) -> Result<PromptSample, String> {
    let (fixed, custom) = load_prompts(conn, kind, overrides)?;
    let system_prompt = build_system_prompt(Some(&fixed));
    match kind {
        PromptKind::Parser => {
            let all: Vec<String> = raw_anime_items::table
                .filter(raw_anime_items::subscription_id.eq(subscription_id))
                .order((
                    raw_anime_items::pub_date.asc(),
                    raw_anime_items::item_id.asc(),
                ))
                .select(raw_anime_items::title)
                .load(conn)
                .map_err(|e| e.to_string())?;
            let mut seen = std::collections::HashSet::new();
            let titles =
                titles_of_first_episodes(all.into_iter().filter(|t| seen.insert(t.clone())));
            if titles.is_empty() {
                return Err("這個訂閱還沒有任何標題可以測試".to_string());
            }
            Ok(PromptSample {
                user_prompt: build_parser_batch_user_prompt(&titles, custom.as_deref()),
                system_prompt,
                schema: parser_schema(),
                titles,
            })
        }
        PromptKind::Filter => {
            let links: Vec<(i32, i32, i32, Option<String>)> = anime_links::table
                .inner_join(raw_anime_items::table)
                .filter(raw_anime_items::subscription_id.eq(subscription_id))
                .order((
                    anime_links::anime_id,
                    anime_links::group_id,
                    anime_links::episode_no,
                    anime_links::link_id,
                ))
                .select((
                    anime_links::anime_id,
                    anime_links::group_id,
                    anime_links::episode_no,
                    anime_links::title,
                ))
                .load(conn)
                .map_err(|e| e.to_string())?;
            let mut groups: std::collections::BTreeMap<(i32, i32, i32), Vec<String>> =
                Default::default();
            for (anime, group, ep, title) in links {
                if let Some(title) = title {
                    let titles = groups.entry((anime, group, ep)).or_default();
                    if !titles.contains(&title) {
                        titles.push(title);
                    }
                }
            }
            let groups = first_episodes(groups.into_values().filter(|t| t.len() > 1));
            if groups.is_empty() {
                return Err("這個訂閱沒有同一集有多個版本的情況，無法測試過濾規則".to_string());
            }
            Ok(PromptSample {
                user_prompt: build_filter_batch_user_prompt(&groups, custom.as_deref()),
                system_prompt,
                schema: filter_schema(),
                titles: groups.into_iter().flatten().collect(),
            })
        }
    }
}

/// 把 AI 的回應整理成 JSON（與正式流程相同的清理與 regex 雙重轉義修正）
pub fn parse_response(kind: PromptKind, raw: &str) -> Result<Value, String> {
    let sanitized = super::sanitize_ai_json(super::extract_json(raw));
    let mut data: Value =
        serde_json::from_str(&sanitized).map_err(|e| format!("AI 回應不是有效的 JSON：{}", e))?;
    let fix = |v: &mut Value| {
        if let Some(fixed) = v.as_str().map(super::fix_regex_escaping) {
            *v = Value::String(fixed);
        }
    };
    match kind {
        PromptKind::Parser => {
            for field in ["condition_regex", "parse_regex"] {
                if let Some(v) = data.get_mut(field) {
                    fix(v);
                }
            }
        }
        PromptKind::Filter => {
            for rule in data
                .get_mut("rules")
                .and_then(|r| r.as_array_mut())
                .into_iter()
                .flatten()
            {
                if let Some(v) = rule.get_mut("regex_pattern") {
                    fix(v);
                }
            }
        }
    }
    Ok(data)
}

fn regex(pattern: &str, field: &str) -> Result<regex::Regex, String> {
    regex::Regex::new(pattern).map_err(|e| format!("{} 不是有效的 regex：{}", field, e))
}

/// 把 AI 產生的規則套到送出的標題上
pub fn evaluate(
    kind: PromptKind,
    data: &Value,
    titles: &[String],
) -> Result<Vec<TitleEvaluation>, String> {
    match kind {
        PromptKind::Parser => {
            let condition = regex(
                data["condition_regex"].as_str().unwrap_or(""),
                "condition_regex",
            )?;
            let parse = regex(data["parse_regex"].as_str().unwrap_or(""), "parse_regex")?;
            let field = |name: &str, caps: &regex::Captures| -> Option<String> {
                let source = data[format!("{name}_source")].as_str()?;
                let value = data[format!("{name}_value")].as_str()?;
                match source {
                    "static" => Some(value.to_string()),
                    _ => {
                        let mut out = String::new();
                        caps.expand(value, &mut out);
                        Some(out).filter(|s| !s.is_empty())
                    }
                }
            };
            Ok(titles
                .iter()
                .map(
                    |title| match parse.captures(title).filter(|_| condition.is_match(title)) {
                        Some(caps) => TitleEvaluation {
                            title: title.clone(),
                            outcome: "match".to_string(),
                            detail: Some(
                                [
                                    ("作品", "anime_title"),
                                    ("集數", "episode_no"),
                                    ("結束集數", "episode_end"),
                                    ("字幕組", "subtitle_group"),
                                    ("解析度", "resolution"),
                                ]
                                .iter()
                                .filter_map(|(label, name)| {
                                    field(name, &caps).map(|v| format!("{label}={v}"))
                                })
                                .collect::<Vec<_>>()
                                .join("、"),
                            ),
                        },
                        None => TitleEvaluation {
                            title: title.clone(),
                            outcome: "no_match".to_string(),
                            detail: None,
                        },
                    },
                )
                .collect())
        }
        PromptKind::Filter => {
            let rules: Vec<(regex::Regex, bool)> = data["rules"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| {
                    regex(r["regex_pattern"].as_str().unwrap_or(""), "regex_pattern")
                        .map(|re| (re, r["is_positive"].as_bool().unwrap_or(true)))
                })
                .collect::<Result<_, _>>()?;
            // 與過濾引擎相同：所有正向規則都要符合，且不符合任何反向規則
            Ok(titles
                .iter()
                .map(|title| {
                    let keep = rules
                        .iter()
                        .all(|(re, positive)| re.is_match(title) == *positive);
                    TitleEvaluation {
                        title: title.clone(),
                        outcome: if keep { "keep" } else { "drop" }.to_string(),
                        detail: None,
                    }
                })
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn titles(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn filter_rules_keep_traditional_japanese_and_drop_the_rest() {
        let data = json!({"rules": [
            {"regex_pattern": "繁日", "is_positive": true, "rule_order": 1},
            {"regex_pattern": "简", "is_positive": false, "rule_order": 2}
        ]});
        let t = titles(&[
            "[G] T [01][繁日内嵌]",
            "[G] T [01][简日内嵌]",
            "[G] T [01][简繁日内封]",
        ]);
        let outcomes: Vec<String> = evaluate(PromptKind::Filter, &data, &t)
            .unwrap()
            .into_iter()
            .map(|e| e.outcome)
            .collect();
        assert_eq!(outcomes, vec!["keep", "drop", "drop"]);
    }

    #[test]
    fn a_parser_reports_what_it_extracts() {
        let data = json!({
            "condition_regex": "^\\[LoliHouse\\]",
            "parse_regex": "^\\[LoliHouse\\]\\s*(.+?)\\s*-\\s*(\\d+)",
            "anime_title_source": "regex", "anime_title_value": "$1",
            "episode_no_source": "regex", "episode_no_value": "$2",
            "subtitle_group_source": "static", "subtitle_group_value": "LoliHouse"
        });
        let t = titles(&["[LoliHouse] Super no Ura - 05 [1080p]", "[Other] X - 01"]);
        let result = evaluate(PromptKind::Parser, &data, &t).unwrap();
        assert_eq!(result[0].outcome, "match");
        assert_eq!(
            result[0].detail.as_deref(),
            Some("作品=Super no Ura、集數=05、字幕組=LoliHouse")
        );
        assert_eq!(result[1].outcome, "no_match");
    }

    #[test]
    fn an_invalid_regex_is_reported_instead_of_evaluated() {
        let data =
            json!({"rules": [{"regex_pattern": "([", "is_positive": true, "rule_order": 1}]});
        let err = evaluate(PromptKind::Filter, &data, &titles(&["t"])).unwrap_err();
        assert!(err.contains("regex_pattern"), "{err}");
    }

    #[test]
    fn responses_are_cleaned_like_real_generation() {
        let raw = "```json\n{\"rules\":[{\"regex_pattern\":\"\\\\\\\\[G\\\\\\\\]\",\"is_positive\":true,\"rule_order\":1}],\"resolved_groups\":[1],\"unresolved_groups\":[]}\n```";
        let data = parse_response(PromptKind::Filter, raw).unwrap();
        assert_eq!(data["rules"][0]["regex_pattern"], "\\[G\\]");
        assert!(parse_response(PromptKind::Filter, "not json").is_err());
    }

    mod db {
        use super::super::*;
        use crate::services::identity::test_db;
        use diesel::sql_types::{Integer, Text};

        #[derive(QueryableByName)]
        struct Id {
            #[diesel(sql_type = Integer)]
            id: i32,
        }

        fn insert(conn: &mut PgConnection, sql: &str) -> i32 {
            diesel::sql_query(sql).get_result::<Id>(conn).unwrap().id
        }

        /// 一個訂閱、一季、一個字幕組；回傳 (訂閱, 季, 字幕組)
        fn subscription(conn: &mut PgConnection) -> (i32, i32, i32) {
            let fetcher = insert(conn, "INSERT INTO service_modules (module_type, name, version, is_enabled, priority, base_url, created_at, updated_at) VALUES ('fetcher', 'prompt-test-fetcher', '1', true, 1, 'http://f', now(), now()) RETURNING module_id AS id");
            let sub = insert(conn, &format!("INSERT INTO subscriptions (fetcher_id, source_url, name, created_at, updated_at) VALUES ({fetcher}, 'https://prompt-test', 's', now(), now()) RETURNING subscription_id AS id"));
            let work = insert(conn, "INSERT INTO anime_works (title, created_at, updated_at) VALUES ('prompt 測試', now(), now()) RETURNING work_id AS id");
            let season = insert(conn, "INSERT INTO seasons (year, season, created_at) VALUES (2097, 'winter', now()) RETURNING season_id AS id");
            let anime = insert(conn, &format!("INSERT INTO animes (work_id, series_no, season_id, created_at, updated_at) VALUES ({work}, 1, {season}, now(), now()) RETURNING anime_id AS id"));
            let group = insert(conn, "INSERT INTO subtitle_groups (group_name) VALUES ('prompt 測試組') RETURNING group_id AS id");
            (sub, anime, group)
        }

        fn item(conn: &mut PgConnection, sub: i32, title: &str, hour: i32) -> i32 {
            diesel::sql_query("INSERT INTO raw_anime_items (title, download_url, subscription_id, status, pub_date, created_at) VALUES ($1, $1, $2, 'no_match', '2026-01-01'::timestamp + make_interval(hours => $3), now()) RETURNING item_id AS id")
                .bind::<Text, _>(title)
                .bind::<Integer, _>(sub)
                .bind::<Integer, _>(hour)
                .get_result::<Id>(conn)
                .unwrap()
                .id
        }

        fn link(conn: &mut PgConnection, anime: i32, group: i32, ep: i32, item: i32, title: &str) {
            diesel::sql_query("INSERT INTO anime_links (anime_id, group_id, episode_no, url, source_hash, raw_item_id, title) VALUES ($1, $2, $3, $4, $4, $5, $4)")
                .bind::<Integer, _>(anime)
                .bind::<Integer, _>(group)
                .bind::<Integer, _>(ep)
                .bind::<Text, _>(title)
                .bind::<Integer, _>(item)
                .execute(conn)
                .unwrap();
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn the_parser_sample_is_every_version_of_the_first_three_episodes() {
            let mut conn = test_db::conn();
            let (sub, _, _) = subscription(&mut conn);
            for (i, title) in [
                "[G] T [01][繁日]",
                "[G] T [01][简日]",
                "[G] T [02][繁日]",
                "[G] T [03][繁日]",
                "[G] T [04][繁日]",
                "[G] T [02][简日]",
            ]
            .iter()
            .enumerate()
            {
                item(&mut conn, sub, title, i as i32);
            }
            let sample = build_sample(
                &mut conn,
                PromptKind::Parser,
                sub,
                &PromptOverrides::default(),
            )
            .unwrap();
            assert_eq!(sample.titles.len(), 5);
            assert!(!sample.titles.iter().any(|t| t.contains("[04]")));
            assert!(sample.user_prompt.contains("[G] T [02][简日]"));
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn a_test_can_try_other_prompts_without_saving_them() {
            let mut conn = test_db::conn();
            let (sub, _, _) = subscription(&mut conn);
            item(&mut conn, sub, "[G] T [01][繁日]", 0);
            diesel::sql_query(
                "UPDATE ai_prompt_settings SET custom_parser_prompt = 'saved custom'",
            )
            .execute(&mut conn)
            .unwrap();
            let sample = |o: PromptOverrides, conn: &mut PgConnection| {
                build_sample(conn, PromptKind::Parser, sub, &o).unwrap()
            };

            let saved = sample(PromptOverrides::default(), &mut conn);
            assert_eq!(saved.system_prompt, DEFAULT_FIXED_PARSER_PROMPT);
            assert!(saved.user_prompt.ends_with("saved custom"));

            let tried = sample(
                PromptOverrides {
                    fixed_prompt: Some("shorter prompt".to_string()),
                    custom_prompt: Some(String::new()),
                },
                &mut conn,
            );
            assert_eq!(tried.system_prompt, "shorter prompt");
            assert!(
                !tried.user_prompt.contains("saved custom"),
                "empty custom means none"
            );

            let only_fixed = sample(
                PromptOverrides {
                    fixed_prompt: Some("shorter prompt".to_string()),
                    custom_prompt: None,
                },
                &mut conn,
            );
            assert!(
                only_fixed.user_prompt.ends_with("saved custom"),
                "custom falls back to settings"
            );
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn the_filter_sample_is_the_first_three_episodes_with_several_versions() {
            let mut conn = test_db::conn();
            let (sub, anime, group) = subscription(&mut conn);
            for ep in 1..=4 {
                for lang in ["繁日", "简日"] {
                    let title = format!("[G] T [0{ep}][{lang}]");
                    let id = item(&mut conn, sub, &title, ep);
                    link(&mut conn, anime, group, ep, id, &title);
                }
            }
            // 只有一個版本的集數不算
            let single = item(&mut conn, sub, "[G] T [00][繁日]", 0);
            link(&mut conn, anime, group, 0, single, "[G] T [00][繁日]");

            let sample = build_sample(
                &mut conn,
                PromptKind::Filter,
                sub,
                &PromptOverrides::default(),
            )
            .unwrap();
            assert_eq!(sample.titles.len(), 6);
            assert!(sample.user_prompt.contains("--- Group 3 ---"));
            assert!(!sample.user_prompt.contains("[04]"));
            assert!(!sample.user_prompt.contains("[00]"));
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn a_subscription_without_versions_cannot_test_filters() {
            let mut conn = test_db::conn();
            let (sub, _, _) = subscription(&mut conn);
            assert!(build_sample(
                &mut conn,
                PromptKind::Filter,
                sub,
                &PromptOverrides::default()
            )
            .is_err());
            assert!(build_sample(
                &mut conn,
                PromptKind::Parser,
                sub,
                &PromptOverrides::default()
            )
            .is_err());
        }
    }
}
