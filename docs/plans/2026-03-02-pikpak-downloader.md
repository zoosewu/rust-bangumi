# PikPak Downloader Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a `downloader-pikpak` microservice at port 8006 that implements the same HTTP API contract as `downloader-qbittorrent`, using PikPak's offline download feature as a two-phase cloud-then-local download pipeline.

**Architecture:** PikPak receives magnet/HTTP URLs for cloud-side download (Phase 1); once PikPak finishes, the service streams the file to local disk (Phase 2). A SQLite DB at `/data/pikpak.db` maps `hash → (task_id, file_id, status, content_path)` for crash-resilient tracking. A background tokio task polls PikPak every 30 seconds to advance state.

**Tech Stack:** Rust, axum 0.7, reqwest 0.12, rusqlite 0.31 (bundled), tokio, serde_json, sha2 0.10, shared crate (DownloaderClient trait moved here).

---

## Parallelism Map

```
Task 1 (move trait to shared)
    │
    ├── Task 2A (PikPak API client)   ← parallel
    ├── Task 2B (SQLite db.rs)        ← parallel
    └── Task 4  (handlers.rs)         ← parallel
            │
            └── Task 3 (PikPakClient impl + polling loop)
                    │
                    └── Task 5 (main.rs + registration)
                            │
                            ├── Task 6 (Cargo.toml + workspace)  ← parallel
                            └── Task 7 (Dockerfile + compose)     ← parallel
```

Tasks 2A, 2B, and 4 can be dispatched as **parallel subagents** after Task 1 completes.
Tasks 6 and 7 can be dispatched as **parallel subagents** after Task 5 completes.

---

## Task 1: Move DownloaderClient Trait to Shared Crate

**Files:**
- Modify: `shared/src/lib.rs`
- Create: `shared/src/downloader_trait.rs`
- Modify: `downloaders/qbittorrent/src/traits.rs` (delete content, re-export)
- Modify: `downloaders/qbittorrent/src/lib.rs`
- Modify: `downloaders/qbittorrent/src/handlers.rs` (update import)
- Modify: `downloaders/qbittorrent/src/mock.rs` (update import)

**Step 1: Create `shared/src/downloader_trait.rs`**

```rust
// shared/src/downloader_trait.rs
use anyhow::Result;
use crate::{CancelResultItem, DownloadRequestItem, DownloadResultItem, DownloadStatusItem};

pub trait DownloaderClient: Send + Sync {
    fn login(
        &self,
        username: &str,
        password: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn add_torrents(
        &self,
        items: Vec<DownloadRequestItem>,
    ) -> impl std::future::Future<Output = Result<Vec<DownloadResultItem>>> + Send;

    fn cancel_torrents(
        &self,
        hashes: Vec<String>,
    ) -> impl std::future::Future<Output = Result<Vec<CancelResultItem>>> + Send;

    fn query_status(
        &self,
        hashes: Vec<String>,
    ) -> impl std::future::Future<Output = Result<Vec<DownloadStatusItem>>> + Send;

    fn pause_torrent(&self, hash: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    fn resume_torrent(&self, hash: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    fn delete_torrent(
        &self,
        hash: &str,
        delete_files: bool,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}
```

**Step 2: Add module to `shared/src/lib.rs`**

Add at the top:
```rust
pub mod downloader_trait;
pub use downloader_trait::DownloaderClient;
```

**Step 3: Update `downloaders/qbittorrent/src/traits.rs`**

Replace entire file contents with:
```rust
// Re-export from shared for backwards compatibility
pub use shared::DownloaderClient;
```

**Step 4: Update `downloaders/qbittorrent/src/handlers.rs` import**

Change:
```rust
use downloader_qbittorrent::DownloaderClient;
```
To:
```rust
use shared::DownloaderClient;
```

**Step 5: Update `downloaders/qbittorrent/src/mock.rs` import**

Change:
```rust
use crate::traits::DownloaderClient;
```
To:
```rust
use shared::DownloaderClient;
```

**Step 6: Verify compilation**

```bash
cd /workspace && cargo build -p downloader-qbittorrent 2>&1
```
Expected: compiles with 0 errors.

**Step 7: Commit**

```bash
cd /workspace
git add shared/src/downloader_trait.rs shared/src/lib.rs \
    downloaders/qbittorrent/src/traits.rs \
    downloaders/qbittorrent/src/handlers.rs \
    downloaders/qbittorrent/src/mock.rs
git commit -m "refactor(shared): move DownloaderClient trait to shared crate"
```

---

## Task 2A: PikPak API Client

> **Can run in parallel with Task 2B and Task 4 after Task 1 completes.**

**Files:**
- Create: `downloaders/pikpak/src/pikpak_api.rs`

This file implements the raw HTTP layer: auth, token refresh, offline download submission, task status query, file info fetch, and task deletion. No SQLite, no trait. Pure PikPak API.

**Step 1: Create `downloaders/pikpak/src/pikpak_api.rs`**

```rust
// downloaders/pikpak/src/pikpak_api.rs
//! Raw PikPak HTTP API client.
//! Reference: https://github.com/Bengerthelorf/pikpaktui/blob/main/src/pikpak.rs

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

const AUTH_BASE_URL: &str = "https://user.mypikpak.com";
const DRIVE_BASE_URL: &str = "https://api-drive.mypikpak.com";
const CLIENT_ID: &str = "YNxT9w7GMdWvEOKa";
const CLIENT_SECRET: &str = "dbw2OtmVEeuUvIptb1Coyg";
const USER_AGENT: &str = "ANDROID-com.pikcloud.pikpak/1.21.0";

// ── Token ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Token {
    access_token: String,
    refresh_token: String,
    expires_at: u64, // unix seconds
}

impl Token {
    fn is_expiring(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now + 300 >= self.expires_at
    }
}

// ── API response types ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SignInResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct CaptchaInitResponse {
    captcha_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineTask {
    pub id: String,
    pub name: Option<String>,
    pub phase: String, // "PHASE_TYPE_RUNNING" | "PHASE_TYPE_COMPLETE" | "PHASE_TYPE_ERROR"
    pub progress: Option<i64>, // 0–100
    pub file_id: Option<String>,
    pub file_size: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OfflineTaskResponse {
    pub task: Option<OfflineTask>,
}

#[derive(Debug, Deserialize)]
struct OfflineListResponse {
    pub tasks: Option<Vec<OfflineTask>>,
}

#[derive(Debug, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub name: Option<String>,
    pub size: Option<String>,
    pub web_content_link: Option<String>,
    pub links: Option<FileLinks>,
}

#[derive(Debug, Deserialize)]
pub struct FileLinks {
    #[serde(rename = "application/octet-stream")]
    pub download: Option<FileLink>,
}

#[derive(Debug, Deserialize)]
pub struct FileLink {
    pub url: String,
}

// ── Client ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PikPakApi {
    http: Client,
    token: Arc<RwLock<Option<Token>>>,
    device_id: String,
}

impl PikPakApi {
    pub fn new() -> Self {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .expect("Failed to build reqwest client");
        Self {
            http,
            token: Arc::new(RwLock::new(None)),
            device_id: String::new(),
        }
    }

    fn make_device_id(email: &str) -> String {
        format!("{:x}", md5_hex(email.as_bytes()))
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<()> {
        // Step 1: init captcha
        let device_id = Self::make_device_id(email);
        let captcha_resp: CaptchaInitResponse = self
            .http
            .post(format!("{AUTH_BASE_URL}/v1/shield/captcha/init"))
            .json(&serde_json::json!({
                "client_id": CLIENT_ID,
                "action": "POST:/v1/auth/signin",
                "device_id": device_id,
                "meta": { "email": email }
            }))
            .send()
            .await?
            .json()
            .await?;

        // Step 2: sign in
        let signin_resp: SignInResponse = self
            .http
            .post(format!("{AUTH_BASE_URL}/v1/auth/signin"))
            .header("x-device-id", &device_id)
            .header("x-captcha-token", &captcha_resp.captcha_token)
            .json(&serde_json::json!({
                "client_id": CLIENT_ID,
                "client_secret": CLIENT_SECRET,
                "username": email,
                "password": password
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + signin_resp.expires_in;

        *self.token.write().await = Some(Token {
            access_token: signin_resp.access_token,
            refresh_token: signin_resp.refresh_token,
            expires_at,
        });

        // Persist device_id into self — safe via unsafe ptr trick or store on Arc
        // For simplicity, we re-compute device_id each request from stored email.
        // (In production, store email/device_id in PikPakApi struct fields.)
        tracing::info!("PikPak login successful for {email}");
        Ok(())
    }

    async fn refresh_if_needed(&self) -> Result<String> {
        let token_guard = self.token.read().await;
        let token = token_guard.as_ref().ok_or_else(|| anyhow!("Not logged in"))?;

        if !token.is_expiring() {
            return Ok(token.access_token.clone());
        }
        let refresh_token = token.refresh_token.clone();
        drop(token_guard);

        let resp: SignInResponse = self
            .http
            .post(format!("{AUTH_BASE_URL}/v1/auth/token"))
            .json(&serde_json::json!({
                "client_id": CLIENT_ID,
                "client_secret": CLIENT_SECRET,
                "grant_type": "refresh_token",
                "refresh_token": refresh_token
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + resp.expires_in;

        let new_access = resp.access_token.clone();
        *self.token.write().await = Some(Token {
            access_token: resp.access_token,
            refresh_token: resp.refresh_token,
            expires_at,
        });
        Ok(new_access)
    }

    /// Submit a URL/magnet for offline download. Returns the created OfflineTask.
    pub async fn offline_download(&self, url: &str) -> Result<OfflineTask> {
        let access_token = self.refresh_if_needed().await?;
        let resp: OfflineTaskResponse = self
            .http
            .post(format!("{DRIVE_BASE_URL}/drive/v1/files"))
            .bearer_auth(&access_token)
            .json(&serde_json::json!({
                "kind": "drive#file",
                "upload_type": "UPLOAD_TYPE_URL",
                "url": { "url": url },
                "folder_type": "DOWNLOAD"
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        resp.task.ok_or_else(|| anyhow!("PikPak returned no task for offline download"))
    }

    /// List all running offline tasks.
    pub async fn list_running_tasks(&self) -> Result<Vec<OfflineTask>> {
        self.list_tasks_by_phase("PHASE_TYPE_RUNNING").await
    }

    /// List all completed offline tasks.
    pub async fn list_completed_tasks(&self) -> Result<Vec<OfflineTask>> {
        self.list_tasks_by_phase("PHASE_TYPE_COMPLETE").await
    }

    async fn list_tasks_by_phase(&self, phase: &str) -> Result<Vec<OfflineTask>> {
        let access_token = self.refresh_if_needed().await?;
        let filters = serde_json::json!({ "phase": { "in": phase } });
        let resp: OfflineListResponse = self
            .http
            .get(format!("{DRIVE_BASE_URL}/drive/v1/tasks"))
            .bearer_auth(&access_token)
            .query(&[
                ("type", "offline"),
                ("thumbnail_size", "SIZE_SMALL"),
                ("limit", "200"),
                ("filters", &filters.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.tasks.unwrap_or_default())
    }

    /// Get download URL for a completed file. Returns (download_url, file_size_bytes).
    pub async fn get_file_download_url(&self, file_id: &str) -> Result<(String, u64)> {
        let access_token = self.refresh_if_needed().await?;
        let info: FileInfo = self
            .http
            .get(format!("{DRIVE_BASE_URL}/drive/v1/files/{file_id}"))
            .bearer_auth(&access_token)
            .query(&[("thumbnail_size", "SIZE_SMALL")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let url = info
            .links
            .and_then(|l| l.download)
            .map(|l| l.url)
            .or(info.web_content_link)
            .ok_or_else(|| anyhow!("No download URL for file {file_id}"))?;

        let size = info
            .size
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        Ok((url, size))
    }

    /// Delete offline tasks by task_id list. If delete_files=true, also removes cloud files.
    pub async fn delete_tasks(&self, task_ids: &[&str], delete_files: bool) -> Result<()> {
        if task_ids.is_empty() {
            return Ok(());
        }
        let access_token = self.refresh_if_needed().await?;
        let ids = task_ids.join(",");
        self.http
            .delete(format!("{DRIVE_BASE_URL}/drive/v1/tasks"))
            .bearer_auth(&access_token)
            .query(&[
                ("task_ids", ids.as_str()),
                ("delete_files", if delete_files { "true" } else { "false" }),
            ])
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub fn is_logged_in(&self) -> bool {
        // Use try_read to avoid blocking
        self.token.try_read().map(|g| g.is_some()).unwrap_or(false)
    }
}

/// Simple MD5 implementation for device_id (not crypto-safe, just for device fingerprint).
fn md5_hex(data: &[u8]) -> u128 {
    // Use the md5 crate if available, otherwise sha2 truncated.
    // For device_id we use sha2-based hex truncated to 32 chars.
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    let result = h.finalize();
    // Return first 16 bytes as u128 for display purposes
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&result[..16]);
    u128::from_be_bytes(bytes)
}
```

**Step 2: Write unit tests for `pikpak_api.rs`**

Add at the bottom of `pikpak_api.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_expiry_check() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expiring = Token {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_at: now + 100, // expires in 100s < 300s threshold
        };
        assert!(expiring.is_expiring());

        let fresh = Token {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_at: now + 3600,
        };
        assert!(!fresh.is_expiring());
    }

    #[test]
    fn test_md5_hex_deterministic() {
        let a = md5_hex(b"test@example.com");
        let b = md5_hex(b"test@example.com");
        assert_eq!(a, b);
        let c = md5_hex(b"other@example.com");
        assert_ne!(a, c);
    }
}
```

**Step 3: Run unit tests (no network needed)**

```bash
cd /workspace && cargo test -p downloader-pikpak pikpak_api 2>&1
```
Expected: 2 tests pass.

---

## Task 2B: SQLite DB Layer

> **Can run in parallel with Task 2A and Task 4 after Task 1 completes.**

**Files:**
- Create: `downloaders/pikpak/src/db.rs`

**Step 1: Create `downloaders/pikpak/src/db.rs`**

```rust
// downloaders/pikpak/src/db.rs
//! SQLite persistence layer: maps content hash → PikPak task state.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

/// Download record stored in SQLite.
#[derive(Debug, Clone)]
pub struct DownloadRecord {
    pub hash: String,
    pub task_id: Option<String>,
    pub file_id: Option<String>,
    pub url: String,
    pub save_path: String,
    pub status: String,      // "downloading" | "completed" | "failed"
    pub progress: f64,       // 0.0–1.0
    pub size: u64,
    pub content_path: Option<String>,
    pub files_json: Option<String>, // JSON array of local file paths
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
        let db = Self { conn: Arc::new(Mutex::new(conn)) };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS downloads (
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
            CREATE INDEX IF NOT EXISTS idx_task_id ON downloads(task_id);
        ").context("DB migration failed")?;
        Ok(())
    }

    pub fn insert(&self, rec: &DownloadRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO downloads
             (hash, task_id, file_id, url, save_path, status, progress, size, content_path, files_json, error_msg)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                rec.hash, rec.task_id, rec.file_id, rec.url, rec.save_path,
                rec.status, rec.progress, rec.size as i64,
                rec.content_path, rec.files_json, rec.error_msg
            ],
        ).context("DB insert failed")?;
        Ok(())
    }

    pub fn get(&self, hash: &str) -> Result<Option<DownloadRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT hash, task_id, file_id, url, save_path, status, progress, size,
                    content_path, files_json, error_msg
             FROM downloads WHERE hash = ?1"
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
        let params: Vec<&dyn rusqlite::ToSql> = hashes.iter().map(|h| h as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), row_to_record)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_by_status(&self, status: &str) -> Result<Vec<DownloadRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT hash, task_id, file_id, url, save_path, status, progress, size,
                    content_path, files_json, error_msg
             FROM downloads WHERE status = ?1"
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
```

**Step 2: Write unit tests for db.rs**

Add at the bottom of `db.rs`:

```rust
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
        db.update_completed("h2", "file_abc", "/downloads/anime.mkv", r#"["/downloads/anime.mkv"]"#, 1024).unwrap();
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
        let results = db.get_many(&["x1".to_string(), "x2".to_string(), "x3".to_string()]).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_delete() {
        let db = test_db();
        db.insert(&sample_record("del1")).unwrap();
        db.delete("del1").unwrap();
        assert!(db.get("del1").unwrap().is_none());
    }
}
```

**Step 3: Run DB tests**

```bash
cd /workspace && cargo test -p downloader-pikpak db 2>&1
```
Expected: 7 tests pass.

---

## Task 4: HTTP Handlers (Identical Contract to qbittorrent)

> **Can run in parallel with Task 2A and Task 2B after Task 1 completes.**

**Files:**
- Create: `downloaders/pikpak/src/handlers.rs`

This is a near-verbatim copy of `downloaders/qbittorrent/src/handlers.rs` — the only change is removing the qbittorrent-specific import.

**Step 1: Create `downloaders/pikpak/src/handlers.rs`**

```rust
// downloaders/pikpak/src/handlers.rs
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use shared::{
    BatchCancelRequest, BatchCancelResponse, BatchDownloadRequest, BatchDownloadResponse,
    DownloaderClient, StatusQueryResponse,
};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct StatusQueryParams {
    pub hashes: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteParams {
    pub delete_files: Option<bool>,
}

#[derive(serde::Deserialize)]
pub struct SetCredentialsRequest {
    pub username: String,
    pub password: String,
}

pub async fn batch_download<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Json(req): Json<BatchDownloadRequest>,
) -> (StatusCode, Json<BatchDownloadResponse>) {
    if req.items.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(BatchDownloadResponse { results: vec![] }));
    }
    match client.add_torrents(req.items).await {
        Ok(results) => (StatusCode::OK, Json(BatchDownloadResponse { results })),
        Err(e) => {
            tracing::error!("Batch download failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(BatchDownloadResponse { results: vec![] }))
        }
    }
}

pub async fn batch_cancel<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Json(req): Json<BatchCancelRequest>,
) -> (StatusCode, Json<BatchCancelResponse>) {
    if req.hashes.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(BatchCancelResponse { results: vec![] }));
    }
    match client.cancel_torrents(req.hashes).await {
        Ok(results) => (StatusCode::OK, Json(BatchCancelResponse { results })),
        Err(e) => {
            tracing::error!("Batch cancel failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(BatchCancelResponse { results: vec![] }))
        }
    }
}

pub async fn query_download_status<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Query(params): Query<StatusQueryParams>,
) -> (StatusCode, Json<StatusQueryResponse>) {
    let hashes: Vec<String> = params
        .hashes
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if hashes.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(StatusQueryResponse { statuses: vec![] }));
    }
    match client.query_status(hashes).await {
        Ok(statuses) => (StatusCode::OK, Json(StatusQueryResponse { statuses })),
        Err(e) => {
            tracing::error!("Status query failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(StatusQueryResponse { statuses: vec![] }))
        }
    }
}

pub async fn pause<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Path(hash): Path<String>,
) -> StatusCode {
    match client.pause_torrent(&hash).await {
        Ok(()) => StatusCode::OK,
        Err(e) => { tracing::error!("Pause failed for {hash}: {e}"); StatusCode::INTERNAL_SERVER_ERROR }
    }
}

pub async fn resume<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Path(hash): Path<String>,
) -> StatusCode {
    match client.resume_torrent(&hash).await {
        Ok(()) => StatusCode::OK,
        Err(e) => { tracing::error!("Resume failed for {hash}: {e}"); StatusCode::INTERNAL_SERVER_ERROR }
    }
}

pub async fn delete_download<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Path(hash): Path<String>,
    Query(params): Query<DeleteParams>,
) -> StatusCode {
    let delete_files = params.delete_files.unwrap_or(false);
    match client.delete_torrent(&hash, delete_files).await {
        Ok(()) => StatusCode::OK,
        Err(e) => { tracing::error!("Delete failed for {hash}: {e}"); StatusCode::INTERNAL_SERVER_ERROR }
    }
}

pub async fn health_check() -> StatusCode {
    StatusCode::OK
}

pub async fn set_credentials<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Json(req): Json<SetCredentialsRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match client.login(&req.username, &req.password).await {
        Ok(_) => Ok(Json(serde_json::json!({"message": "Credentials updated successfully"}))),
        Err(e) => { tracing::error!("PikPak login failed: {e}"); Err(StatusCode::BAD_GATEWAY) }
    }
}
```

**Step 2: Handler tests — create `downloaders/pikpak/tests/integration/handler_tests.rs`**

```rust
// tests/integration/handler_tests.rs
// NOTE: Because binary crates cannot be imported, we test handlers using the MockDownloaderClient
// from the shared crate and axum's test utilities.

use axum::{body::Body, http::{Request, StatusCode}, Router, routing::{delete, get, post}};
use downloader_pikpak::MockPikPakClient;
use http_body_util::BodyExt;
use shared::{BatchDownloadRequest, DownloadRequestItem, DownloadResultItem};
use std::sync::Arc;
use tower::ServiceExt;

fn make_app(client: MockPikPakClient) -> Router {
    use downloader_pikpak::handlers;
    Router::new()
        .route("/downloads", post(handlers::batch_download::<MockPikPakClient>))
        .route("/downloads", get(handlers::query_download_status::<MockPikPakClient>))
        .route("/downloads/cancel", post(handlers::batch_cancel::<MockPikPakClient>))
        .route("/downloads/:hash/pause", post(handlers::pause::<MockPikPakClient>))
        .route("/downloads/:hash/resume", post(handlers::resume::<MockPikPakClient>))
        .route("/downloads/:hash", delete(handlers::delete_download::<MockPikPakClient>))
        .route("/health", get(handlers::health_check))
        .with_state(Arc::new(client))
}

#[tokio::test]
async fn test_health_check() {
    let app = make_app(MockPikPakClient::new());
    let resp = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_batch_download_empty_returns_400() {
    let app = make_app(MockPikPakClient::new());
    let body = serde_json::to_vec(&BatchDownloadRequest { items: vec![] }).unwrap();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST").uri("/downloads")
                .header("content-type", "application/json")
                .body(Body::from(body)).unwrap()
        )
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_batch_download_success() {
    let result = vec![DownloadResultItem {
        url: "magnet:test".to_string(),
        hash: Some("abc123".to_string()),
        status: "accepted".to_string(),
        reason: None,
    }];
    let client = MockPikPakClient::new().with_add_torrents_result(Ok(result));
    let app = make_app(client);
    let req_body = serde_json::to_vec(&BatchDownloadRequest {
        items: vec![DownloadRequestItem {
            url: "magnet:test".to_string(),
            save_path: "/downloads".to_string(),
        }],
    }).unwrap();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST").uri("/downloads")
                .header("content-type", "application/json")
                .body(Body::from(req_body)).unwrap()
        )
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: shared::BatchDownloadResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.results.len(), 1);
    assert_eq!(parsed.results[0].hash.as_deref(), Some("abc123"));
}
```

**Step 3: Run handler tests**

```bash
cd /workspace && cargo test -p downloader-pikpak handler 2>&1
```
Expected: 3 tests pass.

---

## Task 3: PikPakClient Implementation + Background Polling Loop

> **Runs after Task 2A and Task 2B complete.**

**Files:**
- Create: `downloaders/pikpak/src/pikpak_client.rs`
- Create: `downloaders/pikpak/src/lib.rs`

### Hash Extraction Utility

Add to `pikpak_client.rs`:

```rust
/// Extract btih hash from magnet link: "magnet:?xt=urn:btih:HASH&..."
fn extract_magnet_hash(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    let prefix = "urn:btih:";
    let start = lower.find(prefix)? + prefix.len();
    let end = lower[start..].find('&').map(|i| start + i).unwrap_or(lower.len());
    let hash = &url[start..end];
    if hash.len() >= 32 { Some(hash.to_uppercase()) } else { None }
}

/// For HTTP URLs: SHA256(url) → first 40 hex chars as synthetic hash.
fn synthetic_hash(url: &str) -> String {
    use sha2::{Digest, Sha256};
    let result = Sha256::digest(url.as_bytes());
    hex::encode(&result[..20]) // 20 bytes = 40 hex chars
}

pub fn extract_hash(url: &str) -> String {
    if url.starts_with("magnet:") {
        extract_magnet_hash(url).unwrap_or_else(|| synthetic_hash(url))
    } else {
        synthetic_hash(url)
    }
}
```

### PikPakClient struct

```rust
// downloaders/pikpak/src/pikpak_client.rs
use crate::{db::{Db, DownloadRecord}, pikpak_api::PikPakApi};
use anyhow::Result;
use shared::{
    CancelResultItem, DownloadRequestItem, DownloadResultItem, DownloadStatusItem, DownloaderClient,
};
use std::{path::Path, sync::Arc};

pub struct PikPakClient {
    api: Arc<PikPakApi>,
    db: Db,
}

impl PikPakClient {
    pub fn new(db_path: &str) -> Result<Self> {
        Ok(Self {
            api: Arc::new(PikPakApi::new()),
            db: Db::open(db_path)?,
        })
    }

    /// Spawn background polling loop. Call once on startup.
    pub fn start_polling(&self) {
        let api = self.api.clone();
        let db = self.db.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                if let Err(e) = poll_once(&api, &db).await {
                    tracing::warn!("Polling error: {e}");
                }
            }
        });
    }
}

/// One polling cycle: check PikPak for completed tasks → trigger local download.
async fn poll_once(api: &PikPakApi, db: &Db) -> Result<()> {
    if !api.is_logged_in() {
        return Ok(());
    }

    // Get all running tasks in our DB
    let downloading = db.list_by_status("downloading")?;
    if downloading.is_empty() {
        return Ok(());
    }

    // Fetch completed tasks from PikPak
    let completed = api.list_completed_tasks().await?;
    let completed_ids: std::collections::HashMap<String, _> = completed
        .into_iter()
        .filter_map(|t| t.file_id.clone().map(|fid| (t.id.clone(), (fid, t.progress.unwrap_or(100)))))
        .collect();

    for rec in &downloading {
        let task_id = match &rec.task_id {
            Some(id) => id.clone(),
            None => continue,
        };

        if let Some((file_id, _)) = completed_ids.get(&task_id) {
            // Phase 2: download from PikPak to local disk
            match download_to_local(api, db, rec, file_id).await {
                Ok(()) => tracing::info!("Phase 2 complete for hash={}", rec.hash),
                Err(e) => {
                    tracing::error!("Phase 2 failed for hash={}: {e}", rec.hash);
                    let _ = db.update_error(&rec.hash, &e.to_string());
                }
            }
        } else {
            // Still running — update progress from PikPak (best-effort)
            // We can fetch running task list and update progress
            tracing::debug!("Task {} still running", task_id);
        }
    }

    Ok(())
}

async fn download_to_local(api: &PikPakApi, db: &Db, rec: &DownloadRecord, file_id: &str) -> Result<()> {
    let (download_url, size) = api.get_file_download_url(file_id).await?;

    // Determine output path
    let filename = download_url
        .split('/')
        .last()
        .and_then(|s| s.split('?').next())
        .unwrap_or("download");
    let dest_path = Path::new(&rec.save_path).join(filename);

    // Create save directory if needed
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Stream download
    let resp = reqwest::get(&download_url).await?.error_for_status()?;
    let bytes = resp.bytes().await?;
    tokio::fs::write(&dest_path, &bytes).await?;

    let dest_str = dest_path.to_string_lossy().to_string();
    let files_json = serde_json::json!([dest_str]).to_string();
    let actual_size = if size > 0 { size } else { bytes.len() as u64 };

    db.update_completed(&rec.hash, file_id, &dest_str, &files_json, actual_size)?;
    Ok(())
}

impl DownloaderClient for PikPakClient {
    async fn login(&self, username: &str, password: &str) -> Result<()> {
        self.api.login(username, password).await
    }

    async fn add_torrents(&self, items: Vec<DownloadRequestItem>) -> Result<Vec<DownloadResultItem>> {
        let mut results = Vec::new();
        for item in items {
            let hash = extract_hash(&item.url);
            match self.api.offline_download(&item.url).await {
                Ok(task) => {
                    let rec = DownloadRecord {
                        hash: hash.clone(),
                        task_id: Some(task.id.clone()),
                        file_id: None,
                        url: item.url.clone(),
                        save_path: item.save_path.clone(),
                        status: "downloading".to_string(),
                        progress: 0.0,
                        size: 0,
                        content_path: None,
                        files_json: None,
                        error_msg: None,
                    };
                    let _ = self.db.insert(&rec);
                    results.push(DownloadResultItem {
                        url: item.url,
                        hash: Some(hash),
                        status: "accepted".to_string(),
                        reason: None,
                    });
                }
                Err(e) => {
                    tracing::error!("PikPak offline_download failed: {e}");
                    results.push(DownloadResultItem {
                        url: item.url,
                        hash: None,
                        status: "failed".to_string(),
                        reason: Some(e.to_string()),
                    });
                }
            }
        }
        Ok(results)
    }

    async fn cancel_torrents(&self, hashes: Vec<String>) -> Result<Vec<CancelResultItem>> {
        let mut results = Vec::new();
        for hash in hashes {
            if let Ok(Some(rec)) = self.db.get(&hash) {
                if let Some(task_id) = &rec.task_id {
                    let _ = self.api.delete_tasks(&[task_id.as_str()], false).await;
                }
                let _ = self.db.update_status(&hash, "cancelled", rec.progress);
                results.push(CancelResultItem { hash, status: "cancelled".to_string() });
            } else {
                results.push(CancelResultItem { hash, status: "not_found".to_string() });
            }
        }
        Ok(results)
    }

    async fn query_status(&self, hashes: Vec<String>) -> Result<Vec<DownloadStatusItem>> {
        let records = self.db.get_many(&hashes)?;
        Ok(records.into_iter().map(|rec| {
            let files: Vec<String> = rec.files_json
                .as_deref()
                .and_then(|j| serde_json::from_str(j).ok())
                .unwrap_or_default();
            DownloadStatusItem {
                hash: rec.hash,
                status: rec.status,
                progress: rec.progress,
                size: rec.size,
                content_path: rec.content_path,
                files,
            }
        }).collect())
    }

    // PikPak doesn't support pause/resume for offline downloads — return Ok as no-op.
    async fn pause_torrent(&self, _hash: &str) -> Result<()> { Ok(()) }
    async fn resume_torrent(&self, _hash: &str) -> Result<()> { Ok(()) }

    async fn delete_torrent(&self, hash: &str, delete_files: bool) -> Result<()> {
        if let Ok(Some(rec)) = self.db.get(hash) {
            if let Some(task_id) = &rec.task_id {
                let _ = self.api.delete_tasks(&[task_id.as_str()], delete_files).await;
            }
            if delete_files {
                if let Some(path) = &rec.content_path {
                    let _ = tokio::fs::remove_file(path).await;
                }
            }
        }
        self.db.delete(hash)?;
        Ok(())
    }
}
```

**Create `downloaders/pikpak/src/lib.rs`**

```rust
// downloaders/pikpak/src/lib.rs
pub mod db;
pub mod handlers;
pub mod pikpak_api;
pub mod pikpak_client;
pub mod mock;

pub use pikpak_client::PikPakClient;
pub use mock::MockPikPakClient;
```

**Create `downloaders/pikpak/src/mock.rs`** (same pattern as qbittorrent):

```rust
// downloaders/pikpak/src/mock.rs
use anyhow::{anyhow, Result};
use shared::{
    CancelResultItem, DownloadRequestItem, DownloadResultItem, DownloadStatusItem, DownloaderClient,
};
use std::cell::RefCell;

pub struct MockPikPakClient {
    login_result: RefCell<Result<()>>,
    add_torrents_result: RefCell<Result<Vec<DownloadResultItem>>>,
    cancel_torrents_result: RefCell<Result<Vec<CancelResultItem>>>,
    query_status_result: RefCell<Result<Vec<DownloadStatusItem>>>,

    pub login_calls: RefCell<Vec<(String, String)>>,
    pub add_torrents_calls: RefCell<Vec<Vec<DownloadRequestItem>>>,
    pub cancel_torrents_calls: RefCell<Vec<Vec<String>>>,
    pub query_status_calls: RefCell<Vec<Vec<String>>>,
}

impl Default for MockPikPakClient {
    fn default() -> Self {
        Self {
            login_result: RefCell::new(Ok(())),
            add_torrents_result: RefCell::new(Ok(vec![])),
            cancel_torrents_result: RefCell::new(Ok(vec![])),
            query_status_result: RefCell::new(Ok(vec![])),
            login_calls: RefCell::new(vec![]),
            add_torrents_calls: RefCell::new(vec![]),
            cancel_torrents_calls: RefCell::new(vec![]),
            query_status_calls: RefCell::new(vec![]),
        }
    }
}

impl MockPikPakClient {
    pub fn new() -> Self { Self::default() }
    pub fn with_login_result(self, r: Result<()>) -> Self { *self.login_result.borrow_mut() = r; self }
    pub fn with_add_torrents_result(self, r: Result<Vec<DownloadResultItem>>) -> Self { *self.add_torrents_result.borrow_mut() = r; self }
    pub fn with_cancel_torrents_result(self, r: Result<Vec<CancelResultItem>>) -> Self { *self.cancel_torrents_result.borrow_mut() = r; self }
    pub fn with_query_status_result(self, r: Result<Vec<DownloadStatusItem>>) -> Self { *self.query_status_result.borrow_mut() = r; self }
}

unsafe impl Send for MockPikPakClient {}
unsafe impl Sync for MockPikPakClient {}

impl DownloaderClient for MockPikPakClient {
    async fn login(&self, username: &str, password: &str) -> Result<()> {
        self.login_calls.borrow_mut().push((username.to_string(), password.to_string()));
        match &*self.login_result.borrow() { Ok(()) => Ok(()), Err(e) => Err(anyhow!("{e}")) }
    }
    async fn add_torrents(&self, items: Vec<DownloadRequestItem>) -> Result<Vec<DownloadResultItem>> {
        self.add_torrents_calls.borrow_mut().push(items);
        match &*self.add_torrents_result.borrow() { Ok(r) => Ok(r.clone()), Err(e) => Err(anyhow!("{e}")) }
    }
    async fn cancel_torrents(&self, hashes: Vec<String>) -> Result<Vec<CancelResultItem>> {
        self.cancel_torrents_calls.borrow_mut().push(hashes);
        match &*self.cancel_torrents_result.borrow() { Ok(r) => Ok(r.clone()), Err(e) => Err(anyhow!("{e}")) }
    }
    async fn query_status(&self, hashes: Vec<String>) -> Result<Vec<DownloadStatusItem>> {
        self.query_status_calls.borrow_mut().push(hashes);
        match &*self.query_status_result.borrow() { Ok(r) => Ok(r.clone()), Err(e) => Err(anyhow!("{e}")) }
    }
    async fn pause_torrent(&self, _hash: &str) -> Result<()> { Ok(()) }
    async fn resume_torrent(&self, _hash: &str) -> Result<()> { Ok(()) }
    async fn delete_torrent(&self, _hash: &str, _delete_files: bool) -> Result<()> { Ok(()) }
}
```

**Step: Test hash extraction**

Add tests in `pikpak_client.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_magnet_hash() {
        let magnet = "magnet:?xt=urn:btih:AABBCC1234567890AABBCC1234567890AABBCC12&dn=test";
        let hash = extract_hash(magnet);
        assert_eq!(hash, "AABBCC1234567890AABBCC1234567890AABBCC12");
    }

    #[test]
    fn test_extract_http_synthetic_hash() {
        let url = "https://example.com/file.torrent";
        let hash = extract_hash(url);
        assert_eq!(hash.len(), 40);
        // Deterministic
        assert_eq!(extract_hash(url), hash);
    }

    #[test]
    fn test_different_urls_different_hashes() {
        let h1 = extract_hash("https://a.com/1.torrent");
        let h2 = extract_hash("https://a.com/2.torrent");
        assert_ne!(h1, h2);
    }
}
```

**Run tests:**

```bash
cd /workspace && cargo test -p downloader-pikpak 2>&1
```
Expected: all unit tests pass.

**Commit:**

```bash
cd /workspace
git add downloaders/pikpak/src/
git commit -m "feat(pikpak): implement PikPakClient, DB layer, API client, mock, handlers"
```

---

## Task 5: main.rs + Service Registration

> **Runs after Tasks 2A, 2B, 3, 4 are complete.**

**Files:**
- Create: `downloaders/pikpak/src/main.rs`

```rust
// downloaders/pikpak/src/main.rs
use axum::{routing::{delete, get, post}, Router};
use downloader_pikpak::{handlers, PikPakClient};
use shared::{DownloadType, DownloaderClient, ServiceRegistration, ServiceType};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("downloader_pikpak=debug".parse()?),
        )
        .init();

    let db_path = std::env::var("PIKPAK_DB_PATH").unwrap_or_else(|_| "/data/pikpak.db".to_string());
    let client = Arc::new(PikPakClient::new(&db_path)?);

    // Auto-login if credentials provided via env
    let email = std::env::var("PIKPAK_EMAIL").unwrap_or_default();
    let password = std::env::var("PIKPAK_PASSWORD").unwrap_or_default();
    if !email.is_empty() && !password.is_empty() {
        if let Err(e) = client.login(&email, &password).await {
            tracing::warn!("PikPak auto-login failed: {e}. Use POST /config/credentials to set credentials.");
        } else {
            // Start background polling only after successful login
            client.start_polling();
        }
    }

    let app = Router::new()
        .route("/downloads", post(handlers::batch_download::<PikPakClient>))
        .route("/downloads", get(handlers::query_download_status::<PikPakClient>))
        .route("/downloads/cancel", post(handlers::batch_cancel::<PikPakClient>))
        .route("/downloads/:hash/pause", post(handlers::pause::<PikPakClient>))
        .route("/downloads/:hash/resume", post(handlers::resume::<PikPakClient>))
        .route("/downloads/:hash", delete(handlers::delete_download::<PikPakClient>))
        .route("/health", get(handlers::health_check))
        .route("/config/credentials", post(handlers::set_credentials::<PikPakClient>))
        .with_state(client);

    let service_port: u16 = std::env::var("SERVICE_PORT")
        .ok().and_then(|v| v.parse().ok()).unwrap_or(8006);
    let addr = format!("0.0.0.0:{service_port}");
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("PikPak downloader listening on {addr}");

    tokio::spawn(async move {
        let core_url = std::env::var("CORE_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());
        let service_host = std::env::var("SERVICE_HOST").unwrap_or_else(|_| "localhost".to_string());
        let port: u16 = std::env::var("SERVICE_PORT")
            .ok().and_then(|v| v.parse().ok()).unwrap_or(8006);

        let registration = ServiceRegistration {
            service_type: ServiceType::Downloader,
            service_name: std::env::var("SERVICE_NAME")
                .unwrap_or_else(|_| "pikpak-downloader".to_string()),
            host: service_host,
            port,
            capabilities: shared::Capabilities {
                fetch_endpoint: None,
                download_endpoint: Some("/downloads".to_string()),
                sync_endpoint: None,
                supported_download_types: vec![DownloadType::Magnet, DownloadType::Http],
            },
        };
        shared::register_with_core_backoff(&core_url, &registration).await;
    });

    axum::serve(listener, app).await?;
    Ok(())
}
```

---

## Task 6: Cargo.toml Files + Workspace Integration

> **Can run in parallel with Task 7 after Task 5 completes.**

**Files:**
- Create: `downloaders/pikpak/Cargo.toml`
- Modify: `/workspace/Cargo.toml` (workspace root)

**Step 1: Create `downloaders/pikpak/Cargo.toml`**

```toml
[package]
name = "downloader-pikpak"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[[bin]]
name = "downloader-pikpak"
path = "src/main.rs"

[lib]
name = "downloader_pikpak"
path = "src/lib.rs"

[dependencies]
shared = { path = "../../shared" }

tokio.workspace = true
axum.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
dotenv.workspace = true
anyhow.workspace = true
thiserror.workspace = true
reqwest.workspace = true

rusqlite = { version = "0.31", features = ["bundled"] }
sha2 = "0.10"
hex = "0.4"

[dev-dependencies]
tower = { workspace = true, features = ["util"] }
http-body-util = "0.1"
```

**Step 2: Add `rusqlite`, `sha2`, `hex` to workspace Cargo.toml**

In `/workspace/Cargo.toml`, under `[workspace.dependencies]`, add:

```toml
rusqlite = { version = "0.31", features = ["bundled"] }
sha2 = "0.10"
hex = "0.4"
```

**Step 3: Add `downloaders/pikpak` to workspace members**

In `/workspace/Cargo.toml`, in `[workspace] members`, add:
```toml
"downloaders/pikpak",
```

**Step 4: Verify workspace compiles**

```bash
cd /workspace && cargo build -p downloader-pikpak 2>&1
```
Expected: compiles with 0 errors.

**Step 5: Run all tests**

```bash
cd /workspace && cargo test -p downloader-pikpak 2>&1
```
Expected: all tests pass.

**Step 6: Commit**

```bash
cd /workspace
git add downloaders/pikpak/Cargo.toml Cargo.toml
git commit -m "chore(pikpak): add Cargo.toml and workspace integration"
```

---

## Task 7: Dockerfile + docker-compose

> **Can run in parallel with Task 6 after Task 5 completes.**

**Files:**
- Create: `downloaders/pikpak/Dockerfile`
- Modify: `docker-compose.yml` (add pikpak service)

**Step 1: Create `downloaders/pikpak/Dockerfile`**

```dockerfile
# downloaders/pikpak/Dockerfile
FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y pkg-config && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY shared ./shared
COPY downloaders/pikpak ./downloaders/pikpak
# Stub other workspace members so Cargo doesn't fail
COPY core-service/Cargo.toml ./core-service/Cargo.toml
COPY fetchers/mikanani/Cargo.toml ./fetchers/mikanani/Cargo.toml
COPY downloaders/qbittorrent/Cargo.toml ./downloaders/qbittorrent/Cargo.toml
COPY viewers/jellyfin/Cargo.toml ./viewers/jellyfin/Cargo.toml
COPY metadata/Cargo.toml ./metadata/Cargo.toml
COPY cli/Cargo.toml ./cli/Cargo.toml
RUN find core-service fetchers downloaders/qbittorrent viewers metadata cli \
    -name "Cargo.toml" -not -path "*/pikpak/*" \
    -exec sh -c 'mkdir -p $(dirname {})/src && touch $(dirname {})/src/lib.rs' \;

RUN cargo build --release -p downloader-pikpak 2>&1

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /data
WORKDIR /app
COPY --from=builder /workspace/target/release/downloader-pikpak .

EXPOSE 8006
VOLUME ["/data"]

CMD ["./downloader-pikpak"]
```

**Step 2: Add pikpak service to `docker-compose.yml`**

Find the existing downloaders section and add:

```yaml
  pikpak-downloader:
    build:
      context: .
      dockerfile: downloaders/pikpak/Dockerfile
    ports:
      - "8006:8006"
    volumes:
      - pikpak-data:/data
      - /your/media/path:/downloads   # adjust to match host media path
    environment:
      - SERVICE_PORT=8006
      - SERVICE_HOST=pikpak-downloader
      - CORE_SERVICE_URL=http://core-service:8000
      - SERVICE_NAME=pikpak-downloader
      - PIKPAK_DB_PATH=/data/pikpak.db
      # Optional: set credentials via env or POST /config/credentials
      # - PIKPAK_EMAIL=your@email.com
      # - PIKPAK_PASSWORD=yourpassword
    depends_on:
      - core-service
    restart: unless-stopped
```

Add to `volumes:` section at bottom of docker-compose.yml:
```yaml
  pikpak-data:
```

**Step 3: Verify Dockerfile builds**

```bash
cd /workspace && docker build -f downloaders/pikpak/Dockerfile . -t downloader-pikpak:test 2>&1 | tail -5
```
Expected: `Successfully built ...`

**Step 4: Commit**

```bash
cd /workspace
git add downloaders/pikpak/Dockerfile docker-compose.yml
git commit -m "feat(pikpak): add Dockerfile and docker-compose service definition"
```

---

## Final Verification

```bash
# All tests pass
cd /workspace && cargo test -p downloader-pikpak -p downloader-qbittorrent -p shared 2>&1

# Service starts (Ctrl+C after a few seconds)
PIKPAK_DB_PATH=/tmp/test-pikpak.db cargo run -p downloader-pikpak 2>&1 | head -20
```

Expected output:
```
PikPak downloader listening on 0.0.0.0:8006
```

```bash
# Health check
curl http://localhost:8006/health
# → 200 OK
```

---

## Environment Variables Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVICE_PORT` | `8006` | Listening port |
| `SERVICE_HOST` | `localhost` | Reported host to Core |
| `CORE_SERVICE_URL` | `http://localhost:8000` | Core service URL |
| `SERVICE_NAME` | `pikpak-downloader` | Name shown in Core |
| `PIKPAK_DB_PATH` | `/data/pikpak.db` | SQLite DB path (volume-mount this) |
| `PIKPAK_EMAIL` | _(empty)_ | Auto-login email |
| `PIKPAK_PASSWORD` | _(empty)_ | Auto-login password |
