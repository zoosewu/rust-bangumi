//! 使用者手動指定某一集要用的版本（同季、同字幕組、同集數的其中一個 link）。
//!
//! 手動指定的版本不受過濾規則影響；同集的其他版本（包含之後才出現的）一律不採用，
//! 所以這一集不會再產生衝突。

use diesel::prelude::*;

use crate::models::AnimeLink;
use crate::schema::{anime_link_conflicts, anime_links, downloads};

/// 已經在媒體庫或正在搬進媒體庫的下載狀態：改用別的版本時不會刪除這些檔案
const IN_LIBRARY_STATUSES: [&str; 3] = ["synced", "syncing", "resyncing"];

#[derive(Debug, PartialEq)]
pub struct VersionChoice {
    pub chosen_link_id: i32,
    /// 同集其他版本（不再採用）
    pub other_link_ids: Vec<i32>,
    /// 其他版本已在媒體庫的下載（檔案保留）
    pub kept_in_library: Vec<i32>,
}

/// 把 `link_id` 設為它那一集的手動指定版本，並把該集的衝突標為已解決。
/// 取消其他版本的下載與派送被選版本由呼叫端處理。
pub fn choose_version(conn: &mut PgConnection, link_id: i32) -> Result<VersionChoice, String> {
    conn.transaction(|conn| {
        let link: AnimeLink = anime_links::table
            .find(link_id)
            .first(conn)
            .optional()?
            .ok_or(diesel::result::Error::NotFound)?;

        let episode = anime_links::table
            .filter(anime_links::anime_id.eq(link.anime_id))
            .filter(anime_links::group_id.eq(link.group_id))
            .filter(anime_links::episode_no.eq(link.episode_no));

        let other_link_ids: Vec<i32> = episode
            .filter(anime_links::link_id.ne(link_id))
            .select(anime_links::link_id)
            .order(anime_links::link_id)
            .load(conn)?;

        diesel::update(anime_links::table.find(link_id))
            .set((
                anime_links::manual_choice.eq(true),
                anime_links::filtered_flag.eq(false),
                anime_links::conflict_flag.eq(false),
                anime_links::link_status.eq("active"),
            ))
            .execute(conn)?;

        diesel::update(anime_links::table.filter(anime_links::link_id.eq_any(&other_link_ids)))
            .set((
                anime_links::manual_choice.eq(false),
                anime_links::conflict_flag.eq(false),
                anime_links::link_status.eq("resolved"),
            ))
            .execute(conn)?;

        diesel::update(
            anime_link_conflicts::table
                .filter(anime_link_conflicts::anime_id.eq(link.anime_id))
                .filter(anime_link_conflicts::group_id.eq(link.group_id))
                .filter(anime_link_conflicts::episode_no.eq(link.episode_no)),
        )
        .set((
            anime_link_conflicts::resolution_status.eq("resolved"),
            anime_link_conflicts::chosen_link_id.eq(link_id),
            anime_link_conflicts::resolved_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .execute(conn)?;

        let kept_in_library: Vec<i32> = downloads::table
            .filter(downloads::link_id.eq_any(&other_link_ids))
            .filter(downloads::status.eq_any(IN_LIBRARY_STATUSES))
            .select(downloads::download_id)
            .order(downloads::download_id)
            .load(conn)?;

        Ok(VersionChoice {
            chosen_link_id: link_id,
            other_link_ids,
            kept_in_library,
        })
    })
    .map_err(|e: diesel::result::Error| match e {
        diesel::result::Error::NotFound => format!("link {} 不存在", link_id),
        e => e.to_string(),
    })
}

/// 有 active 手動指定版本的集數，其他 active 版本改為不採用（衝突偵測前呼叫）。
/// 回傳被改掉的 link。
pub fn defer_to_manual_choices(conn: &mut PgConnection) -> QueryResult<Vec<i32>> {
    #[derive(QueryableByName)]
    struct LinkId {
        #[diesel(sql_type = diesel::sql_types::Integer)]
        link_id: i32,
    }
    diesel::sql_query(
        "UPDATE anime_links o SET link_status = 'resolved', conflict_flag = false \
         WHERE o.link_status = 'active' AND NOT o.manual_choice \
           AND EXISTS (SELECT 1 FROM anime_links m \
                       WHERE m.manual_choice AND m.link_status = 'active' \
                         AND m.anime_id = o.anime_id AND m.group_id = o.group_id \
                         AND m.episode_no = o.episode_no) \
         RETURNING o.link_id",
    )
    .load::<LinkId>(conn)
    .map(|rows| rows.into_iter().map(|r| r.link_id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::identity::test_db;

    #[derive(QueryableByName)]
    struct Id {
        #[diesel(sql_type = diesel::sql_types::Integer)]
        id: i32,
    }

    fn insert(conn: &mut PgConnection, sql: &str) -> i32 {
        diesel::sql_query(sql).get_result::<Id>(conn).unwrap().id
    }

    /// 一季、一個字幕組（每次呼叫用不同的名稱，測試可平行執行）
    fn season(conn: &mut PgConnection) -> (i32, i32) {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let work = insert(conn, &format!("INSERT INTO anime_works (title, created_at, updated_at) VALUES ('版本測試{n}', now(), now()) RETURNING work_id AS id"));
        let season = insert(conn, "INSERT INTO seasons (year, season, created_at) VALUES (2096, 'winter', now()) ON CONFLICT (year, season) DO UPDATE SET year = EXCLUDED.year RETURNING season_id AS id");
        let anime = insert(conn, &format!("INSERT INTO animes (work_id, series_no, season_id, created_at, updated_at) VALUES ({work}, 1, {season}, now(), now()) RETURNING anime_id AS id"));
        let group = insert(conn, &format!("INSERT INTO subtitle_groups (group_name) VALUES ('版本測試組{n}') RETURNING group_id AS id"));
        (anime, group)
    }

    fn version(
        conn: &mut PgConnection,
        (anime, group): (i32, i32),
        ep: i32,
        title: &str,
        filtered: bool,
    ) -> i32 {
        insert(conn, &format!("INSERT INTO anime_links (anime_id, group_id, episode_no, title, url, source_hash, filtered_flag) VALUES ({anime}, {group}, {ep}, '{title}', 'u-{anime}-{title}', 'version-test-{anime}-{title}', {filtered}) RETURNING link_id AS id"))
    }

    fn download(conn: &mut PgConnection, link: i32, status: &str) -> i32 {
        insert(conn, &format!("INSERT INTO downloads (link_id, downloader_type, status) VALUES ({link}, 'qbittorrent', '{status}') RETURNING download_id AS id"))
    }

    fn state(conn: &mut PgConnection, link: i32) -> (String, bool, bool) {
        anime_links::table
            .find(link)
            .select((
                anime_links::link_status,
                anime_links::manual_choice,
                anime_links::filtered_flag,
            ))
            .first(conn)
            .unwrap()
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn a_filtered_version_can_be_chosen_for_its_episode() {
        let mut conn = test_db::conn();
        let s = season(&mut conn);
        let abema = version(&mut conn, s, 11, "ABEMA-11", false);
        let cr = version(&mut conn, s, 11, "CR-11", true);
        let other_episode = version(&mut conn, s, 12, "ABEMA-12", false);
        download(&mut conn, abema, "downloading");
        let old_synced = version(&mut conn, s, 11, "OLD-11", false);
        let synced_download = download(&mut conn, old_synced, "synced");
        insert(&mut conn, &format!("INSERT INTO anime_link_conflicts (anime_id, group_id, episode_no, resolution_status, created_at) VALUES ({}, {}, 11, 'unresolved', now()) RETURNING conflict_id AS id", s.0, s.1));

        let choice = choose_version(&mut conn, cr).unwrap();

        assert_eq!(
            choice,
            VersionChoice {
                chosen_link_id: cr,
                other_link_ids: vec![abema, old_synced],
                kept_in_library: vec![synced_download],
            }
        );
        assert_eq!(state(&mut conn, cr), ("active".to_string(), true, false));
        assert_eq!(
            state(&mut conn, abema),
            ("resolved".to_string(), false, false)
        );
        assert_eq!(
            state(&mut conn, other_episode),
            ("active".to_string(), false, false),
            "other episodes are untouched"
        );
        let (status, chosen): (String, Option<i32>) = anime_link_conflicts::table
            .filter(anime_link_conflicts::anime_id.eq(s.0))
            .select((
                anime_link_conflicts::resolution_status,
                anime_link_conflicts::chosen_link_id,
            ))
            .first(&mut conn)
            .unwrap();
        assert_eq!((status.as_str(), chosen), ("resolved", Some(cr)));
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn later_versions_of_a_chosen_episode_are_not_used() {
        let mut conn = test_db::conn();
        let s = season(&mut conn);
        let cr = version(&mut conn, s, 11, "CR-11", true);
        choose_version(&mut conn, cr).unwrap();
        let newer = version(&mut conn, s, 11, "CR-11-v2", false);
        let next_episode = version(&mut conn, s, 12, "CR-12", false);

        assert_eq!(defer_to_manual_choices(&mut conn).unwrap(), vec![newer]);
        assert_eq!(state(&mut conn, newer).0, "resolved");
        assert_eq!(state(&mut conn, next_episode).0, "active");
        assert!(choose_version(&mut conn, 0).unwrap_err().contains("不存在"));
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn a_reopened_conflict_forgets_the_previous_choice() {
        use crate::db::repository::{
            AnimeLinkConflictRepository, DieselAnimeLinkConflictRepository,
        };
        let pool = test_db::pool();
        let s = {
            let mut conn = pool.get().unwrap();
            let s = season(&mut conn);
            let chosen = version(&mut conn, s, 2, "TC-2", false);
            insert(&mut conn, &format!("INSERT INTO anime_link_conflicts (anime_id, group_id, episode_no, resolution_status, chosen_link_id, resolved_at, created_at) VALUES ({}, {}, 2, 'resolved', {chosen}, '2026-10-19', '2026-10-14') RETURNING conflict_id AS id", s.0, s.1));
            s
        };
        let repo = DieselAnimeLinkConflictRepository::new(pool.clone());

        let reopened = repo.upsert(s.0, s.1, 2).await.unwrap();
        assert_eq!(reopened.resolution_status, "unresolved");
        assert_eq!(
            (reopened.chosen_link_id, reopened.resolved_at),
            (None, None)
        );
        assert!(
            reopened.created_at
                > chrono::NaiveDate::from_ymd_opt(2026, 10, 20)
                    .unwrap()
                    .into()
        );

        // 仍未解決時再偵測一次，偵測時間不變
        let again = repo.upsert(s.0, s.1, 2).await.unwrap();
        assert_eq!(again.created_at, reopened.created_at);
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn filter_rules_do_not_filter_out_a_chosen_version() {
        let mut conn = test_db::conn();
        let s = season(&mut conn);
        let cr = version(&mut conn, s, 11, "CR-11", true);
        let cr_12 = version(&mut conn, s, 12, "CR-12", false);
        choose_version(&mut conn, cr).unwrap();
        insert(&mut conn, &format!("INSERT INTO filter_rules (target_type, target_id, rule_order, is_positive, regex_pattern, created_at, updated_at) VALUES ('anime', {}, 1, false, 'CR', now(), now()) RETURNING rule_id AS id", s.0));

        let recalc = crate::services::filter_recalc::recalculate_filtered_flags(
            &mut conn,
            crate::models::FilterTargetType::Anime,
            Some(s.0),
        )
        .unwrap();
        assert_eq!(recalc.newly_filtered, vec![cr_12]);
        assert!(!state(&mut conn, cr).2);
    }
}
