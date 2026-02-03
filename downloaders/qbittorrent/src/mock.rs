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
