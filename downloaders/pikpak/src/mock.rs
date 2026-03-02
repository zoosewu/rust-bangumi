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
    pause_result: RefCell<Result<()>>,
    resume_result: RefCell<Result<()>>,
    delete_result: RefCell<Result<()>>,

    pub login_calls: RefCell<Vec<(String, String)>>,
    pub add_torrents_calls: RefCell<Vec<Vec<DownloadRequestItem>>>,
    pub cancel_torrents_calls: RefCell<Vec<Vec<String>>>,
    pub query_status_calls: RefCell<Vec<Vec<String>>>,
    pub pause_calls: RefCell<Vec<String>>,
    pub resume_calls: RefCell<Vec<String>>,
    pub delete_calls: RefCell<Vec<(String, bool)>>,
}

impl Default for MockPikPakClient {
    fn default() -> Self {
        Self {
            login_result: RefCell::new(Ok(())),
            add_torrents_result: RefCell::new(Ok(vec![])),
            cancel_torrents_result: RefCell::new(Ok(vec![])),
            query_status_result: RefCell::new(Ok(vec![])),
            pause_result: RefCell::new(Ok(())),
            resume_result: RefCell::new(Ok(())),
            delete_result: RefCell::new(Ok(())),
            login_calls: RefCell::new(vec![]),
            add_torrents_calls: RefCell::new(vec![]),
            cancel_torrents_calls: RefCell::new(vec![]),
            query_status_calls: RefCell::new(vec![]),
            pause_calls: RefCell::new(vec![]),
            resume_calls: RefCell::new(vec![]),
            delete_calls: RefCell::new(vec![]),
        }
    }
}

impl MockPikPakClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_login_result(self, r: Result<()>) -> Self {
        *self.login_result.borrow_mut() = r;
        self
    }

    pub fn with_add_torrents_result(self, r: Result<Vec<DownloadResultItem>>) -> Self {
        *self.add_torrents_result.borrow_mut() = r;
        self
    }

    pub fn with_cancel_torrents_result(self, r: Result<Vec<CancelResultItem>>) -> Self {
        *self.cancel_torrents_result.borrow_mut() = r;
        self
    }

    pub fn with_query_status_result(self, r: Result<Vec<DownloadStatusItem>>) -> Self {
        *self.query_status_result.borrow_mut() = r;
        self
    }

    pub fn with_pause_result(self, r: Result<()>) -> Self {
        *self.pause_result.borrow_mut() = r;
        self
    }

    pub fn with_resume_result(self, r: Result<()>) -> Self {
        *self.resume_result.borrow_mut() = r;
        self
    }

    pub fn with_delete_result(self, r: Result<()>) -> Self {
        *self.delete_result.borrow_mut() = r;
        self
    }
}

// SAFETY: MockPikPakClient uses RefCell internally but is designed for single-threaded test use.
// Send + Sync bounds are required by the DownloaderClient trait.
unsafe impl Send for MockPikPakClient {}
unsafe impl Sync for MockPikPakClient {}

impl DownloaderClient for MockPikPakClient {
    async fn login(&self, username: &str, password: &str) -> Result<()> {
        self.login_calls
            .borrow_mut()
            .push((username.to_string(), password.to_string()));
        match &*self.login_result.borrow() {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("{e}")),
        }
    }

    async fn add_torrents(&self, items: Vec<DownloadRequestItem>) -> Result<Vec<DownloadResultItem>> {
        self.add_torrents_calls.borrow_mut().push(items);
        match &*self.add_torrents_result.borrow() {
            Ok(r) => Ok(r.clone()),
            Err(e) => Err(anyhow!("{e}")),
        }
    }

    async fn cancel_torrents(&self, hashes: Vec<String>) -> Result<Vec<CancelResultItem>> {
        self.cancel_torrents_calls.borrow_mut().push(hashes);
        match &*self.cancel_torrents_result.borrow() {
            Ok(r) => Ok(r.clone()),
            Err(e) => Err(anyhow!("{e}")),
        }
    }

    async fn query_status(&self, hashes: Vec<String>) -> Result<Vec<DownloadStatusItem>> {
        self.query_status_calls.borrow_mut().push(hashes);
        match &*self.query_status_result.borrow() {
            Ok(r) => Ok(r.clone()),
            Err(e) => Err(anyhow!("{e}")),
        }
    }

    async fn pause_torrent(&self, hash: &str) -> Result<()> {
        self.pause_calls.borrow_mut().push(hash.to_string());
        match &*self.pause_result.borrow() {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("{e}")),
        }
    }

    async fn resume_torrent(&self, hash: &str) -> Result<()> {
        self.resume_calls.borrow_mut().push(hash.to_string());
        match &*self.resume_result.borrow() {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("{e}")),
        }
    }

    async fn delete_torrent(&self, hash: &str, delete_files: bool) -> Result<()> {
        self.delete_calls
            .borrow_mut()
            .push((hash.to_string(), delete_files));
        match &*self.delete_result.borrow() {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("{e}")),
        }
    }
}
