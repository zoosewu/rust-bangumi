//! 下載目錄的孤兒項目：不屬於任何 downloader 仍保留的 torrent、也沒有需要檔案的下載引用。
//!
//! core 只能唯讀掛載下載目錄，所以由 core 判斷、viewer（可寫入）刪除。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use diesel::prelude::*;
use serde::Serialize;

use crate::db::DbPool;
use crate::models::ModuleTypeEnum;
use crate::schema::{downloads, service_modules};
use crate::services::sync_service::{tracked_torrents, NEEDS_DOWNLOAD_FILES};

/// 最近有變動的項目不算孤兒：可能是剛加入、還沒記錄到下載的 torrent。
pub const ORPHAN_MIN_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OrphanEntry {
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    /// 項目（資料夾含裡面的檔案）最近一次修改的時間
    pub modified_at: chrono::DateTime<chrono::Utc>,
}

/// 下載目錄第一層的項目
#[derive(Debug, Clone)]
struct DirEntryInfo {
    name: String,
    is_dir: bool,
    size_bytes: u64,
    newest_mtime: SystemTime,
}

/// 下載目錄的位置（core 與 viewer 掛載在同一路徑）
pub fn downloads_dir() -> PathBuf {
    PathBuf::from(std::env::var("DOWNLOADS_DIR").unwrap_or_else(|_| "/downloads".to_string()))
}

/// `path` 落在下載目錄第一層的哪個項目；不在目錄底下時為 None。
fn top_level_name(dir: &Path, path: &str) -> Option<String> {
    Path::new(path)
        .strip_prefix(dir)
        .ok()?
        .components()
        .next()
        .and_then(|c| match c {
            std::path::Component::Normal(name) => name.to_str().map(str::to_string),
            _ => None,
        })
}

/// 沒被保護、也超過 `min_age` 沒變動的項目（隱藏檔不列入）
fn orphans(
    entries: Vec<DirEntryInfo>,
    protected: &HashSet<String>,
    now: SystemTime,
    min_age: Duration,
) -> Vec<OrphanEntry> {
    let mut found: Vec<OrphanEntry> = entries
        .into_iter()
        .filter(|e| !e.name.starts_with('.') && !protected.contains(&e.name))
        .filter(|e| {
            now.duration_since(e.newest_mtime)
                .is_ok_and(|age| age >= min_age)
        })
        .map(|e| OrphanEntry {
            name: e.name,
            is_dir: e.is_dir,
            size_bytes: e.size_bytes,
            modified_at: e.newest_mtime.into(),
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// 列出下載目錄第一層（同步 IO，放在 spawn_blocking 裡跑）
fn scan_dir(dir: &Path) -> std::io::Result<Vec<DirEntryInfo>> {
    std::fs::read_dir(dir)?
        .map(|entry| {
            let entry = entry?;
            let metadata = std::fs::symlink_metadata(entry.path())?;
            let (size_bytes, newest_mtime) = if metadata.is_dir() {
                tree_size_and_mtime(&entry.path())
            } else {
                (metadata.len(), metadata.modified()?)
            };
            Ok(DirEntryInfo {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: metadata.is_dir(),
                size_bytes,
                newest_mtime,
            })
        })
        .collect()
}

/// 資料夾的總大小與裡面最新的修改時間（讀不到的項目略過；不跟隨符號連結）
fn tree_size_and_mtime(dir: &Path) -> (u64, SystemTime) {
    let own = std::fs::symlink_metadata(dir)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let metadata = std::fs::symlink_metadata(entry.path()).ok()?;
            Some(if metadata.is_dir() {
                tree_size_and_mtime(&entry.path())
            } else {
                (
                    metadata.len(),
                    metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                )
            })
        })
        .fold((0, own), |(size, newest), (s, m)| (size + s, newest.max(m)))
}

/// 還有人在用的第一層項目：仍需要檔案的下載記錄的路徑，以及 downloader 仍保留的 torrent 的內容。
/// 任何一個 downloader 查不到時回傳錯誤，不冒險判斷。
async fn protected_names(
    db: &DbPool,
    http_client: &reqwest::Client,
    dir: &Path,
) -> Result<HashSet<String>, String> {
    let (rows, downloaders) = {
        let mut conn = db.get().map_err(|e| e.to_string())?;
        let rows: Vec<(Option<i32>, Option<String>, String, Option<String>)> = downloads::table
            .select((
                downloads::module_id,
                downloads::torrent_hash,
                downloads::status,
                downloads::file_path,
            ))
            .load(&mut conn)
            .map_err(|e| format!("Failed to query downloads: {}", e))?;
        let downloaders: Vec<(i32, String)> = service_modules::table
            .filter(service_modules::module_type.eq(ModuleTypeEnum::Downloader))
            .select((service_modules::module_id, service_modules::base_url))
            .load(&mut conn)
            .map_err(|e| format!("Failed to query downloaders: {}", e))?;
        (rows, downloaders)
    };

    let mut protected: HashSet<String> = rows
        .iter()
        .filter(|(_, _, status, _)| NEEDS_DOWNLOAD_FILES.contains(&status.as_str()))
        .filter_map(|(_, _, _, path)| path.as_deref())
        .filter_map(|path| top_level_name(dir, path))
        .collect();

    for (module_id, base_url) in downloaders {
        let hashes: Vec<String> = rows
            .iter()
            .filter(|(m, _, _, _)| *m == Some(module_id))
            .filter_map(|(_, hash, _, _)| hash.clone().filter(|h| !h.is_empty()))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if hashes.is_empty() {
            continue;
        }
        let tracked = tracked_torrents(http_client, &base_url, &hashes)
            .await
            .map_err(|e| format!("無法確認 {} 保留的 torrent：{}", base_url, e))?;
        protected.extend(
            tracked
                .iter()
                .flat_map(|t| t.content_path.iter().chain(t.files.iter()))
                .filter_map(|path| top_level_name(dir, path)),
        );
    }
    Ok(protected)
}

/// 目前的孤兒項目
pub async fn find_orphans(
    db: &DbPool,
    http_client: &reqwest::Client,
    dir: &Path,
) -> Result<Vec<OrphanEntry>, String> {
    let protected = protected_names(db, http_client, dir).await?;
    let scan_path = dir.to_path_buf();
    let entries = tokio::task::spawn_blocking(move || scan_dir(&scan_path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("無法讀取下載目錄 {}：{}", dir.display(), e))?;
    Ok(orphans(
        entries,
        &protected,
        SystemTime::now(),
        ORPHAN_MIN_AGE,
    ))
}

/// 刪除孤兒項目（`names` 為 None 時刪除全部）。刪除前重新掃描，只刪仍是孤兒的項目。
/// 回傳實際送去刪除的項目與 viewer 的結果。
pub async fn delete_orphans(
    db: &DbPool,
    http_client: &reqwest::Client,
    dir: &Path,
    names: Option<&[String]>,
) -> Result<(Vec<OrphanEntry>, shared::DownloadDirCleanupResponse), String> {
    let targets: Vec<OrphanEntry> = find_orphans(db, http_client, dir)
        .await?
        .into_iter()
        .filter(|o| names.is_none_or(|names| names.contains(&o.name)))
        .collect();
    if targets.is_empty() {
        return Ok((
            targets,
            shared::DownloadDirCleanupResponse { results: vec![] },
        ));
    }

    let viewer_url: String = {
        let mut conn = db.get().map_err(|e| e.to_string())?;
        service_modules::table
            .filter(service_modules::is_enabled.eq(true))
            .filter(service_modules::module_type.eq(ModuleTypeEnum::Viewer))
            .order(service_modules::priority.desc())
            .select(service_modules::base_url)
            .first(&mut conn)
            .optional()
            .map_err(|e| format!("Failed to query viewers: {}", e))?
            .ok_or("沒有可用的 viewer，無法刪除")?
    };

    let request = shared::DownloadDirCleanupRequest {
        names: targets.iter().map(|o| o.name.clone()).collect(),
    };
    let response = http_client
        .post(format!("{}/downloads/cleanup", viewer_url))
        .json(&request)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("Failed to reach viewer: {}", e))?;
    if !response.status().is_success() {
        return Err(format!(
            "Viewer returned status {} for cleanup",
            response.status()
        ));
    }
    let result = response
        .json::<shared::DownloadDirCleanupResponse>()
        .await
        .map_err(|e| format!("Failed to parse viewer cleanup response: {}", e))?;
    Ok((targets, result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_map_to_their_top_level_entry() {
        let dir = Path::new("/downloads");
        assert_eq!(
            top_level_name(dir, "/downloads/[G] Show - 01.mkv").as_deref(),
            Some("[G] Show - 01.mkv")
        );
        assert_eq!(
            top_level_name(dir, "/downloads/Batch/Fonts/a.ttf").as_deref(),
            Some("Batch")
        );
        assert_eq!(top_level_name(dir, "/media/jellyfin/X/a.mkv"), None);
        assert_eq!(top_level_name(dir, "/downloads"), None);
    }

    fn entry(name: &str, age_hours: u64, now: SystemTime) -> DirEntryInfo {
        DirEntryInfo {
            name: name.to_string(),
            is_dir: false,
            size_bytes: 1,
            newest_mtime: now - Duration::from_secs(age_hours * 3600),
        }
    }

    #[test]
    fn orphans_skip_protected_recent_and_hidden_entries() {
        let now = SystemTime::now();
        let entries = vec![
            entry("old leftover.mkv", 48, now),
            entry("still downloading.mkv", 48, now),
            entry("just added.mkv", 1, now),
            entry(".DS_Store", 48, now),
            entry("another leftover", 25, now),
        ];
        let protected = HashSet::from(["still downloading.mkv".to_string()]);
        let names: Vec<String> = orphans(entries, &protected, now, ORPHAN_MIN_AGE)
            .into_iter()
            .map(|o| o.name)
            .collect();
        assert_eq!(names, ["another leftover", "old leftover.mkv"]);
    }

    #[test]
    fn a_folder_is_as_new_as_its_newest_file() {
        let dir = tempfile::tempdir().unwrap();
        let batch = dir.path().join("Batch");
        std::fs::create_dir_all(batch.join("Fonts")).unwrap();
        std::fs::write(batch.join("01.mkv"), b"12345").unwrap();
        std::fs::write(batch.join("Fonts/a.ttf"), b"123").unwrap();
        std::fs::write(dir.path().join("single.mkv"), b"12").unwrap();

        let mut entries = scan_dir(dir.path()).unwrap();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(entries.len(), 2);
        assert!(entries[0].is_dir && entries[0].size_bytes == 8);
        assert!(!entries[1].is_dir && entries[1].size_bytes == 2);
        // 剛寫入的檔案讓資料夾也算是最近有變動
        let found = orphans(entries, &HashSet::new(), SystemTime::now(), ORPHAN_MIN_AGE);
        assert!(found.is_empty());
    }

    mod with_db {
        use super::super::*;
        use crate::services::identity::test_db;
        use crate::services::{SyncService, WebhookService};
        use axum::extract::{Path as UrlPath, Query, State};
        use axum::routing::{delete, get};
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};

        /// 假的 downloader：保留 `torrents`（hash → (狀態, content_path)），記錄被移除的 hash 與參數
        #[derive(Clone, Default)]
        struct FakeDownloader {
            torrents: Arc<Mutex<HashMap<String, (String, String)>>>,
            removed: Arc<Mutex<Vec<(String, String)>>>,
        }

        async fn query(
            State(fake): State<FakeDownloader>,
            Query(q): Query<HashMap<String, String>>,
        ) -> axum::Json<shared::StatusQueryResponse> {
            let torrents = fake.torrents.lock().unwrap();
            let statuses = q["hashes"]
                .split(',')
                .map(|h| {
                    let (status, path) = torrents
                        .get(h)
                        .cloned()
                        .unwrap_or(("not_found".to_string(), String::new()));
                    shared::DownloadStatusItem {
                        hash: h.to_string(),
                        status,
                        progress: 1.0,
                        size: 1,
                        content_path: (!path.is_empty()).then_some(path),
                        files: vec![],
                    }
                })
                .collect();
            axum::Json(shared::StatusQueryResponse { statuses })
        }

        async fn remove(
            State(fake): State<FakeDownloader>,
            UrlPath(hash): UrlPath<String>,
            Query(q): Query<HashMap<String, String>>,
        ) {
            fake.torrents.lock().unwrap().remove(&hash);
            fake.removed
                .lock()
                .unwrap()
                .push((hash, q.get("delete_files").cloned().unwrap_or_default()));
        }

        async fn serve(fake: FakeDownloader) -> String {
            let app = axum::Router::new()
                .route("/downloads", get(query))
                .route("/downloads/:hash", delete(remove))
                .with_state(fake);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            format!("http://{addr}")
        }

        #[derive(QueryableByName)]
        struct Id {
            #[diesel(sql_type = diesel::sql_types::Integer)]
            id: i32,
        }

        fn insert(conn: &mut PgConnection, sql: &str) -> i32 {
            diesel::sql_query(sql).get_result::<Id>(conn).unwrap().id
        }

        /// 一筆指定 torrent 與狀態的下載（同一部動畫底下）
        fn download(
            conn: &mut PgConnection,
            module: i32,
            hash: &str,
            status: &str,
            path: Option<&str>,
        ) {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let (anime, group) = (anime_id(conn), group_id(conn));
            let link = insert(conn, &format!("INSERT INTO anime_links (anime_id, group_id, episode_no, url, source_hash) VALUES ({anime}, {group}, {n}, 'u{n}', 'cleanup-test-{n}') RETURNING link_id AS id"));
            let path = path.map(|p| format!("'{p}'")).unwrap_or("NULL".to_string());
            insert(conn, &format!("INSERT INTO downloads (link_id, downloader_type, status, module_id, torrent_hash, file_path) VALUES ({link}, 'qbittorrent', '{status}', {module}, '{hash}', {path}) RETURNING download_id AS id"));
        }

        fn anime_id(conn: &mut PgConnection) -> i32 {
            diesel::sql_query("SELECT anime_id AS id FROM animes a JOIN anime_works w USING (work_id) WHERE w.title = '清理測試'")
                .get_result::<Id>(conn)
                .map(|r| r.id)
                .unwrap_or_else(|_| {
                    let work = insert(conn, "INSERT INTO anime_works (title, created_at, updated_at) VALUES ('清理測試', now(), now()) RETURNING work_id AS id");
                    let season = insert(conn, "INSERT INTO seasons (year, season, created_at) VALUES (2097, 'winter', now()) RETURNING season_id AS id");
                    insert(conn, &format!("INSERT INTO animes (work_id, series_no, season_id, created_at, updated_at) VALUES ({work}, 1, {season}, now(), now()) RETURNING anime_id AS id"))
                })
        }

        fn group_id(conn: &mut PgConnection) -> i32 {
            diesel::sql_query("SELECT group_id AS id FROM subtitle_groups WHERE group_name = '清理測試組'")
                .get_result::<Id>(conn)
                .map(|r| r.id)
                .unwrap_or_else(|_| insert(conn, "INSERT INTO subtitle_groups (group_name) VALUES ('清理測試組') RETURNING group_id AS id"))
        }

        #[tokio::test]
        #[ignore = "needs DATABASE_TEST_URL"]
        async fn settled_torrents_go_with_their_files_and_the_rest_stay_protected() {
            let fake = FakeDownloader::default();
            let base_url = serve(fake.clone()).await;
            {
                let mut torrents = fake.torrents.lock().unwrap();
                for (hash, status, path) in [
                    ("aaaa", "completed", "/downloads/Settled Batch"),
                    ("bbbb", "completed", "/downloads/Half Synced Batch"),
                    ("dddd", "downloading", "/downloads/Readded.mkv"),
                ] {
                    torrents.insert(hash.to_string(), (status.to_string(), path.to_string()));
                }
            }

            let pool = test_db::pool();
            {
                let mut conn = pool.get().unwrap();
                diesel::sql_query("UPDATE service_modules SET is_enabled = false WHERE module_type = 'downloader'")
                    .execute(&mut conn)
                    .unwrap();
                let module = insert(&mut conn, &format!("INSERT INTO service_modules (module_type, name, version, is_enabled, priority, base_url, created_at, updated_at) VALUES ('downloader', 'fake-qb', '1', true, 50, '{base_url}', now(), now()) RETURNING module_id AS id"));
                download(&mut conn, module, "aaaa", "synced", None);
                download(&mut conn, module, "aaaa", "batch_failed", None);
                download(&mut conn, module, "bbbb", "synced", None);
                download(
                    &mut conn,
                    module,
                    "bbbb",
                    "completed",
                    Some("/downloads/Half Synced Batch/02.mkv"),
                );
                download(&mut conn, module, "cccc", "cancelled", None);
                download(&mut conn, module, "dddd", "synced", None);
                download(
                    &mut conn,
                    module,
                    "eeee",
                    "sync_failed",
                    Some("/downloads/Waiting.mkv"),
                );
            }

            let sync = SyncService::new(pool.clone(), Arc::new(WebhookService::new(pool.clone())));
            assert_eq!(sync.release_settled_torrents().await.unwrap(), 1);
            assert_eq!(
                *fake.removed.lock().unwrap(),
                [("aaaa".to_string(), "true".to_string())],
                "only the settled torrent goes, files included; a busy one is left alone"
            );

            let protected =
                protected_names(&pool, &reqwest::Client::new(), Path::new("/downloads"))
                    .await
                    .unwrap();
            assert_eq!(
                protected,
                HashSet::from(
                    ["Half Synced Batch", "Readded.mkv", "Waiting.mkv"].map(String::from)
                )
            );
        }
    }
}
