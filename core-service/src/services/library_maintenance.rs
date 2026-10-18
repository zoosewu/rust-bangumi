//! 媒體庫整理（spec 第 10 節最後一段）：依目前的系列名稱重新同步所有已同步的集數，
//! 以及刪除回填合併掉的重複檔案。兩者都預設 dry-run。

use diesel::prelude::*;
use diesel::sql_types::{Integer, Text};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::db::DbPool;
use crate::models::Download;
use crate::schema::downloads;
use crate::services::SyncService;

/// 一集等 viewer 回報的最長時間
const RESYNC_TIMEOUT: Duration = Duration::from_secs(180);
const RESYNC_POLL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, QueryableByName)]
pub struct ResyncItem {
    #[diesel(sql_type = Integer)]
    pub download_id: i32,
    #[diesel(sql_type = Text)]
    pub file_path: String,
    /// 重新同步後的系列資料夾名稱（作品名稱）
    #[diesel(sql_type = Text)]
    pub series_title: String,
    #[diesel(sql_type = Integer)]
    pub series_no: i32,
    #[diesel(sql_type = Integer)]
    pub episode_no: i32,
}

/// 所有已同步、有媒體庫路徑的集數。多筆下載指向同一個檔案時只同步一筆
/// （搬走之後另一筆就找不到檔案了），其餘在報表中列為 `shared_with`。
pub fn resync_plan(conn: &mut PgConnection) -> QueryResult<(Vec<ResyncItem>, Vec<(i32, i32)>)> {
    let items: Vec<ResyncItem> = diesel::sql_query(
        "SELECT d.download_id, d.file_path, w.title AS series_title, a.series_no, l.episode_no \
         FROM downloads d \
         JOIN anime_links l ON l.link_id = d.link_id \
         JOIN animes a ON a.anime_id = l.anime_id \
         JOIN anime_works w ON w.work_id = a.work_id \
         WHERE d.status = 'synced' AND d.file_path IS NOT NULL \
         ORDER BY w.title, a.series_no, l.episode_no, d.download_id",
    )
    .load(conn)?;
    let mut first_by_path: std::collections::HashMap<String, i32> = Default::default();
    let mut unique = Vec::new();
    let mut shared = Vec::new();
    for item in items {
        match first_by_path.get(&item.file_path) {
            Some(&first) => shared.push((item.download_id, first)),
            None => {
                first_by_path.insert(item.file_path.clone(), item.download_id);
                unique.push(item);
            }
        }
    }
    Ok((unique, shared))
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ResyncProgress {
    pub running: bool,
    pub total: usize,
    pub done: usize,
    pub failed: Vec<ResyncFailure>,
    pub current: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResyncFailure {
    pub download_id: i32,
    pub reason: String,
}

fn progress() -> &'static Mutex<ResyncProgress> {
    static PROGRESS: std::sync::OnceLock<Mutex<ResyncProgress>> = std::sync::OnceLock::new();
    PROGRESS.get_or_init(Default::default)
}

pub fn resync_progress() -> ResyncProgress {
    progress().lock().unwrap().clone()
}

/// 開始重新同步；已經有一輪在跑時回傳 false。
pub fn start_resync(sync: Arc<SyncService>, db: DbPool, items: Vec<ResyncItem>) -> bool {
    {
        let mut p = progress().lock().unwrap();
        if p.running {
            return false;
        }
        *p = ResyncProgress {
            running: true,
            total: items.len(),
            ..Default::default()
        };
    }
    tokio::spawn(async move {
        for item in items {
            progress().lock().unwrap().current = Some(item.download_id);
            if let Err(reason) = resync_one(&sync, &db, item.download_id).await {
                tracing::warn!(
                    "Library resync of download {} failed: {}",
                    item.download_id,
                    reason
                );
                progress().lock().unwrap().failed.push(ResyncFailure {
                    download_id: item.download_id,
                    reason,
                });
            }
            progress().lock().unwrap().done += 1;
        }
        let mut p = progress().lock().unwrap();
        p.running = false;
        p.current = None;
        tracing::info!(
            "Library resync finished: {} episodes, {} failed",
            p.total,
            p.failed.len()
        );
    });
    true
}

/// 一次一集：送出 resync 後等 viewer 回報（狀態離開 resyncing）才換下一集，
/// 避免大量搬檔同時打在外接硬碟上（稽核 E6）。
async fn resync_one(sync: &SyncService, db: &DbPool, download_id: i32) -> Result<(), String> {
    let download: Download = {
        let mut conn = db.get().map_err(|e| e.to_string())?;
        downloads::table
            .find(download_id)
            .first(&mut conn)
            .map_err(|e| e.to_string())?
    };
    if download.status != "synced" {
        return Err(format!("status changed to {}", download.status));
    }
    if !sync.notify_viewer_resync(&download).await? {
        return Err("no viewer available".to_string());
    }
    let deadline = tokio::time::Instant::now() + RESYNC_TIMEOUT;
    loop {
        tokio::time::sleep(RESYNC_POLL).await;
        let status: String = {
            let mut conn = db.get().map_err(|e| e.to_string())?;
            downloads::table
                .find(download_id)
                .select(downloads::status)
                .first(&mut conn)
                .map_err(|e| e.to_string())?
        };
        match status.as_str() {
            "resyncing" if tokio::time::Instant::now() < deadline => continue,
            "resyncing" => return Err("viewer did not report back in time".to_string()),
            "synced" => return Ok(()),
            other => return Err(format!("viewer reported failure (status {})", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, QueryableByName)]
pub struct DuplicateFile {
    #[diesel(sql_type = Integer)]
    pub download_id: i32,
    #[diesel(sql_type = Text)]
    pub file_path: String,
    #[diesel(sql_type = Integer)]
    pub kept_download_id: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    pub kept_file_path: Option<String>,
}

/// 回填合併掉、還留在媒體庫的重複檔案。保留的那份與之共用同一個檔案時不列入（刪了會刪到保留的檔案）。
pub fn merged_duplicate_files(conn: &mut PgConnection) -> QueryResult<Vec<DuplicateFile>> {
    diesel::sql_query(
        "SELECT d.download_id, d.file_path, k.download_id AS kept_download_id, \
                k.file_path AS kept_file_path \
         FROM downloads d \
         JOIN downloads k ON k.download_id = \
              substring(d.error_message FROM 'merged duplicate of download ([0-9]+)')::int \
         WHERE d.status = 'cancelled' AND d.file_path IS NOT NULL \
           AND NOT EXISTS (SELECT 1 FROM downloads o \
                           WHERE o.file_path = d.file_path AND o.download_id <> d.download_id \
                             AND o.status NOT IN ('cancelled', 'failed')) \
         ORDER BY d.file_path",
    )
    .load(conn)
}

/// 刪除重複檔案（viewer `/delete`）；刪成功的下載清掉 `file_path`，避免重複處理。
pub async fn delete_duplicate_files(
    sync: &SyncService,
    db: &DbPool,
    files: &[DuplicateFile],
) -> Result<shared::ViewerDeleteResponse, String> {
    if files.is_empty() {
        return Ok(shared::ViewerDeleteResponse { deleted: vec![] });
    }
    let response = sync
        .notify_viewer_delete(files.iter().map(|f| f.download_id).collect())
        .await?;
    let deleted: Vec<i32> = response
        .deleted
        .iter()
        .filter(|r| r.success)
        .map(|r| r.download_id)
        .collect();
    let mut conn = db.get().map_err(|e| e.to_string())?;
    diesel::update(downloads::table.filter(downloads::download_id.eq_any(&deleted)))
        .set((
            downloads::file_path.eq(None::<String>),
            downloads::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|e| e.to_string())?;
    Ok(response)
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

    /// 一季底下的一集與它的下載
    fn episode(
        conn: &mut PgConnection,
        anime: i32,
        group: i32,
        ep: i32,
        status: &str,
        path: Option<&str>,
        note: Option<&str>,
    ) -> i32 {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let link = insert(conn, &format!("INSERT INTO anime_links (anime_id, group_id, episode_no, url, source_hash) VALUES ({anime}, {group}, {ep}, 'u{n}', 'maintenance-test-{n}') RETURNING link_id AS id"));
        let path = path.map(|p| format!("'{p}'")).unwrap_or("NULL".to_string());
        let note = note.map(|n| format!("'{n}'")).unwrap_or("NULL".to_string());
        insert(conn, &format!("INSERT INTO downloads (link_id, downloader_type, status, file_path, error_message) VALUES ({link}, 'qbittorrent', '{status}', {path}, {note}) RETURNING download_id AS id"))
    }

    fn season(conn: &mut PgConnection) -> (i32, i32) {
        let work = insert(conn, "INSERT INTO anime_works (title, created_at, updated_at) VALUES ('整理測試', now(), now()) RETURNING work_id AS id");
        let season = insert(conn, "INSERT INTO seasons (year, season, created_at) VALUES (2098, 'winter', now()) RETURNING season_id AS id");
        let anime = insert(conn, &format!("INSERT INTO animes (work_id, series_no, season_id, created_at, updated_at) VALUES ({work}, 2, {season}, now(), now()) RETURNING anime_id AS id"));
        let group = insert(conn, "INSERT INTO subtitle_groups (group_name) VALUES ('整理測試組') RETURNING group_id AS id");
        (anime, group)
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn the_resync_plan_covers_synced_episodes_once_per_file() {
        let mut conn = test_db::conn();
        let (anime, group) = season(&mut conn);
        let first = episode(
            &mut conn,
            anime,
            group,
            1,
            "synced",
            Some("/m/a/e1.mkv"),
            None,
        );
        let sharing = episode(
            &mut conn,
            anime,
            group,
            1,
            "synced",
            Some("/m/a/e1.mkv"),
            None,
        );
        let second = episode(
            &mut conn,
            anime,
            group,
            2,
            "synced",
            Some("/m/a/e2.mkv"),
            None,
        );
        episode(
            &mut conn,
            anime,
            group,
            3,
            "completed",
            Some("/downloads/e3.mkv"),
            None,
        );
        episode(
            &mut conn,
            anime,
            group,
            4,
            "cancelled",
            Some("/m/a/e4.mkv"),
            None,
        );

        let (items, shared) = resync_plan(&mut conn).unwrap();
        let mine: Vec<&ResyncItem> = items
            .iter()
            .filter(|i| i.series_title == "整理測試")
            .collect();
        assert_eq!(
            mine.iter().map(|i| i.download_id).collect::<Vec<_>>(),
            vec![first, second]
        );
        assert_eq!(mine[0].series_no, 2);
        assert!(shared.contains(&(sharing, first)));
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn merged_duplicates_are_listed_with_the_copy_that_was_kept() {
        let mut conn = test_db::conn();
        let (anime, group) = season(&mut conn);
        let kept = episode(
            &mut conn,
            anime,
            group,
            1,
            "synced",
            Some("/m/new/e1.mkv"),
            None,
        );
        let dup = episode(
            &mut conn,
            anime,
            group,
            1,
            "cancelled",
            Some("/m/old/e1.mkv"),
            Some(&format!("merged duplicate of download {kept}")),
        );
        // 沒有檔案的不列入
        episode(
            &mut conn,
            anime,
            group,
            2,
            "cancelled",
            None,
            Some(&format!("merged duplicate of download {kept}")),
        );
        // 與保留的那份共用同一個檔案時不列入
        episode(
            &mut conn,
            anime,
            group,
            3,
            "cancelled",
            Some("/m/new/e1.mkv"),
            Some(&format!("merged duplicate of download {kept}")),
        );

        let files = merged_duplicate_files(&mut conn).unwrap();
        let mine: Vec<&DuplicateFile> = files
            .iter()
            .filter(|f| f.kept_download_id == kept)
            .collect();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].download_id, dup);
        assert_eq!(mine[0].kept_file_path.as_deref(), Some("/m/new/e1.mkv"));
    }
}
