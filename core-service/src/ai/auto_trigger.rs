//! 何時自動請 AI 產生解析器 / 過濾規則。
//!
//! 同一個訂閱的同一種結果一次只有一組等待確認；失敗後隔一段時間才重試（避免每次抓取都撞上限流）；
//! 使用者看過（確認或拒絕）之後，只有出現新的輸入（未匹配標題 / 衝突）才再產生。

use std::sync::Arc;

use chrono::{Duration, NaiveDateTime};
use diesel::prelude::*;

use crate::db::DbPool;
use crate::schema::{anime_link_conflicts, anime_links, pending_ai_results, raw_anime_items};
use crate::services::title_parser::ParseStatus;

/// 失敗後多久才自動重試
pub const RETRY_AFTER_FAILURE: Duration = Duration::hours(6);
/// 停在 generating 超過這段時間視為中斷（例如服務重啟），可以重新產生
pub const STALE_GENERATING: Duration = Duration::hours(1);

/// 依同訂閱最新一筆結果（狀態、建立時間）與最新的輸入時間，決定要不要再產生
pub fn should_generate(
    latest: Option<(&str, NaiveDateTime)>,
    newest_input_at: Option<NaiveDateTime>,
    now: NaiveDateTime,
) -> bool {
    let Some(newest_input_at) = newest_input_at else {
        return false;
    };
    match latest {
        None => true,
        Some(("pending", _)) => false,
        Some(("generating", at)) => now - at >= STALE_GENERATING,
        Some(("failed", at)) => now - at >= RETRY_AFTER_FAILURE,
        // confirmed / rejected：使用者看過了，只有之後的新輸入才再產生
        Some((_, at)) => newest_input_at > at,
    }
}

fn latest_result(
    conn: &mut PgConnection,
    result_type: &str,
    subscription_id: i32,
) -> QueryResult<Option<(String, NaiveDateTime)>> {
    pending_ai_results::table
        .filter(pending_ai_results::result_type.eq(result_type))
        .filter(pending_ai_results::subscription_id.eq(subscription_id))
        .order(pending_ai_results::created_at.desc())
        .select((pending_ai_results::status, pending_ai_results::created_at))
        .first(conn)
        .optional()
}

/// 訂閱有未匹配標題、且沒有等待中的解析器時才產生
pub fn parser_due(conn: &mut PgConnection, subscription_id: i32) -> QueryResult<bool> {
    let newest_unmatched: Option<NaiveDateTime> = raw_anime_items::table
        .filter(raw_anime_items::subscription_id.eq(subscription_id))
        .filter(raw_anime_items::status.eq(ParseStatus::NoMatch.as_str()))
        .select(diesel::dsl::max(raw_anime_items::created_at))
        .first(conn)?;
    let latest = latest_result(conn, "parser", subscription_id)?;
    Ok(should_generate(
        latest.as_ref().map(|(s, at)| (s.as_str(), *at)),
        newest_unmatched,
        chrono::Utc::now().naive_utc(),
    ))
}

/// 訂閱的衝突群組：每個未解決衝突的集數一組（該集所有衝突版本的標題，第一個標題當來源），
/// 依（季, 字幕組, 集數）排序，送給 AI 時才能取「前幾集」
pub fn conflict_groups_for_subscription(
    conn: &mut PgConnection,
    subscription_id: i32,
) -> QueryResult<Vec<(Vec<String>, String)>> {
    let episodes: Vec<(i32, i32, i32)> = anime_links::table
        .inner_join(
            raw_anime_items::table
                .on(anime_links::raw_item_id.eq(raw_anime_items::item_id.nullable())),
        )
        .filter(raw_anime_items::subscription_id.eq(subscription_id))
        .filter(anime_links::conflict_flag.eq(true))
        .select((
            anime_links::anime_id,
            anime_links::group_id,
            anime_links::episode_no,
        ))
        .distinct()
        .load(conn)?;

    let mut groups = std::collections::BTreeMap::new();
    for (anime_id, group_id, episode_no) in episodes {
        // 該集所有衝突版本（不限本訂閱）
        let titles: Vec<String> = anime_links::table
            .filter(anime_links::anime_id.eq(anime_id))
            .filter(anime_links::group_id.eq(group_id))
            .filter(anime_links::episode_no.eq(episode_no))
            .filter(anime_links::conflict_flag.eq(true))
            .select(anime_links::title)
            .load::<Option<String>>(conn)?
            .into_iter()
            .flatten()
            .collect();
        if titles.len() > 1 {
            groups.insert((anime_id, group_id, episode_no), titles);
        }
    }
    Ok(groups
        .into_values()
        .map(|titles| {
            let source = titles[0].clone();
            (titles, source)
        })
        .collect())
}

/// 訂閱的未解決衝突最近一次出現的時間
fn newest_conflict(
    conn: &mut PgConnection,
    subscription_id: i32,
) -> QueryResult<Option<NaiveDateTime>> {
    anime_link_conflicts::table
        .filter(anime_link_conflicts::resolution_status.eq("unresolved"))
        .filter(diesel::dsl::exists(
            anime_links::table
                .inner_join(
                    raw_anime_items::table
                        .on(anime_links::raw_item_id.eq(raw_anime_items::item_id.nullable())),
                )
                .filter(raw_anime_items::subscription_id.eq(subscription_id))
                .filter(anime_links::anime_id.eq(anime_link_conflicts::anime_id))
                .filter(anime_links::group_id.eq(anime_link_conflicts::group_id))
                .filter(anime_links::episode_no.eq(anime_link_conflicts::episode_no))
                .filter(anime_links::conflict_flag.eq(true)),
        ))
        .select(diesel::dsl::max(anime_link_conflicts::created_at))
        .first(conn)
}

/// 訂閱有未解決的衝突、且沒有等待中的過濾規則時，背景請 AI 產生（結果進待確認清單）。
/// 回傳是否觸發。
pub fn spawn_filter_generation_if_due(pool: Arc<DbPool>, subscription_id: i32) -> bool {
    let groups = pool.get().map_err(|e| e.to_string()).and_then(|mut conn| {
        let latest =
            latest_result(&mut conn, "filter", subscription_id).map_err(|e| e.to_string())?;
        let newest = newest_conflict(&mut conn, subscription_id).map_err(|e| e.to_string())?;
        if !should_generate(
            latest.as_ref().map(|(s, at)| (s.as_str(), *at)),
            newest,
            chrono::Utc::now().naive_utc(),
        ) {
            return Ok(vec![]);
        }
        conflict_groups_for_subscription(&mut conn, subscription_id).map_err(|e| e.to_string())
    });

    match groups {
        Ok(groups) if !groups.is_empty() => {
            tracing::info!(
                "subscription={} 有 {} 集衝突，自動請 AI 產生過濾規則",
                subscription_id,
                groups.len()
            );
            tokio::spawn(async move {
                if let Err(e) =
                    crate::ai::filter_generator::generate_filters_for_subscription_batch(
                        pool,
                        Some(subscription_id),
                        groups,
                    )
                    .await
                {
                    tracing::warn!(
                        "自動產生過濾規則失敗 subscription={}: {}",
                        subscription_id,
                        e
                    );
                }
            });
            true
        }
        Ok(_) => false,
        Err(e) => {
            tracing::warn!(
                "無法判斷是否產生過濾規則 subscription={}: {}",
                subscription_id,
                e
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(h: i64) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 22)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            + Duration::hours(h)
    }

    #[test]
    fn nothing_to_send_means_nothing_to_generate() {
        assert!(!should_generate(None, None, at(10)));
    }

    #[test]
    fn the_first_input_is_sent() {
        assert!(should_generate(None, Some(at(1)), at(1)));
    }

    #[test]
    fn one_result_at_a_time_waits_for_the_user() {
        assert!(!should_generate(
            Some(("pending", at(0))),
            Some(at(5)),
            at(5)
        ));
        assert!(!should_generate(
            Some(("generating", at(0))),
            Some(at(0)),
            at(0)
        ));
        // 卡在 generating（服務中途重啟）超過一小時就重來
        assert!(should_generate(
            Some(("generating", at(0))),
            Some(at(0)),
            at(2)
        ));
    }

    #[test]
    fn a_failure_waits_six_hours_before_retrying() {
        assert!(!should_generate(
            Some(("failed", at(0))),
            Some(at(3)),
            at(5)
        ));
        assert!(should_generate(Some(("failed", at(0))), Some(at(0)), at(6)));
    }

    #[test]
    fn after_the_user_decides_only_new_input_counts() {
        for status in ["confirmed", "rejected"] {
            assert!(!should_generate(Some((status, at(2))), Some(at(1)), at(9)));
            assert!(should_generate(Some((status, at(2))), Some(at(3)), at(3)));
        }
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

        fn subscription(conn: &mut PgConnection, n: &str) -> i32 {
            insert(conn, &format!("INSERT INTO subscriptions (fetcher_id, source_url, name, created_at, updated_at) SELECT module_id, 'https://auto-trigger-test/{n}', 's', now(), now() FROM service_modules LIMIT 1 RETURNING subscription_id AS id"))
        }

        fn item(conn: &mut PgConnection, sub: i32, title: &str, status: &str) -> i32 {
            insert(conn, &format!("INSERT INTO raw_anime_items (title, download_url, subscription_id, status, created_at) VALUES ('{title}', 'auto-trigger-{sub}-{title}', {sub}, '{status}', now()) RETURNING item_id AS id"))
        }

        fn result(conn: &mut PgConnection, kind: &str, sub: i32, status: &str) {
            insert(conn, &format!("INSERT INTO pending_ai_results (result_type, source_title, status, subscription_id) VALUES ('{kind}', 'batch', '{status}', {sub}) RETURNING id"));
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn a_waiting_parser_blocks_another_one() {
            let mut conn = test_db::conn();
            let sub = subscription(&mut conn, "parser");
            assert!(
                !parser_due(&mut conn, sub).unwrap(),
                "nothing unmatched yet"
            );
            item(&mut conn, sub, "[X] new - 01", "no_match");
            assert!(parser_due(&mut conn, sub).unwrap());
            result(&mut conn, "parser", sub, "pending");
            item(&mut conn, sub, "[X] new - 02", "no_match");
            assert!(!parser_due(&mut conn, sub).unwrap());
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn conflicts_of_a_subscription_are_grouped_per_episode() {
            let mut conn = test_db::conn();
            let sub = subscription(&mut conn, "filter");
            let work = insert(&mut conn, "INSERT INTO anime_works (title, created_at, updated_at) VALUES ('自動觸發測試', now(), now()) RETURNING work_id AS id");
            let season = insert(&mut conn, "INSERT INTO seasons (year, season, created_at) VALUES (2095, 'winter', now()) ON CONFLICT (year, season) DO UPDATE SET year = EXCLUDED.year RETURNING season_id AS id");
            let anime = insert(&mut conn, &format!("INSERT INTO animes (work_id, series_no, season_id, created_at, updated_at) VALUES ({work}, 1, {season}, now(), now()) RETURNING anime_id AS id"));
            let group = insert(&mut conn, "INSERT INTO subtitle_groups (group_name) VALUES ('自動觸發測試組') RETURNING group_id AS id");
            for (ep, title, conflict) in [
                (51, "[G] S - 51 [繁日内嵌]", true),
                (51, "[G] S - 51 [简日内嵌]", true),
                (52, "[G] S - 52 [繁日内嵌]", false),
            ] {
                let raw = item(&mut conn, sub, title, "parsed");
                insert(&mut conn, &format!("INSERT INTO anime_links (anime_id, group_id, episode_no, title, url, source_hash, raw_item_id, conflict_flag) VALUES ({anime}, {group}, {ep}, '{title}', 'u-{title}', 'auto-trigger-{title}', {raw}, {conflict}) RETURNING link_id AS id"));
            }
            assert_eq!(
                newest_conflict(&mut conn, sub).unwrap(),
                None,
                "no conflict record yet"
            );
            insert(&mut conn, &format!("INSERT INTO anime_link_conflicts (anime_id, group_id, episode_no, resolution_status, created_at) VALUES ({anime}, {group}, 51, 'unresolved', now()) RETURNING conflict_id AS id"));

            assert!(newest_conflict(&mut conn, sub).unwrap().is_some());
            let groups = conflict_groups_for_subscription(&mut conn, sub).unwrap();
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].0.len(), 2);
        }
    }
}
