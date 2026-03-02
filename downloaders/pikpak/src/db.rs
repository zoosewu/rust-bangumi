// downloaders/pikpak/src/db.rs
//! SQLite persistence layer: maps content hash → PikPak task state.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct DownloadRecord {
    pub hash: String,
    pub task_id: Option<String>,
    pub file_id: Option<String>,
    pub url: String,
    pub save_path: String,
    pub status: String,
    pub progress: f64,
    pub size: u64,
    pub content_path: Option<String>,
    pub files_json: Option<String>,
    pub error_msg: Option<String>,
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open SQLite DB at {path}"))?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS downloads (
                hash         TEXT PRIMARY KEY,
                task_id      TEXT,
                file_id      TEXT,
                url          TEXT NOT NULL,
                save_path    TEXT NOT NULL,
                status       TEXT NOT NULL DEFAULT 'downloading',
                progress     REAL NOT NULL DEFAULT 0.0,
                size         INTEGER NOT NULL DEFAULT 0,
                content_path TEXT,
                files_json   TEXT,
                error_msg    TEXT,
                created_at   TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_status ON downloads(status);
            CREATE INDEX IF NOT EXISTS idx_task_id ON downloads(task_id);",
        )
        .context("DB migration failed")?;
        Ok(())
    }

    pub fn insert(&self, rec: &DownloadRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO downloads
             (hash, task_id, file_id, url, save_path, status, progress, size, content_path, files_json, error_msg)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                rec.hash,
                rec.task_id,
                rec.file_id,
                rec.url,
                rec.save_path,
                rec.status,
                rec.progress,
                rec.size as i64,
                rec.content_path,
                rec.files_json,
                rec.error_msg
            ],
        )
        .context("DB insert failed")?;
        Ok(())
    }

    pub fn get(&self, hash: &str) -> Result<Option<DownloadRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT hash, task_id, file_id, url, save_path, status, progress, size,
                    content_path, files_json, error_msg
             FROM downloads WHERE hash = ?1",
        )?;
        let result = stmt.query_row(params![hash], row_to_record);
        match result {
            Ok(rec) => Ok(Some(rec)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_many(&self, hashes: &[String]) -> Result<Vec<DownloadRecord>> {
        if hashes.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn.lock().unwrap();
        let placeholders: Vec<String> = (1..=hashes.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT hash, task_id, file_id, url, save_path, status, progress, size,
                    content_path, files_json, error_msg
             FROM downloads WHERE hash IN ({})",
            placeholders.join(",")
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            hashes.iter().map(|h| h as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), row_to_record)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_by_status(&self, status: &str) -> Result<Vec<DownloadRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT hash, task_id, file_id, url, save_path, status, progress, size,
                    content_path, files_json, error_msg
             FROM downloads WHERE status = ?1",
        )?;
        let rows = stmt.query_map(params![status], row_to_record)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_status(&self, hash: &str, status: &str, progress: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET status = ?1, progress = ?2, updated_at = datetime('now')
             WHERE hash = ?3",
            params![status, progress, hash],
        )?;
        Ok(())
    }

    pub fn update_task_id(&self, hash: &str, task_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET task_id = ?1, updated_at = datetime('now') WHERE hash = ?2",
            params![task_id, hash],
        )?;
        Ok(())
    }

    pub fn update_completed(
        &self,
        hash: &str,
        file_id: &str,
        content_path: &str,
        files_json: &str,
        size: u64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET file_id = ?1, status = 'completed', progress = 1.0,
                    content_path = ?2, files_json = ?3, size = ?4, updated_at = datetime('now')
             WHERE hash = ?5",
            params![file_id, content_path, files_json, size as i64, hash],
        )?;
        Ok(())
    }

    pub fn update_error(&self, hash: &str, error_msg: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET status = 'failed', error_msg = ?1, updated_at = datetime('now')
             WHERE hash = ?2",
            params![error_msg, hash],
        )?;
        Ok(())
    }

    pub fn delete(&self, hash: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM downloads WHERE hash = ?1", params![hash])?;
        Ok(())
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<DownloadRecord> {
    Ok(DownloadRecord {
        hash: row.get(0)?,
        task_id: row.get(1)?,
        file_id: row.get(2)?,
        url: row.get(3)?,
        save_path: row.get(4)?,
        status: row.get(5)?,
        progress: row.get(6)?,
        size: row.get::<_, i64>(7)? as u64,
        content_path: row.get(8)?,
        files_json: row.get(9)?,
        error_msg: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Db {
        Db::open(":memory:").unwrap()
    }

    fn sample_record(hash: &str) -> DownloadRecord {
        DownloadRecord {
            hash: hash.to_string(),
            task_id: None,
            file_id: None,
            url: "magnet:?xt=urn:btih:test".to_string(),
            save_path: "/downloads".to_string(),
            status: "downloading".to_string(),
            progress: 0.0,
            size: 0,
            content_path: None,
            files_json: None,
            error_msg: None,
        }
    }

    #[test]
    fn test_insert_and_get() {
        let db = test_db();
        let rec = sample_record("abc123");
        db.insert(&rec).unwrap();
        let got = db.get("abc123").unwrap().unwrap();
        assert_eq!(got.hash, "abc123");
        assert_eq!(got.status, "downloading");
    }

    #[test]
    fn test_get_missing_returns_none() {
        let db = test_db();
        assert!(db.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_update_status() {
        let db = test_db();
        db.insert(&sample_record("h1")).unwrap();
        db.update_status("h1", "completed", 1.0).unwrap();
        let got = db.get("h1").unwrap().unwrap();
        assert_eq!(got.status, "completed");
        assert_eq!(got.progress, 1.0);
    }

    #[test]
    fn test_update_completed() {
        let db = test_db();
        db.insert(&sample_record("h2")).unwrap();
        db.update_completed(
            "h2",
            "file_abc",
            "/downloads/anime.mkv",
            r#"["/downloads/anime.mkv"]"#,
            1024,
        )
        .unwrap();
        let got = db.get("h2").unwrap().unwrap();
        assert_eq!(got.status, "completed");
        assert_eq!(got.content_path.as_deref(), Some("/downloads/anime.mkv"));
        assert_eq!(got.size, 1024);
    }

    #[test]
    fn test_list_by_status() {
        let db = test_db();
        db.insert(&sample_record("h3")).unwrap();
        db.insert(&sample_record("h4")).unwrap();
        db.update_status("h4", "failed", 0.0).unwrap();
        let downloading = db.list_by_status("downloading").unwrap();
        assert_eq!(downloading.len(), 1);
        assert_eq!(downloading[0].hash, "h3");
    }

    #[test]
    fn test_get_many() {
        let db = test_db();
        db.insert(&sample_record("x1")).unwrap();
        db.insert(&sample_record("x2")).unwrap();
        let results = db
            .get_many(&["x1".to_string(), "x2".to_string(), "x3".to_string()])
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_delete() {
        let db = test_db();
        db.insert(&sample_record("del1")).unwrap();
        db.delete("del1").unwrap();
        assert!(db.get("del1").unwrap().is_none());
    }

    #[test]
    fn test_update_error() {
        let db = test_db();
        db.insert(&sample_record("err1")).unwrap();
        db.update_error("err1", "network timeout").unwrap();
        let got = db.get("err1").unwrap().unwrap();
        assert_eq!(got.status, "failed");
        assert_eq!(got.error_msg.as_deref(), Some("network timeout"));
    }
}
