//! 一集只留一個版本：新採用的版本同步進媒體庫之後，同一集已不採用的版本留在媒體庫的檔案要移除。
//!
//! 舊版本的檔案留到新版本同步完成才移除，換版本期間這一集不會沒有檔案。

use diesel::prelude::*;
use diesel::sql_types::{Integer, Text};

use crate::schema::downloads;

#[derive(Debug, PartialEq, QueryableByName)]
pub struct Replaced {
    #[diesel(sql_type = Integer)]
    pub download_id: i32,
    /// 舊版本在媒體庫的路徑
    #[diesel(sql_type = Text)]
    pub file_path: String,
}

/// 下載 `download_id` 同步後要取代的下載：同季同一集、其他連結、已同步且有媒體庫路徑，
/// 而且那個連結已不採用（被換掉 = resolved，或被過濾且不是手動指定）。
/// `download_id` 自己的連結不是採用中的版本時，什麼都不取代。
pub fn replaced_by(conn: &mut PgConnection, download_id: i32) -> QueryResult<Vec<Replaced>> {
    diesel::sql_query(
        "SELECT o.download_id, o.file_path \
         FROM downloads d \
         JOIN anime_links l ON l.link_id = d.link_id \
         JOIN anime_links ol ON ol.anime_id = l.anime_id AND ol.episode_no = l.episode_no \
                            AND ol.link_id <> l.link_id \
         JOIN downloads o ON o.link_id = ol.link_id \
         WHERE d.download_id = $1 \
           AND l.link_status = 'active' AND (NOT l.filtered_flag OR l.manual_choice) \
           AND o.status = 'synced' AND o.file_path IS NOT NULL \
           AND (ol.link_status = 'resolved' OR (ol.filtered_flag AND NOT ol.manual_choice)) \
         ORDER BY o.download_id",
    )
    .bind::<Integer, _>(download_id)
    .load(conn)
}

/// 被取代、檔案已移除（或已被新版本覆蓋）的下載：標為取消、清掉媒體庫路徑。只改仍是 synced 的。
pub fn retire(conn: &mut PgConnection, replaced: &[i32], by: i32) -> QueryResult<usize> {
    diesel::update(
        downloads::table
            .filter(downloads::download_id.eq_any(replaced))
            .filter(downloads::status.eq("synced")),
    )
    .set((
        downloads::status.eq("cancelled"),
        downloads::file_path.eq(None::<String>),
        downloads::error_message.eq(Some(format!("replaced by download {}", by))),
        downloads::updated_at.eq(chrono::Utc::now().naive_utc()),
    ))
    .execute(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::identity::test_db;

    #[derive(QueryableByName)]
    struct Id {
        #[diesel(sql_type = Integer)]
        id: i32,
    }

    fn insert(conn: &mut PgConnection, sql: &str) -> i32 {
        diesel::sql_query(sql).get_result::<Id>(conn).unwrap().id
    }

    /// 一季（每次呼叫用不同的名稱，測試可平行執行）
    fn season(conn: &mut PgConnection) -> i32 {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let work = insert(conn, &format!("INSERT INTO anime_works (title, created_at, updated_at) VALUES ('取代測試{n}', now(), now()) RETURNING work_id AS id"));
        let season = insert(conn, "INSERT INTO seasons (year, season, created_at) VALUES (2097, 'winter', now()) ON CONFLICT (year, season) DO UPDATE SET year = EXCLUDED.year RETURNING season_id AS id");
        insert(conn, &format!("INSERT INTO animes (work_id, series_no, season_id, created_at, updated_at) VALUES ({work}, 1, {season}, now(), now()) RETURNING anime_id AS id"))
    }

    fn group(conn: &mut PgConnection, name: &str) -> i32 {
        insert(conn, &format!("INSERT INTO subtitle_groups (group_name) VALUES ('{name}') ON CONFLICT (group_name) DO UPDATE SET group_name = EXCLUDED.group_name RETURNING group_id AS id"))
    }

    /// 一個版本與它的下載；回傳 (link, download)
    fn version(
        conn: &mut PgConnection,
        anime: i32,
        group: i32,
        ep: i32,
        state: (&str, bool, bool),
        download: (&str, Option<&str>),
    ) -> (i32, i32) {
        let (link_status, filtered, manual) = state;
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let key = format!("{anime}-{group}-{ep}-{n}");
        let link = insert(conn, &format!("INSERT INTO anime_links (anime_id, group_id, episode_no, title, url, source_hash, filtered_flag, link_status, manual_choice) VALUES ({anime}, {group}, {ep}, 't', 'u-{key}', 'replace-test-{key}', {filtered}, '{link_status}', {manual}) RETURNING link_id AS id"));
        let (status, path) = download;
        let path = path.map_or("NULL".to_string(), |p| format!("'{p}'"));
        let dl = insert(conn, &format!("INSERT INTO downloads (link_id, downloader_type, status, file_path) VALUES ({link}, 'qbittorrent', '{status}', {path}) RETURNING download_id AS id"));
        (link, dl)
    }

    const ADOPTED: (&str, bool, bool) = ("active", false, false);
    const SET_ASIDE: (&str, bool, bool) = ("resolved", false, false);
    const FILTERED: (&str, bool, bool) = ("active", true, false);

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn the_adopted_version_replaces_set_aside_and_filtered_versions_in_the_library() {
        let mut conn = test_db::conn();
        let anime = season(&mut conn);
        let (a, b, c) = (
            group(&mut conn, "取代組A"),
            group(&mut conn, "取代組B"),
            group(&mut conn, "取代組C"),
        );
        let (_, new) = version(
            &mut conn,
            anime,
            a,
            1,
            ADOPTED,
            ("syncing", Some("/downloads/new.mkv")),
        );
        let (_, set_aside) = version(
            &mut conn,
            anime,
            b,
            1,
            SET_ASIDE,
            ("synced", Some("/media/S01E01.mp4")),
        );
        let (_, filtered) = version(
            &mut conn,
            anime,
            c,
            1,
            FILTERED,
            ("synced", Some("/media/S01E01.mkv")),
        );
        // 不取代：還沒進媒體庫的、別集的、手動指定的過濾版本不會和採用中的版本同時存在，但也不動
        version(&mut conn, anime, b, 1, SET_ASIDE, ("cancelled", None));
        version(
            &mut conn,
            anime,
            b,
            2,
            SET_ASIDE,
            ("synced", Some("/media/S01E02.mp4")),
        );

        assert_eq!(
            replaced_by(&mut conn, new).unwrap(),
            vec![
                Replaced {
                    download_id: set_aside,
                    file_path: "/media/S01E01.mp4".to_string()
                },
                Replaced {
                    download_id: filtered,
                    file_path: "/media/S01E01.mkv".to_string()
                },
            ]
        );
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn a_version_that_is_not_adopted_replaces_nothing() {
        let mut conn = test_db::conn();
        let anime = season(&mut conn);
        let (a, b) = (group(&mut conn, "取代組A"), group(&mut conn, "取代組B"));
        let (_, kept) = version(
            &mut conn,
            anime,
            a,
            1,
            ADOPTED,
            ("synced", Some("/media/S01E01.mp4")),
        );
        let (_, late) = version(
            &mut conn,
            anime,
            b,
            1,
            SET_ASIDE,
            ("synced", Some("/media/S01E01.mkv")),
        );
        let (_, filtered) = version(
            &mut conn,
            anime,
            b,
            1,
            FILTERED,
            ("synced", Some("/media/S01E01.avi")),
        );

        assert!(replaced_by(&mut conn, late).unwrap().is_empty());
        assert!(replaced_by(&mut conn, filtered).unwrap().is_empty());
        assert_eq!(
            replaced_by(&mut conn, kept)
                .unwrap()
                .into_iter()
                .map(|r| r.download_id)
                .collect::<Vec<_>>(),
            vec![late, filtered]
        );
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn retired_downloads_leave_the_library() {
        let mut conn = test_db::conn();
        let anime = season(&mut conn);
        let (a, b) = (group(&mut conn, "取代組A"), group(&mut conn, "取代組B"));
        let (_, new) = version(
            &mut conn,
            anime,
            a,
            1,
            ADOPTED,
            ("synced", Some("/media/S01E01.mkv")),
        );
        let (_, old) = version(
            &mut conn,
            anime,
            b,
            1,
            SET_ASIDE,
            ("synced", Some("/media/S01E01.mp4")),
        );
        let (_, downloading) = version(&mut conn, anime, b, 1, SET_ASIDE, ("downloading", None));

        assert_eq!(retire(&mut conn, &[old, downloading], new).unwrap(), 1);
        let state: (String, Option<String>, Option<String>) = downloads::table
            .find(old)
            .select((
                downloads::status,
                downloads::file_path,
                downloads::error_message,
            ))
            .first(&mut conn)
            .unwrap();
        assert_eq!(
            state,
            (
                "cancelled".to_string(),
                None,
                Some(format!("replaced by download {new}"))
            )
        );
    }
}
