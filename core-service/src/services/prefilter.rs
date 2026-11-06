//! 解析前過濾：全域與訂閱層級的過濾規則在解析標題之前就能套用。
//!
//! 過濾規則是 AND（每條都要符合），被這兩層排除的標題，解析後套上全部規則也一定被排除，
//! 所以提早排除不會擋掉任何本來會通過的標題。被排除的原始項目標成 `filtered`：不解析、
//! 不算未匹配，也就不會為了它們請 AI 產生解析器（例如只要合集時，單集版本不必能解析）。
//! 作品 / 季 / 字幕組層級的規則要解析後才知道套用對象，維持在解析後套用。

use diesel::prelude::*;

use crate::models::{FilterRule, FilterTargetType};
use crate::schema::{anime_links, filter_rules, raw_anime_items};
use crate::services::filter::FilterEngine;
use crate::services::title_parser::ParseStatus;

/// 訂閱在解析前就能套用的規則：全域 + 訂閱（含舊的 fetcher 層級）
pub fn rules_before_parsing(
    conn: &mut PgConnection,
    subscription_id: i32,
) -> QueryResult<Vec<FilterRule>> {
    filter_rules::table
        .filter(
            filter_rules::target_type
                .eq(FilterTargetType::Global)
                .and(filter_rules::target_id.is_null())
                .or(filter_rules::target_type
                    .eq_any([FilterTargetType::Subscription, FilterTargetType::Fetcher])
                    .and(filter_rules::target_id.eq(subscription_id))),
        )
        .load(conn)
}

/// 這個標題在解析前就會被排除
pub fn excluded(rules: &[FilterRule], title: &str) -> bool {
    !FilterEngine::with_priority_sorted(rules.to_vec()).should_include(title)
}

/// 載入訂閱的解析前規則並判斷；查詢失敗時當作不排除（照舊解析）
pub fn excluded_for(conn: &mut PgConnection, subscription_id: i32, title: &str) -> bool {
    match rules_before_parsing(conn, subscription_id) {
        Ok(rules) => excluded(&rules, title),
        Err(e) => {
            tracing::warn!("無法載入訂閱 {} 的過濾規則: {}", subscription_id, e);
            false
        }
    }
}

/// 把原始項目標成解析前已過濾
pub fn mark_filtered(conn: &mut PgConnection, item_id: i32) -> QueryResult<usize> {
    diesel::update(raw_anime_items::table.find(item_id))
        .set((
            raw_anime_items::status.eq(ParseStatus::Filtered.as_str()),
            raw_anime_items::parser_id.eq(None::<i32>),
            raw_anime_items::error_message.eq(None::<String>),
            raw_anime_items::parsed_at.eq(Some(chrono::Utc::now().naive_utc())),
        ))
        .execute(conn)
}

/// 原始項目是否已經有連結（解析過）
pub fn has_links(conn: &mut PgConnection, item_id: i32) -> QueryResult<bool> {
    diesel::select(diesel::dsl::exists(
        anime_links::table.filter(anime_links::raw_item_id.eq(item_id)),
    ))
    .get_result(conn)
}

#[derive(Debug, Default, PartialEq)]
pub struct Refresh {
    /// 原本未匹配、現在被排除 → filtered
    pub filtered: usize,
    /// 原本 filtered、現在通過 → no_match（呼叫端要重跑解析）
    pub reopened: usize,
}

/// 規則變更後重新評估未匹配 / 已過濾的原始項目。`subscription_id` 為 None 時評估全部訂閱（全域規則變更）。
pub fn refresh(conn: &mut PgConnection, subscription_id: Option<i32>) -> QueryResult<Refresh> {
    let mut query = raw_anime_items::table
        .filter(raw_anime_items::status.eq_any([
            ParseStatus::NoMatch.as_str(),
            ParseStatus::Filtered.as_str(),
        ]))
        .select((
            raw_anime_items::item_id,
            raw_anime_items::subscription_id,
            raw_anime_items::title,
            raw_anime_items::status,
        ))
        .into_boxed();
    if let Some(id) = subscription_id {
        query = query.filter(raw_anime_items::subscription_id.eq(id));
    }
    let items: Vec<(i32, i32, String, String)> = query.load(conn)?;

    let mut rules_by_subscription = std::collections::HashMap::new();
    let mut result = Refresh::default();
    for (item_id, sub, title, status) in items {
        let rules = match rules_by_subscription.entry(sub) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(rules_before_parsing(conn, sub)?)
            }
        };
        let now_excluded = excluded(rules, &title);
        let was_filtered = status == ParseStatus::Filtered.as_str();
        if now_excluded && !was_filtered {
            mark_filtered(conn, item_id)?;
            result.filtered += 1;
        } else if !now_excluded && was_filtered {
            diesel::update(raw_anime_items::table.find(item_id))
                .set(raw_anime_items::status.eq(ParseStatus::NoMatch.as_str()))
                .execute(conn)?;
            result.reopened += 1;
        }
    }
    Ok(result)
}

/// 規則變更後的解析前重新評估（只有全域與訂閱層級會影響解析前過濾）；
/// 有項目重新開放時在背景重跑解析（可能因此請 AI 產生解析器）。
pub fn refresh_after_rule_change(
    pool: std::sync::Arc<crate::db::DbPool>,
    target_type: FilterTargetType,
    target_id: Option<i32>,
) {
    let scope = match target_type {
        FilterTargetType::Global => None,
        FilterTargetType::Subscription | FilterTargetType::Fetcher => match target_id {
            Some(id) => Some(id),
            None => return,
        },
        _ => return,
    };
    let outcome = pool
        .get()
        .map_err(|e| e.to_string())
        .and_then(|mut conn| refresh(&mut conn, scope).map_err(|e| e.to_string()));
    match outcome {
        Ok(r) => {
            if r.filtered > 0 || r.reopened > 0 {
                tracing::info!(
                    "解析前過濾重新評估（訂閱 {:?}）：{} 筆改為已過濾、{} 筆重新開放",
                    scope,
                    r.filtered,
                    r.reopened
                );
            }
            if r.reopened > 0 {
                tokio::spawn(async move {
                    if let Err(e) =
                        crate::handlers::pending_ai_results::rerun_unmatched_raw_items(pool).await
                    {
                        tracing::warn!("重新開放的項目重跑解析失敗: {}", e);
                    }
                });
            }
        }
        Err(e) => tracing::warn!("解析前過濾重新評估失敗: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn rule(target_type: FilterTargetType, positive: bool, pattern: &str) -> FilterRule {
        let now = Utc::now().naive_utc();
        FilterRule {
            rule_id: 1,
            target_type,
            target_id: Some(33),
            rule_order: 1,
            is_positive: positive,
            regex_pattern: pattern.to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn only_batches_pass_a_batch_only_rule() {
        let rules = [rule(
            FilterTargetType::Subscription,
            true,
            "合集.*1080p|1080p.*合集",
        )];
        let g = "[澄空学园&华盟字幕社] 城下町的蒲公英";
        assert!(!excluded(
            &rules,
            &format!("{g} 第01-12话 MKV 1080p HEVC 简体外挂 合集")
        ));
        assert!(excluded(&rules, &format!("{g} 第01-12话 MP4 720p 合集")));
        assert!(excluded(
            &rules,
            &format!("{g} 第04话 MKV 1080p HEVC 简体外挂")
        ));
        assert!(excluded(
            &rules,
            &format!("{g} 第10-11话 MKV 1080p HEVC 简体外挂")
        ));
    }

    #[test]
    fn no_rules_exclude_nothing() {
        assert!(!excluded(&[], "anything"));
    }

    mod with_db {
        use super::super::*;
        use crate::services::identity::test_db;

        #[derive(QueryableByName)]
        struct Id {
            #[diesel(sql_type = diesel::sql_types::Integer)]
            id: i32,
        }

        fn insert(conn: &mut PgConnection, sql: &str) -> i32 {
            diesel::sql_query(sql).get_result::<Id>(conn).unwrap().id
        }

        fn status(conn: &mut PgConnection, item: i32) -> String {
            raw_anime_items::table
                .find(item)
                .select(raw_anime_items::status)
                .first(conn)
                .unwrap()
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn a_batch_only_rule_sets_singles_aside_and_removing_it_reopens_them() {
            let mut conn = test_db::conn();
            let sub = insert(&mut conn, "INSERT INTO subscriptions (fetcher_id, source_url, name, created_at, updated_at) SELECT module_id, 'https://prefilter-test', 's', now(), now() FROM service_modules LIMIT 1 RETURNING subscription_id AS id");
            let item = |conn: &mut PgConnection, title: &str| {
                insert(conn, &format!("INSERT INTO raw_anime_items (title, download_url, subscription_id, status, created_at) VALUES ('{title}', 'u-{title}', {sub}, 'no_match', now()) RETURNING item_id AS id"))
            };
            let single = item(&mut conn, "[G] 城下町的蒲公英 第04话 MKV 1080p");
            let batch = item(&mut conn, "[G] 城下町的蒲公英 第01-12话 MKV 1080p 合集");
            let rule = insert(&mut conn, &format!("INSERT INTO filter_rules (target_type, target_id, rule_order, is_positive, regex_pattern, created_at, updated_at) VALUES ('subscription', {sub}, 1, true, '合集', now(), now()) RETURNING rule_id AS id"));

            assert_eq!(
                refresh(&mut conn, Some(sub)).unwrap(),
                Refresh {
                    filtered: 1,
                    reopened: 0
                }
            );
            assert_eq!(status(&mut conn, single), "filtered");
            assert_eq!(status(&mut conn, batch), "no_match", "still needs a parser");
            assert_eq!(
                refresh(&mut conn, Some(sub)).unwrap(),
                Refresh::default(),
                "nothing changes twice"
            );

            diesel::delete(filter_rules::table.find(rule))
                .execute(&mut conn)
                .unwrap();
            assert_eq!(
                refresh(&mut conn, Some(sub)).unwrap(),
                Refresh {
                    filtered: 0,
                    reopened: 1
                }
            );
            assert_eq!(status(&mut conn, single), "no_match");
        }
    }
}
