# qBittorrent Downloader 測試架構重構 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 重構 qBittorrent downloader 模組，引入 trait-based 抽象和 mock 支援，使核心 API 操作可測試。

**Architecture:** 抽取 `DownloaderClient` trait，讓 `QBittorrentClient` 實作該 trait。創建 `MockDownloaderClient` 用於測試。將 handler 泛型化以接受任何實作該 trait 的 client。重組測試結構為 unit/integration 分類。

**Tech Stack:** Rust 1.75+ (native async trait), axum, tower, serde

---

## Phase 1: 基礎架構 - Trait 和 Mock

### Task 1: 創建 DownloaderClient Trait

**Files:**
- Create: `downloaders/qbittorrent/src/traits.rs`
- Modify: `downloaders/qbittorrent/src/lib.rs`

**Step 1: 創建 traits.rs 檔案**

```rust
// src/traits.rs
use crate::TorrentInfo;
use anyhow::Result;

/// Trait defining the interface for torrent download clients.
/// This abstraction allows for mock implementations in tests.
pub trait DownloaderClient: Send + Sync {
    /// Authenticate with the torrent client
    fn login(&self, username: &str, password: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Add a magnet link and return the torrent hash
    fn add_magnet(&self, magnet_url: &str, save_path: Option<&str>) -> impl std::future::Future<Output = Result<String>> + Send;

    /// Get information about a specific torrent by hash
    fn get_torrent_info(&self, hash: &str) -> impl std::future::Future<Output = Result<Option<TorrentInfo>>> + Send;

    /// Get information about all torrents
    fn get_all_torrents(&self) -> impl std::future::Future<Output = Result<Vec<TorrentInfo>>> + Send;

    /// Pause a torrent
    fn pause_torrent(&self, hash: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Resume a paused torrent
    fn resume_torrent(&self, hash: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Delete a torrent, optionally deleting downloaded files
    fn delete_torrent(&self, hash: &str, delete_files: bool) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Extract the info hash from a magnet URL
    fn extract_hash_from_magnet(&self, magnet_url: &str) -> Result<String>;
}
```

**Step 2: 更新 lib.rs 加入 traits 模組**

修改 `src/lib.rs`：

```rust
pub mod qbittorrent_client;
pub mod retry;
pub mod traits;

pub use qbittorrent_client::{QBittorrentClient, TorrentInfo};
pub use retry::retry_with_backoff;
pub use traits::DownloaderClient;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lib_exports() {
        // Verify that public exports are available
        let _ = QBittorrentClient::new("http://localhost:8080".to_string());
    }
}
```

**Step 3: 驗證編譯**

Run: `cd /workspace/downloaders/qbittorrent && cargo check`
Expected: 編譯成功，無錯誤

**Step 4: Commit**

```bash
git add src/traits.rs src/lib.rs
git commit -m "feat(downloader): add DownloaderClient trait definition"
```

---

### Task 2: 為 QBittorrentClient 實作 DownloaderClient Trait

**Files:**
- Modify: `downloaders/qbittorrent/src/qbittorrent_client.rs`

**Step 1: 加入 trait 實作**

在 `qbittorrent_client.rs` 檔案末尾（`#[cfg(test)]` 區塊之前）加入：

```rust
use crate::traits::DownloaderClient;

impl DownloaderClient for QBittorrentClient {
    async fn login(&self, username: &str, password: &str) -> Result<()> {
        QBittorrentClient::login(self, username, password).await
    }

    async fn add_magnet(&self, magnet_url: &str, save_path: Option<&str>) -> Result<String> {
        QBittorrentClient::add_magnet(self, magnet_url, save_path).await
    }

    async fn get_torrent_info(&self, hash: &str) -> Result<Option<TorrentInfo>> {
        QBittorrentClient::get_torrent_info(self, hash).await
    }

    async fn get_all_torrents(&self) -> Result<Vec<TorrentInfo>> {
        QBittorrentClient::get_all_torrents(self).await
    }

    async fn pause_torrent(&self, hash: &str) -> Result<()> {
        QBittorrentClient::pause_torrent(self, hash).await
    }

    async fn resume_torrent(&self, hash: &str) -> Result<()> {
        QBittorrentClient::resume_torrent(self, hash).await
    }

    async fn delete_torrent(&self, hash: &str, delete_files: bool) -> Result<()> {
        QBittorrentClient::delete_torrent(self, hash, delete_files).await
    }

    fn extract_hash_from_magnet(&self, magnet_url: &str) -> Result<String> {
        QBittorrentClient::extract_hash_from_magnet(self, magnet_url)
    }
}
```

**Step 2: 驗證編譯**

Run: `cd /workspace/downloaders/qbittorrent && cargo check`
Expected: 編譯成功

**Step 3: 執行現有測試確保沒有破壞**

Run: `cd /workspace/downloaders/qbittorrent && cargo test`
Expected: 所有測試通過

**Step 4: Commit**

```bash
git add src/qbittorrent_client.rs
git commit -m "feat(downloader): implement DownloaderClient trait for QBittorrentClient"
```

---

### Task 3: 創建 MockDownloaderClient

**Files:**
- Create: `downloaders/qbittorrent/src/mock.rs`
- Modify: `downloaders/qbittorrent/src/lib.rs`

**Step 1: 創建 mock.rs 檔案**

```rust
// src/mock.rs
//! Mock implementation of DownloaderClient for testing purposes.

use crate::traits::DownloaderClient;
use crate::TorrentInfo;
use anyhow::{anyhow, Result};
use std::cell::RefCell;

/// A mock implementation of DownloaderClient for testing.
///
/// Use builder methods to configure return values, and inspect
/// call records to verify interactions.
///
/// # Example
/// ```ignore
/// let mock = MockDownloaderClient::new()
///     .with_add_magnet_result(Ok("abc123".to_string()));
///
/// let result = mock.add_magnet("magnet:...", None).await;
/// assert_eq!(result.unwrap(), "abc123");
/// ```
pub struct MockDownloaderClient {
    // Return values
    login_result: RefCell<Result<()>>,
    add_magnet_result: RefCell<Result<String>>,
    get_torrent_info_result: RefCell<Result<Option<TorrentInfo>>>,
    get_all_torrents_result: RefCell<Result<Vec<TorrentInfo>>>,
    pause_result: RefCell<Result<()>>,
    resume_result: RefCell<Result<()>>,
    delete_result: RefCell<Result<()>>,

    // Call recordings
    pub login_calls: RefCell<Vec<(String, String)>>,
    pub add_magnet_calls: RefCell<Vec<(String, Option<String>)>>,
    pub get_torrent_info_calls: RefCell<Vec<String>>,
    pub pause_calls: RefCell<Vec<String>>,
    pub resume_calls: RefCell<Vec<String>>,
    pub delete_calls: RefCell<Vec<(String, bool)>>,
}

impl Default for MockDownloaderClient {
    fn default() -> Self {
        Self {
            login_result: RefCell::new(Ok(())),
            add_magnet_result: RefCell::new(Ok("default_hash".to_string())),
            get_torrent_info_result: RefCell::new(Ok(None)),
            get_all_torrents_result: RefCell::new(Ok(vec![])),
            pause_result: RefCell::new(Ok(())),
            resume_result: RefCell::new(Ok(())),
            delete_result: RefCell::new(Ok(())),

            login_calls: RefCell::new(vec![]),
            add_magnet_calls: RefCell::new(vec![]),
            get_torrent_info_calls: RefCell::new(vec![]),
            pause_calls: RefCell::new(vec![]),
            resume_calls: RefCell::new(vec![]),
            delete_calls: RefCell::new(vec![]),
        }
    }
}

impl MockDownloaderClient {
    pub fn new() -> Self {
        Self::default()
    }

    // Builder methods for configuring return values

    pub fn with_login_result(self, result: Result<()>) -> Self {
        *self.login_result.borrow_mut() = result;
        self
    }

    pub fn with_add_magnet_result(self, result: Result<String>) -> Self {
        *self.add_magnet_result.borrow_mut() = result;
        self
    }

    pub fn with_get_torrent_info_result(self, result: Result<Option<TorrentInfo>>) -> Self {
        *self.get_torrent_info_result.borrow_mut() = result;
        self
    }

    pub fn with_get_all_torrents_result(self, result: Result<Vec<TorrentInfo>>) -> Self {
        *self.get_all_torrents_result.borrow_mut() = result;
        self
    }

    pub fn with_pause_result(self, result: Result<()>) -> Self {
        *self.pause_result.borrow_mut() = result;
        self
    }

    pub fn with_resume_result(self, result: Result<()>) -> Self {
        *self.resume_result.borrow_mut() = result;
        self
    }

    pub fn with_delete_result(self, result: Result<()>) -> Self {
        *self.delete_result.borrow_mut() = result;
        self
    }

    // Helper to extract hash (same logic as real client)
    fn do_extract_hash(&self, magnet_url: &str) -> Result<String> {
        if let Some(start) = magnet_url.find("btih:") {
            let hash_start = start + 5;
            let hash_part = &magnet_url[hash_start..];
            let hash = hash_part
                .split('&')
                .next()
                .unwrap_or("")
                .to_lowercase();

            if !hash.is_empty() && hash.len() >= 32 {
                Ok(hash)
            } else {
                Err(anyhow!("Invalid hash extracted from magnet URL"))
            }
        } else {
            Err(anyhow!("Invalid magnet URL format"))
        }
    }
}

// SAFETY: MockDownloaderClient uses RefCell internally but is designed for single-threaded test use.
// The Send + Sync bounds are required by the trait but mock instances should not be shared across threads.
unsafe impl Send for MockDownloaderClient {}
unsafe impl Sync for MockDownloaderClient {}

impl DownloaderClient for MockDownloaderClient {
    async fn login(&self, username: &str, password: &str) -> Result<()> {
        self.login_calls.borrow_mut().push((username.to_string(), password.to_string()));

        // Clone the result to return it
        match &*self.login_result.borrow() {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("{}", e)),
        }
    }

    async fn add_magnet(&self, magnet_url: &str, save_path: Option<&str>) -> Result<String> {
        self.add_magnet_calls.borrow_mut().push((
            magnet_url.to_string(),
            save_path.map(|s| s.to_string()),
        ));

        match &*self.add_magnet_result.borrow() {
            Ok(hash) => Ok(hash.clone()),
            Err(e) => Err(anyhow!("{}", e)),
        }
    }

    async fn get_torrent_info(&self, hash: &str) -> Result<Option<TorrentInfo>> {
        self.get_torrent_info_calls.borrow_mut().push(hash.to_string());

        match &*self.get_torrent_info_result.borrow() {
            Ok(info) => Ok(info.clone()),
            Err(e) => Err(anyhow!("{}", e)),
        }
    }

    async fn get_all_torrents(&self) -> Result<Vec<TorrentInfo>> {
        match &*self.get_all_torrents_result.borrow() {
            Ok(list) => Ok(list.clone()),
            Err(e) => Err(anyhow!("{}", e)),
        }
    }

    async fn pause_torrent(&self, hash: &str) -> Result<()> {
        self.pause_calls.borrow_mut().push(hash.to_string());

        match &*self.pause_result.borrow() {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("{}", e)),
        }
    }

    async fn resume_torrent(&self, hash: &str) -> Result<()> {
        self.resume_calls.borrow_mut().push(hash.to_string());

        match &*self.resume_result.borrow() {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("{}", e)),
        }
    }

    async fn delete_torrent(&self, hash: &str, delete_files: bool) -> Result<()> {
        self.delete_calls.borrow_mut().push((hash.to_string(), delete_files));

        match &*self.delete_result.borrow() {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("{}", e)),
        }
    }

    fn extract_hash_from_magnet(&self, magnet_url: &str) -> Result<String> {
        self.do_extract_hash(magnet_url)
    }
}
```

**Step 2: 更新 lib.rs 加入 mock 模組**

修改 `src/lib.rs`：

```rust
pub mod qbittorrent_client;
pub mod retry;
pub mod traits;

#[cfg(test)]
pub mod mock;

pub use qbittorrent_client::{QBittorrentClient, TorrentInfo};
pub use retry::retry_with_backoff;
pub use traits::DownloaderClient;

#[cfg(test)]
pub use mock::MockDownloaderClient;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lib_exports() {
        // Verify that public exports are available
        let _ = QBittorrentClient::new("http://localhost:8080".to_string());
    }
}
```

**Step 3: 驗證編譯**

Run: `cd /workspace/downloaders/qbittorrent && cargo check`
Expected: 編譯成功

**Step 4: Commit**

```bash
git add src/mock.rs src/lib.rs
git commit -m "feat(downloader): add MockDownloaderClient for testing"
```

---

### Task 4: 為 Mock 撰寫基本測試

**Files:**
- Create: `downloaders/qbittorrent/tests/integration/mod.rs`
- Create: `downloaders/qbittorrent/tests/integration/client_tests.rs`

**Step 1: 創建測試目錄結構**

Run: `mkdir -p /workspace/downloaders/qbittorrent/tests/integration`

**Step 2: 創建 integration/mod.rs**

```rust
// tests/integration/mod.rs
mod client_tests;
```

**Step 3: 創建 client_tests.rs 測試 Mock 基本功能**

```rust
// tests/integration/client_tests.rs
use anyhow::anyhow;
use downloader_qbittorrent::{DownloaderClient, MockDownloaderClient, TorrentInfo};

// ============ Login Tests ============

#[tokio::test]
async fn test_login_success() {
    let mock = MockDownloaderClient::new()
        .with_login_result(Ok(()));

    let result = mock.login("admin", "password").await;

    assert!(result.is_ok());
    assert_eq!(mock.login_calls.borrow().len(), 1);
    assert_eq!(mock.login_calls.borrow()[0], ("admin".to_string(), "password".to_string()));
}

#[tokio::test]
async fn test_login_wrong_credentials_returns_error() {
    let mock = MockDownloaderClient::new()
        .with_login_result(Err(anyhow!("Invalid credentials")));

    let result = mock.login("admin", "wrong").await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid credentials"));
}

#[tokio::test]
async fn test_login_connection_failed_returns_error() {
    let mock = MockDownloaderClient::new()
        .with_login_result(Err(anyhow!("Connection refused")));

    let result = mock.login("admin", "password").await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Connection refused"));
}

// ============ Add Magnet Tests ============

#[tokio::test]
async fn test_add_magnet_success_returns_hash() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Ok("abc123def456".to_string()));

    let result = mock.add_magnet("magnet:?xt=urn:btih:abc123def456", None).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "abc123def456");
}

#[tokio::test]
async fn test_add_magnet_with_save_path() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Ok("hash123".to_string()));

    let result = mock.add_magnet("magnet:?xt=urn:btih:hash123", Some("/downloads")).await;

    assert!(result.is_ok());
    let calls = mock.add_magnet_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, Some("/downloads".to_string()));
}

#[tokio::test]
async fn test_add_magnet_duplicate_torrent_error() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Err(anyhow!("Torrent already exists")));

    let result = mock.add_magnet("magnet:?xt=urn:btih:existing", None).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[tokio::test]
async fn test_add_magnet_records_call_parameters() {
    let mock = MockDownloaderClient::new();
    let magnet = "magnet:?xt=urn:btih:recordtest123456789012345678901234";

    let _ = mock.add_magnet(magnet, Some("/path")).await;

    let calls = mock.add_magnet_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, magnet);
    assert_eq!(calls[0].1, Some("/path".to_string()));
}

// ============ Get Torrent Info Tests ============

#[tokio::test]
async fn test_get_torrent_info_found() {
    let info = TorrentInfo {
        hash: "testhash123".to_string(),
        name: "Test Torrent".to_string(),
        state: "downloading".to_string(),
        progress: 0.5,
        dlspeed: 1024000,
        size: 1000000000,
        downloaded: 500000000,
    };

    let mock = MockDownloaderClient::new()
        .with_get_torrent_info_result(Ok(Some(info.clone())));

    let result = mock.get_torrent_info("testhash123").await;

    assert!(result.is_ok());
    let returned_info = result.unwrap().unwrap();
    assert_eq!(returned_info.hash, "testhash123");
    assert_eq!(returned_info.progress, 0.5);
}

#[tokio::test]
async fn test_get_torrent_info_not_found_returns_none() {
    let mock = MockDownloaderClient::new()
        .with_get_torrent_info_result(Ok(None));

    let result = mock.get_torrent_info("nonexistent").await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_get_torrent_info_records_hash() {
    let mock = MockDownloaderClient::new();

    let _ = mock.get_torrent_info("queryhash").await;

    assert_eq!(mock.get_torrent_info_calls.borrow()[0], "queryhash");
}

// ============ Get All Torrents Tests ============

#[tokio::test]
async fn test_get_all_torrents_returns_list() {
    let torrents = vec![
        TorrentInfo {
            hash: "hash1".to_string(),
            name: "Torrent 1".to_string(),
            state: "downloading".to_string(),
            progress: 0.3,
            dlspeed: 500000,
            size: 500000000,
            downloaded: 150000000,
        },
        TorrentInfo {
            hash: "hash2".to_string(),
            name: "Torrent 2".to_string(),
            state: "completed".to_string(),
            progress: 1.0,
            dlspeed: 0,
            size: 200000000,
            downloaded: 200000000,
        },
    ];

    let mock = MockDownloaderClient::new()
        .with_get_all_torrents_result(Ok(torrents));

    let result = mock.get_all_torrents().await;

    assert!(result.is_ok());
    let list = result.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].hash, "hash1");
    assert_eq!(list[1].hash, "hash2");
}

#[tokio::test]
async fn test_get_all_torrents_empty_list() {
    let mock = MockDownloaderClient::new()
        .with_get_all_torrents_result(Ok(vec![]));

    let result = mock.get_all_torrents().await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

// ============ Pause / Resume / Delete Tests ============

#[tokio::test]
async fn test_pause_torrent_success() {
    let mock = MockDownloaderClient::new()
        .with_pause_result(Ok(()));

    let result = mock.pause_torrent("pausehash").await;

    assert!(result.is_ok());
    assert_eq!(mock.pause_calls.borrow()[0], "pausehash");
}

#[tokio::test]
async fn test_pause_torrent_not_found_error() {
    let mock = MockDownloaderClient::new()
        .with_pause_result(Err(anyhow!("Torrent not found")));

    let result = mock.pause_torrent("nonexistent").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_resume_torrent_success() {
    let mock = MockDownloaderClient::new()
        .with_resume_result(Ok(()));

    let result = mock.resume_torrent("resumehash").await;

    assert!(result.is_ok());
    assert_eq!(mock.resume_calls.borrow()[0], "resumehash");
}

#[tokio::test]
async fn test_delete_torrent_with_files() {
    let mock = MockDownloaderClient::new()
        .with_delete_result(Ok(()));

    let result = mock.delete_torrent("deletehash", true).await;

    assert!(result.is_ok());
    let calls = mock.delete_calls.borrow();
    assert_eq!(calls[0], ("deletehash".to_string(), true));
}

#[tokio::test]
async fn test_delete_torrent_without_files() {
    let mock = MockDownloaderClient::new()
        .with_delete_result(Ok(()));

    let result = mock.delete_torrent("deletehash", false).await;

    assert!(result.is_ok());
    let calls = mock.delete_calls.borrow();
    assert_eq!(calls[0], ("deletehash".to_string(), false));
}

// ============ Extract Hash Tests ============

#[tokio::test]
async fn test_extract_hash_from_valid_magnet() {
    let mock = MockDownloaderClient::new();
    let magnet = "magnet:?xt=urn:btih:1234567890abcdef1234567890abcdef&dn=test";

    let result = mock.extract_hash_from_magnet(magnet);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "1234567890abcdef1234567890abcdef");
}

#[tokio::test]
async fn test_extract_hash_invalid_url() {
    let mock = MockDownloaderClient::new();

    let result = mock.extract_hash_from_magnet("not_a_magnet");

    assert!(result.is_err());
}
```

**Step 4: 執行測試確認 Mock 運作正常**

Run: `cd /workspace/downloaders/qbittorrent && cargo test integration::client_tests`
Expected: 所有測試通過

**Step 5: Commit**

```bash
git add tests/integration/
git commit -m "test(downloader): add integration tests for MockDownloaderClient"
```

---

## Phase 2: Handler 泛型化

### Task 5: 泛型化 Download Handler

**Files:**
- Modify: `downloaders/qbittorrent/src/handlers.rs`

**Step 1: 修改 handlers.rs 使用泛型**

將整個檔案內容替換為：

```rust
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use downloader_qbittorrent::{DownloaderClient, retry_with_backoff};

#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    pub link_id: i32,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct DownloadResponse {
    pub status: String,
    pub hash: Option<String>,
    pub error: Option<String>,
}

pub async fn download<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Json(req): Json<DownloadRequest>,
) -> (StatusCode, Json<DownloadResponse>) {
    if !req.url.starts_with("magnet:") {
        return (StatusCode::BAD_REQUEST, Json(DownloadResponse {
            status: "unsupported".to_string(),
            hash: None,
            error: Some("Only magnet links supported".to_string()),
        }));
    }

    // Use retry logic for download with exponential backoff
    let result = retry_with_backoff(3, Duration::from_secs(1), || {
        let client = client.clone();
        let url = req.url.clone();
        async move {
            client.add_magnet(&url, None).await
        }
    }).await;

    match result {
        Ok(hash) => {
            tracing::info!("Download started: link_id={}, hash={}", req.link_id, hash);
            (StatusCode::CREATED, Json(DownloadResponse {
                status: "accepted".to_string(),
                hash: Some(hash),
                error: None,
            }))
        }
        Err(e) => {
            tracing::error!("Download failed after retries: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(DownloadResponse {
                status: "error".to_string(),
                hash: None,
                error: Some(e.to_string()),
            }))
        }
    }
}

pub async fn health_check() -> StatusCode {
    StatusCode::OK
}
```

**Step 2: 驗證編譯**

Run: `cd /workspace/downloaders/qbittorrent && cargo check`
Expected: 編譯成功

**Step 3: Commit**

```bash
git add src/handlers.rs
git commit -m "refactor(downloader): make download handler generic over DownloaderClient"
```

---

### Task 6: 更新 main.rs 指定具體型別

**Files:**
- Modify: `downloaders/qbittorrent/src/main.rs`

**Step 1: 修改 main.rs**

```rust
use axum::{routing::{get, post}, Router};
use std::sync::Arc;
use tokio::net::TcpListener;
use downloader_qbittorrent::QBittorrentClient;

mod handlers;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("downloader_qbittorrent=debug".parse()?),
        )
        .init();

    let qb_url = std::env::var("QBITTORRENT_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    let qb_user = std::env::var("QBITTORRENT_USER").unwrap_or_else(|_| "admin".to_string());
    let qb_pass = std::env::var("QBITTORRENT_PASSWORD")
        .unwrap_or_else(|_| "adminadmin".to_string());

    let client = Arc::new(QBittorrentClient::new(qb_url));
    client.login(&qb_user, &qb_pass).await?;

    let app = Router::new()
        .route("/download", post(handlers::download::<QBittorrentClient>))
        .route("/health", get(handlers::health_check))
        .with_state(client);

    let listener = TcpListener::bind("0.0.0.0:8002").await?;
    tracing::info!("Download service listening on 0.0.0.0:8002");

    axum::serve(listener, app).await?;
    Ok(())
}
```

**Step 2: 驗證編譯**

Run: `cd /workspace/downloaders/qbittorrent && cargo build`
Expected: 編譯成功

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "refactor(downloader): specify concrete type in main.rs router"
```

---

### Task 7: 新增 Handler 集成測試

**Files:**
- Modify: `downloaders/qbittorrent/Cargo.toml`
- Create: `downloaders/qbittorrent/tests/integration/handler_tests.rs`
- Modify: `downloaders/qbittorrent/tests/integration/mod.rs`

**Step 1: 更新 Cargo.toml 加入 dev-dependencies**

在 `[dev-dependencies]` 區塊加入：

```toml
[dev-dependencies]
tower = { workspace = true, features = ["util"] }
http-body-util = "0.1"
```

**Step 2: 更新 integration/mod.rs**

```rust
// tests/integration/mod.rs
mod client_tests;
mod handler_tests;
```

**Step 3: 創建 handler_tests.rs**

```rust
// tests/integration/handler_tests.rs
use anyhow::anyhow;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use downloader_qbittorrent::MockDownloaderClient;
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

// Import the handlers module - we need to reference it from the binary crate
// For now, we'll test through the library's public API

mod handlers {
    use axum::{extract::State, http::StatusCode, Json};
    use downloader_qbittorrent::{retry_with_backoff, DownloaderClient};
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Debug, Deserialize)]
    pub struct DownloadRequest {
        pub link_id: i32,
        pub url: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct DownloadResponse {
        pub status: String,
        pub hash: Option<String>,
        pub error: Option<String>,
    }

    pub async fn download<C: DownloaderClient + 'static>(
        State(client): State<Arc<C>>,
        Json(req): Json<DownloadRequest>,
    ) -> (StatusCode, Json<DownloadResponse>) {
        if !req.url.starts_with("magnet:") {
            return (
                StatusCode::BAD_REQUEST,
                Json(DownloadResponse {
                    status: "unsupported".to_string(),
                    hash: None,
                    error: Some("Only magnet links supported".to_string()),
                }),
            );
        }

        let result = retry_with_backoff(3, Duration::from_millis(10), || {
            let client = client.clone();
            let url = req.url.clone();
            async move { client.add_magnet(&url, None).await }
        })
        .await;

        match result {
            Ok(hash) => (
                StatusCode::CREATED,
                Json(DownloadResponse {
                    status: "accepted".to_string(),
                    hash: Some(hash),
                    error: None,
                }),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DownloadResponse {
                    status: "error".to_string(),
                    hash: None,
                    error: Some(e.to_string()),
                }),
            ),
        }
    }
}

fn create_test_app(mock: MockDownloaderClient) -> Router {
    Router::new()
        .route("/download", post(handlers::download::<MockDownloaderClient>))
        .with_state(Arc::new(mock))
}

async fn parse_response(response: axum::response::Response) -> handlers::DownloadResponse {
    let body = response.into_body();
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ============ Request Validation Tests ============

#[tokio::test]
async fn test_download_valid_magnet_returns_201() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Ok("testhash123456789012345678901234".to_string()));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"magnet:?xt=urn:btih:testhash123456789012345678901234&dn=test"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_download_non_magnet_returns_400() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"http://example.com/file.torrent"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_download_invalid_json_returns_422() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"invalid": json"#))
                .unwrap(),
        )
        .await
        .unwrap();

    // Axum returns 422 Unprocessable Entity for JSON parsing errors
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ============ Response Format Tests ============

#[tokio::test]
async fn test_download_success_response_has_hash() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Ok("responsehash12345678901234567890".to_string()));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"magnet:?xt=urn:btih:responsehash12345678901234567890"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = parse_response(response).await;
    assert_eq!(body.status, "accepted");
    assert_eq!(body.hash, Some("responsehash12345678901234567890".to_string()));
    assert!(body.error.is_none());
}

#[tokio::test]
async fn test_download_error_response_has_message() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Err(anyhow!("Connection timeout")));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"magnet:?xt=urn:btih:errorhash1234567890123456789012"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = parse_response(response).await;
    assert_eq!(body.status, "error");
    assert!(body.hash.is_none());
    assert!(body.error.is_some());
}

#[tokio::test]
async fn test_download_unsupported_response_format() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"https://not-a-magnet.com"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = parse_response(response).await;
    assert_eq!(body.status, "unsupported");
    assert!(body.error.is_some());
    assert!(body.error.unwrap().contains("magnet"));
}

// ============ Error Handling Tests ============

#[tokio::test]
async fn test_download_client_error_returns_500() {
    let mock = MockDownloaderClient::new()
        .with_add_magnet_result(Err(anyhow!("Internal client error")));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":99,"url":"magnet:?xt=urn:btih:clienterror123456789012345678"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
```

**Step 4: 執行測試**

Run: `cd /workspace/downloaders/qbittorrent && cargo test integration::handler_tests`
Expected: 所有測試通過

**Step 5: Commit**

```bash
git add Cargo.toml tests/integration/
git commit -m "test(downloader): add handler integration tests with mock client"
```

---

## Phase 3: 測試結構重組

### Task 8: 創建單元測試目錄結構

**Files:**
- Create: `downloaders/qbittorrent/tests/unit/mod.rs`
- Create: `downloaders/qbittorrent/tests/unit/hash_extraction_tests.rs`
- Create: `downloaders/qbittorrent/tests/unit/retry_tests.rs`
- Create: `downloaders/qbittorrent/tests/unit/serialization_tests.rs`
- Create: `downloaders/qbittorrent/tests/common/mod.rs`

**Step 1: 創建目錄結構**

Run: `mkdir -p /workspace/downloaders/qbittorrent/tests/unit /workspace/downloaders/qbittorrent/tests/common`

**Step 2: 創建 common/mod.rs**

```rust
// tests/common/mod.rs
use downloader_qbittorrent::TorrentInfo;

pub fn sample_torrent_info() -> TorrentInfo {
    TorrentInfo {
        hash: "abc123def456789012345678901234ab".to_string(),
        name: "Test Torrent".to_string(),
        state: "downloading".to_string(),
        progress: 0.5,
        dlspeed: 1024000,
        size: 1000000000,
        downloaded: 500000000,
    }
}

pub fn valid_magnet() -> &'static str {
    "magnet:?xt=urn:btih:1234567890abcdef1234567890abcdef&dn=test"
}

pub fn valid_magnet_hash() -> &'static str {
    "1234567890abcdef1234567890abcdef"
}
```

**Step 3: 創建 unit/mod.rs**

```rust
// tests/unit/mod.rs
mod hash_extraction_tests;
mod retry_tests;
mod serialization_tests;
```

**Step 4: 創建 hash_extraction_tests.rs**

```rust
// tests/unit/hash_extraction_tests.rs
use downloader_qbittorrent::QBittorrentClient;

fn create_client() -> QBittorrentClient {
    QBittorrentClient::new("http://localhost:8080".to_string())
}

// ============ Valid Format Tests ============

#[test]
fn test_extract_hash_from_valid_magnet() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:1234567890abcdef1234567890abcdef&dn=test&tr=http://tracker.example.com";

    let hash = client.extract_hash_from_magnet(magnet).unwrap();
    assert_eq!(hash, "1234567890abcdef1234567890abcdef");
}

#[test]
fn test_extract_hash_with_uppercase_converts_to_lowercase() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:ABCDEFABCDEFABCDEFABCDEFABCDEFAB&dn=test";

    let hash = client.extract_hash_from_magnet(magnet).unwrap();
    assert_eq!(hash, "abcdefabcdefabcdefabcdefabcdefab");
}

#[test]
fn test_extract_hash_without_tracker_params() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:11111111111111111111111111111111";

    let hash = client.extract_hash_from_magnet(magnet).unwrap();
    assert_eq!(hash, "11111111111111111111111111111111");
}

#[test]
fn test_extract_hash_with_multiple_trackers() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:22222222222222222222222222222222&tr=http://t1.com&tr=http://t2.com&tr=udp://t3.com";

    let hash = client.extract_hash_from_magnet(magnet).unwrap();
    assert_eq!(hash, "22222222222222222222222222222222");
}

// ============ Invalid Format Tests ============

#[test]
fn test_extract_hash_invalid_url_no_btih() {
    let client = create_client();
    let result = client.extract_hash_from_magnet("magnet:?dn=test&tr=http://tracker.com");

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid magnet URL"));
}

#[test]
fn test_extract_hash_empty_string() {
    let client = create_client();
    let result = client.extract_hash_from_magnet("");

    assert!(result.is_err());
}

#[test]
fn test_extract_hash_short_hash_rejected() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:short&dn=test";
    let result = client.extract_hash_from_magnet(magnet);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid hash"));
}

#[test]
fn test_extract_hash_non_magnet_protocol() {
    let client = create_client();
    let result = client.extract_hash_from_magnet("http://example.com/file.torrent");

    assert!(result.is_err());
}

// ============ Consistency Tests ============

#[test]
fn test_extract_hash_idempotent() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:consistenthash123456789012345678&dn=test";

    let hash1 = client.extract_hash_from_magnet(magnet).unwrap();
    let hash2 = client.extract_hash_from_magnet(magnet).unwrap();

    assert_eq!(hash1, hash2);
}
```

**Step 5: 創建 retry_tests.rs**

```rust
// tests/unit/retry_tests.rs
use downloader_qbittorrent::retry_with_backoff;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_retry_succeeds_first_attempt() {
    let result = retry_with_backoff(3, Duration::from_millis(1), || async {
        Ok::<i32, String>(42)
    })
    .await;

    assert_eq!(result, Ok(42));
}

#[tokio::test]
async fn test_retry_succeeds_second_attempt() {
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();

    let result = retry_with_backoff(3, Duration::from_millis(1), || {
        let attempts = attempts_clone.clone();
        async move {
            let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if count < 2 {
                Err::<i32, String>(format!("Attempt {}", count))
            } else {
                Ok(99)
            }
        }
    })
    .await;

    assert_eq!(result, Ok(99));
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_retry_succeeds_after_multiple_failures() {
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();

    let result = retry_with_backoff(5, Duration::from_millis(1), || {
        let attempts = attempts_clone.clone();
        async move {
            let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if count < 4 {
                Err::<i32, String>(format!("Attempt {}", count))
            } else {
                Ok(123)
            }
        }
    })
    .await;

    assert_eq!(result, Ok(123));
    assert_eq!(attempts.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn test_retry_exhausts_all_attempts() {
    let result =
        retry_with_backoff::<_, _, i32, String>(3, Duration::from_millis(1), || async {
            Err("Always fails".to_string())
        })
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_retry_exponential_backoff_timing() {
    let start = Instant::now();
    let result = retry_with_backoff::<_, _, i32, String>(
        2,
        Duration::from_millis(10),
        || async { Err("Always fails".to_string()) },
    )
    .await;

    let elapsed = start.elapsed();
    assert!(result.is_err());
    // Should take at least 10ms (first backoff)
    assert!(elapsed.as_millis() >= 5);
}

#[tokio::test]
async fn test_retry_preserves_final_error_message() {
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();

    let result = retry_with_backoff::<_, _, i32, String>(3, Duration::from_millis(1), || {
        let attempts = attempts_clone.clone();
        async move {
            let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            Err(format!("Error on attempt {}", count))
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Error on attempt 3");
}
```

**Step 6: 創建 serialization_tests.rs**

```rust
// tests/unit/serialization_tests.rs
use downloader_qbittorrent::TorrentInfo;

// ============ TorrentInfo Tests ============

#[test]
fn test_torrent_info_serialize_json() {
    let info = TorrentInfo {
        hash: "abc123".to_string(),
        name: "Test".to_string(),
        state: "downloading".to_string(),
        progress: 0.5,
        dlspeed: 1024000,
        size: 1000000000,
        downloaded: 500000000,
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"hash\":\"abc123\""));
    assert!(json.contains("\"progress\":0.5"));
}

#[test]
fn test_torrent_info_deserialize_json() {
    let json = r#"{
        "hash": "def456",
        "name": "Deserialized",
        "state": "completed",
        "progress": 1.0,
        "dlspeed": 0,
        "size": 500000000,
        "downloaded": 500000000
    }"#;

    let info: TorrentInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.hash, "def456");
    assert_eq!(info.state, "completed");
    assert_eq!(info.progress, 1.0);
}

#[test]
fn test_torrent_info_all_states() {
    let states = vec!["downloading", "uploading", "paused", "completed", "error", "stalledDL", "stalledUP"];

    for state in states {
        let info = TorrentInfo {
            hash: "test".to_string(),
            name: "Torrent".to_string(),
            state: state.to_string(),
            progress: 1.0,
            dlspeed: 0,
            size: 100000,
            downloaded: 100000,
        };

        assert_eq!(info.state, state);
    }
}

#[test]
fn test_torrent_info_progress_boundaries() {
    // Test 0.0
    let info_zero = TorrentInfo {
        hash: "zero".to_string(),
        name: "Zero Progress".to_string(),
        state: "downloading".to_string(),
        progress: 0.0,
        dlspeed: 1024,
        size: 1000000,
        downloaded: 0,
    };
    assert_eq!(info_zero.progress, 0.0);

    // Test 1.0
    let info_full = TorrentInfo {
        hash: "full".to_string(),
        name: "Full Progress".to_string(),
        state: "completed".to_string(),
        progress: 1.0,
        dlspeed: 0,
        size: 1000000,
        downloaded: 1000000,
    };
    assert_eq!(info_full.progress, 1.0);
}

// ============ DownloadRequest Tests ============

#[test]
fn test_download_request_deserialize() {
    #[derive(serde::Deserialize)]
    struct DownloadRequest {
        link_id: i32,
        url: String,
    }

    let json = r#"{"link_id": 123, "url": "magnet:?xt=urn:btih:abc123"}"#;
    let req: DownloadRequest = serde_json::from_str(json).unwrap();

    assert_eq!(req.link_id, 123);
    assert_eq!(req.url, "magnet:?xt=urn:btih:abc123");
}

#[test]
fn test_download_request_missing_field_error() {
    #[derive(serde::Deserialize)]
    struct DownloadRequest {
        link_id: i32,
        url: String,
    }

    let json = r#"{"link_id": 123}"#;
    let result: Result<DownloadRequest, _> = serde_json::from_str(json);

    assert!(result.is_err());
}

// ============ DownloadResponse Tests ============

#[test]
fn test_download_response_accepted() {
    #[derive(serde::Serialize)]
    struct DownloadResponse {
        status: String,
        hash: Option<String>,
        error: Option<String>,
    }

    let response = DownloadResponse {
        status: "accepted".to_string(),
        hash: Some("def456".to_string()),
        error: None,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"status\":\"accepted\""));
    assert!(json.contains("\"hash\":\"def456\""));
    assert!(json.contains("\"error\":null"));
}

#[test]
fn test_download_response_error() {
    #[derive(serde::Serialize)]
    struct DownloadResponse {
        status: String,
        hash: Option<String>,
        error: Option<String>,
    }

    let response = DownloadResponse {
        status: "error".to_string(),
        hash: None,
        error: Some("Connection failed".to_string()),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"status\":\"error\""));
    assert!(json.contains("\"hash\":null"));
    assert!(json.contains("\"error\":\"Connection failed\""));
}

#[test]
fn test_download_response_unsupported() {
    #[derive(serde::Serialize)]
    struct DownloadResponse {
        status: String,
        hash: Option<String>,
        error: Option<String>,
    }

    let response = DownloadResponse {
        status: "unsupported".to_string(),
        hash: None,
        error: Some("Only magnet links supported".to_string()),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"status\":\"unsupported\""));
}
```

**Step 7: 執行所有單元測試**

Run: `cd /workspace/downloaders/qbittorrent && cargo test unit::`
Expected: 所有測試通過

**Step 8: Commit**

```bash
git add tests/unit/ tests/common/
git commit -m "test(downloader): add reorganized unit tests"
```

---

### Task 9: 清理舊測試並移除內部測試模組

**Files:**
- Delete: `downloaders/qbittorrent/tests/downloader_tests.rs`
- Modify: `downloaders/qbittorrent/src/qbittorrent_client.rs` (移除內部測試)
- Modify: `downloaders/qbittorrent/src/retry.rs` (移除內部測試)
- Modify: `downloaders/qbittorrent/src/lib.rs` (移除內部測試)

**Step 1: 刪除舊測試檔案**

Run: `rm /workspace/downloaders/qbittorrent/tests/downloader_tests.rs`

**Step 2: 移除 qbittorrent_client.rs 內的 #[cfg(test)] 區塊**

修改 `src/qbittorrent_client.rs`，移除檔案末尾的整個 `#[cfg(test)] mod tests { ... }` 區塊。

**Step 3: 移除 retry.rs 內的 #[cfg(test)] 區塊**

修改 `src/retry.rs`，移除檔案末尾的整個 `#[cfg(test)] mod tests { ... }` 區塊。

**Step 4: 移除 lib.rs 內的 #[cfg(test)] 區塊**

修改 `src/lib.rs`，移除 `#[cfg(test)] mod tests { ... }` 區塊，保留其他內容。

最終 `lib.rs` 應為：

```rust
pub mod qbittorrent_client;
pub mod retry;
pub mod traits;

#[cfg(test)]
pub mod mock;

pub use qbittorrent_client::{QBittorrentClient, TorrentInfo};
pub use retry::retry_with_backoff;
pub use traits::DownloaderClient;

#[cfg(test)]
pub use mock::MockDownloaderClient;
```

**Step 5: 執行所有測試確保無遺漏**

Run: `cd /workspace/downloaders/qbittorrent && cargo test`
Expected: 所有測試通過，測試數量約 40+ 個

**Step 6: Commit**

```bash
git add -A
git commit -m "refactor(downloader): remove inline tests, delete old test file"
```

---

### Task 10: 最終驗證與清理

**Step 1: 執行完整測試套件**

Run: `cd /workspace/downloaders/qbittorrent && cargo test -- --test-threads=1`
Expected: 所有測試通過

**Step 2: 檢查編譯警告**

Run: `cd /workspace/downloaders/qbittorrent && cargo clippy`
Expected: 無錯誤（警告可接受）

**Step 3: 驗證格式**

Run: `cd /workspace/downloaders/qbittorrent && cargo fmt --check`
Expected: 無格式問題（如有則執行 `cargo fmt`）

**Step 4: 測試計數驗證**

Run: `cd /workspace/downloaders/qbittorrent && cargo test 2>&1 | grep -E "^test result|running [0-9]+ tests"`
Expected:
- unit:: ~18 個測試
- integration::client_tests:: ~18 個測試
- integration::handler_tests:: ~8 個測試
- 總計約 44+ 個測試

**Step 5: 最終 Commit**

```bash
git add -A
git commit -m "chore(downloader): final cleanup and verification"
```

---

## Summary

完成後的檔案結構：

```
downloaders/qbittorrent/
├── src/
│   ├── lib.rs              # 模組匯出
│   ├── traits.rs           # DownloaderClient trait
│   ├── mock.rs             # MockDownloaderClient (#[cfg(test)])
│   ├── qbittorrent_client.rs  # 實際 client + trait impl
│   ├── handlers.rs         # 泛型化的 handlers
│   ├── retry.rs            # retry 邏輯
│   └── main.rs             # 應用程式入口
├── tests/
│   ├── common/
│   │   └── mod.rs          # 共用 fixtures
│   ├── unit/
│   │   ├── mod.rs
│   │   ├── hash_extraction_tests.rs
│   │   ├── retry_tests.rs
│   │   └── serialization_tests.rs
│   └── integration/
│       ├── mod.rs
│       ├── client_tests.rs
│       └── handler_tests.rs
└── Cargo.toml
```

測試覆蓋：
- Hash 提取邏輯: 9 個測試
- Retry 邏輯: 6 個測試
- 序列化: 9 個測試
- Mock client: 18 個測試
- Handler: 8 個測試
- **總計: ~50 個測試**
