# Magnet Link Priority for Mikanani Fetcher

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Mikanani fetcher 優先產生 magnet link，撈不到才使用 .torrent URL；Downloader 同時支援 magnet 和 .torrent URL。

**Architecture:** Fetcher 在解析 RSS 時，從 `.torrent` URL 中提取 info hash 並構造 magnet link（mikanani 的 `.torrent` 檔名即為 btih hash）。Downloader 移除 magnet-only 限制，支援任何 qBittorrent API 可接受的 URL 格式（magnet 和 http .torrent）。

**Tech Stack:** Rust, Axum, reqwest, feed-rs, regex

---

### Task 1: Fetcher — 新增 magnet link 轉換函式與單元測試

**Files:**
- Modify: `fetchers/mikanani/src/rss_parser.rs`

**Step 1: 在 `rss_parser.rs` 底部新增 magnet 轉換函式和測試**

在 `impl Default for RssParser` 之後加入：

```rust
/// 常用 BitTorrent tracker 列表
const TRACKERS: &[&str] = &[
    "http://open.acgtracker.com:1096/announce",
    "http://t.nyaatracker.com:80/announce",
    "udp://tracker.openbittorrent.com:80/announce",
];

/// 嘗試從 mikanani 的 .torrent URL 提取 hash 並構造 magnet link
///
/// URL 格式: `https://mikanani.me/Download/{date}/{hash}.torrent`
/// 例如: `https://mikanani.me/Download/20241222/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc.torrent`
fn torrent_url_to_magnet(url: &str) -> Option<String> {
    // 必須是 mikanani 的 .torrent URL
    if !url.contains("mikanani.me") || !url.ends_with(".torrent") {
        return None;
    }

    // 取得最後一段路徑，去掉 .torrent 後綴
    let filename = url.rsplit('/').next()?;
    let hash = filename.strip_suffix(".torrent")?;

    // 驗證 hash 是合法的 hex 字串（至少 32 字元）
    if hash.len() < 32 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let trackers: String = TRACKERS
        .iter()
        .map(|t| format!("&tr={}", t))
        .collect();

    Some(format!("magnet:?xt=urn:btih:{}{}", hash.to_lowercase(), trackers))
}
```

**Step 2: 新增單元測試**

在同檔案底部新增 `#[cfg(test)]` 模組：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torrent_url_to_magnet_valid_mikanani_url() {
        let url = "https://mikanani.me/Download/20241222/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc.torrent";
        let result = torrent_url_to_magnet(url);
        assert!(result.is_some());
        let magnet = result.unwrap();
        assert!(magnet.starts_with("magnet:?xt=urn:btih:ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc"));
        assert!(magnet.contains("&tr="));
    }

    #[test]
    fn test_torrent_url_to_magnet_uppercase_hash_lowered() {
        let url = "https://mikanani.me/Download/20241222/ABCDEF1234567890ABCDEF1234567890ABCDEF12.torrent";
        let result = torrent_url_to_magnet(url);
        assert!(result.is_some());
        assert!(result.unwrap().contains("abcdef1234567890abcdef1234567890abcdef12"));
    }

    #[test]
    fn test_torrent_url_to_magnet_non_mikanani_returns_none() {
        let url = "https://example.com/Download/20241222/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc.torrent";
        assert!(torrent_url_to_magnet(url).is_none());
    }

    #[test]
    fn test_torrent_url_to_magnet_non_torrent_returns_none() {
        let url = "https://mikanani.me/Home/Episode/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc";
        assert!(torrent_url_to_magnet(url).is_none());
    }

    #[test]
    fn test_torrent_url_to_magnet_short_hash_returns_none() {
        let url = "https://mikanani.me/Download/20241222/shorthash.torrent";
        assert!(torrent_url_to_magnet(url).is_none());
    }

    #[test]
    fn test_torrent_url_to_magnet_preserves_valid_magnet() {
        // 已經是 magnet 的不處理
        let url = "magnet:?xt=urn:btih:abc123";
        assert!(torrent_url_to_magnet(url).is_none());
    }
}
```

**Step 3: 執行測試確認通過**

Run: `cargo test -p fetcher-mikanani -- torrent_url_to_magnet`
Expected: 所有 6 個測試通過

**Step 4: Commit**

```bash
git add fetchers/mikanani/src/rss_parser.rs
git commit -m "feat(fetcher): add torrent_url_to_magnet conversion function"
```

---

### Task 2: Fetcher — 整合 magnet 轉換到 RSS 解析流程

**Files:**
- Modify: `fetchers/mikanani/src/rss_parser.rs`

**Step 1: 在 `fetch_raw_items` 中呼叫 `torrent_url_to_magnet`**

修改 `rss_parser.rs` 中 `fetch_raw_items` 方法裡建立 `download_url` 之後的程式碼。在原本的 `if download_url.is_empty()` 檢查之前，加入 magnet 轉換：

```rust
            // 原始下載 URL（從 enclosure 或 link 取得）
            let original_url = entry.media.first()
                .and_then(|m| m.content.first())
                .and_then(|c| c.url.as_ref())
                .map(|u| u.to_string())
                .or_else(|| entry.links.first().map(|l| l.href.clone()))
                .unwrap_or_default();

            if original_url.is_empty() {
                continue;
            }

            // 優先轉換為 magnet link，失敗則使用原始 URL
            let download_url = torrent_url_to_magnet(&original_url)
                .unwrap_or(original_url);
```

這段取代了原本的：

```rust
            let download_url = entry.media.first()
                .and_then(|m| m.content.first())
                .and_then(|c| c.url.as_ref())
                .map(|u| u.to_string())
                .or_else(|| entry.links.first().map(|l| l.href.clone()))
                .unwrap_or_default();

            if download_url.is_empty() {
                continue;
            }
```

**Step 2: 執行所有 fetcher 測試確認不影響現有功能**

Run: `cargo test -p fetcher-mikanani`
Expected: 所有測試通過

**Step 3: Commit**

```bash
git add fetchers/mikanani/src/rss_parser.rs
git commit -m "feat(fetcher): prioritize magnet links over .torrent URLs in RSS parsing"
```

---

### Task 3: Downloader — trait 新增 `add_torrent` 方法與 `extract_hash_from_url`

**Files:**
- Modify: `downloaders/qbittorrent/src/traits.rs`

**Step 1: 新增兩個方法到 `DownloaderClient` trait**

在 `extract_hash_from_magnet` 之後新增：

```rust
    /// Add a torrent by URL (magnet link or .torrent HTTP URL) and return the torrent hash
    fn add_torrent(
        &self,
        url: &str,
        save_path: Option<&str>,
    ) -> impl std::future::Future<Output = Result<String>> + Send;

    /// Extract the info hash from a URL (magnet link or .torrent URL)
    fn extract_hash_from_url(&self, url: &str) -> Result<String>;
```

**Step 2: 執行 cargo check 確認 trait 定義正確**

Run: `cargo check -p downloader-qbittorrent`
Expected: 會有編譯錯誤因為 impl 還沒更新，這是正常的

**Step 3: Commit**

```bash
git add downloaders/qbittorrent/src/traits.rs
git commit -m "feat(downloader): add add_torrent and extract_hash_from_url to DownloaderClient trait"
```

---

### Task 4: Downloader — QBittorrentClient 實作新方法

**Files:**
- Modify: `downloaders/qbittorrent/src/qbittorrent_client.rs`

**Step 1: 新增 `extract_hash_from_url` 方法到 `QBittorrentClient`**

在 `extract_hash_from_magnet` 方法之後新增：

```rust
    pub fn extract_hash_from_url(&self, url: &str) -> Result<String> {
        if url.starts_with("magnet:") {
            return self.extract_hash_from_magnet(url);
        }

        // 嘗試從 .torrent URL 的檔名提取 hash
        // 格式: https://example.com/path/{hash}.torrent
        if url.ends_with(".torrent") {
            if let Some(filename) = url.rsplit('/').next() {
                if let Some(hash) = filename.strip_suffix(".torrent") {
                    let hash = hash.to_lowercase();
                    if hash.len() >= 32 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Ok(hash);
                    }
                }
            }
        }

        Err(anyhow!("Cannot extract hash from URL: {}", url))
    }

    pub async fn add_torrent(&self, url: &str, save_path: Option<&str>) -> Result<String> {
        let mut params = vec![("urls", url)];

        if let Some(path) = save_path {
            params.push(("savepath", path));
        }

        let resp = self
            .client
            .post(format!("{}/api/v2/torrents/add", self.base_url))
            .form(&params)
            .send()
            .await?;

        if resp.status().is_success() {
            tracing::info!("Torrent added successfully");
            let hash = self.extract_hash_from_url(url)?;
            Ok(hash)
        } else {
            Err(anyhow!("Failed to add torrent: {}", resp.status()))
        }
    }
```

**Step 2: 更新 trait impl 區塊**

在 `impl DownloaderClient for QBittorrentClient` 區塊末尾加入：

```rust
    async fn add_torrent(&self, url: &str, save_path: Option<&str>) -> Result<String> {
        QBittorrentClient::add_torrent(self, url, save_path).await
    }

    fn extract_hash_from_url(&self, url: &str) -> Result<String> {
        QBittorrentClient::extract_hash_from_url(self, url)
    }
```

**Step 3: 執行 cargo check**

Run: `cargo check -p downloader-qbittorrent`
Expected: 仍有錯誤（MockDownloaderClient 還沒更新）

**Step 4: Commit**

```bash
git add downloaders/qbittorrent/src/qbittorrent_client.rs
git commit -m "feat(downloader): implement add_torrent and extract_hash_from_url for QBittorrentClient"
```

---

### Task 5: Downloader — MockDownloaderClient 實作新方法

**Files:**
- Modify: `downloaders/qbittorrent/src/mock.rs`

**Step 1: 更新 mock 支援新方法**

在 `MockDownloaderClient` struct 中新增欄位：

```rust
    add_torrent_result: RefCell<Result<String>>,
    pub add_torrent_calls: RefCell<Vec<(String, Option<String>)>>,
```

在 `Default` impl 中初始化：

```rust
    add_torrent_result: RefCell::new(Ok("default_hash".to_string())),
    add_torrent_calls: RefCell::new(vec![]),
```

新增 builder 方法：

```rust
    pub fn with_add_torrent_result(self, result: Result<String>) -> Self {
        *self.add_torrent_result.borrow_mut() = result;
        self
    }
```

新增 `extract_hash_from_url` helper：

```rust
    fn do_extract_hash_from_url(&self, url: &str) -> Result<String> {
        if url.starts_with("magnet:") {
            return self.do_extract_hash(url);
        }
        if url.ends_with(".torrent") {
            if let Some(filename) = url.rsplit('/').next() {
                if let Some(hash) = filename.strip_suffix(".torrent") {
                    let hash = hash.to_lowercase();
                    if hash.len() >= 32 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Ok(hash);
                    }
                }
            }
        }
        Err(anyhow!("Cannot extract hash from URL: {}", url))
    }
```

在 `impl DownloaderClient for MockDownloaderClient` 中新增：

```rust
    async fn add_torrent(&self, url: &str, save_path: Option<&str>) -> Result<String> {
        self.add_torrent_calls
            .borrow_mut()
            .push((url.to_string(), save_path.map(|s| s.to_string())));

        match &*self.add_torrent_result.borrow() {
            Ok(hash) => Ok(hash.clone()),
            Err(e) => Err(anyhow!("{}", e)),
        }
    }

    fn extract_hash_from_url(&self, url: &str) -> Result<String> {
        self.do_extract_hash_from_url(url)
    }
```

**Step 2: 驗證編譯通過**

Run: `cargo check -p downloader-qbittorrent`
Expected: 編譯成功

**Step 3: Commit**

```bash
git add downloaders/qbittorrent/src/mock.rs
git commit -m "feat(downloader): implement add_torrent and extract_hash_from_url for MockDownloaderClient"
```

---

### Task 6: Downloader — 更新 handler 支援 .torrent URL

**Files:**
- Modify: `downloaders/qbittorrent/src/handlers.rs`

**Step 1: 修改 download handler**

將 handler 中的 magnet-only 限制改為接受 magnet 和 http(s) URL：

```rust
pub async fn download<C: DownloaderClient + 'static>(
    State(client): State<Arc<C>>,
    Json(req): Json<DownloadRequest>,
) -> (StatusCode, Json<DownloadResponse>) {
    if !req.url.starts_with("magnet:") && !req.url.starts_with("http") {
        return (
            StatusCode::BAD_REQUEST,
            Json(DownloadResponse {
                status: "unsupported".to_string(),
                hash: None,
                error: Some("Only magnet links and torrent URLs supported".to_string()),
            }),
        );
    }

    // Use retry logic for download with exponential backoff
    let result = retry_with_backoff(3, Duration::from_secs(1), || {
        let client = client.clone();
        let url = req.url.clone();
        async move { client.add_torrent(&url, None).await }
    })
    .await;

    match result {
        Ok(hash) => {
            tracing::info!("Download started: link_id={}, hash={}", req.link_id, hash);
            (
                StatusCode::CREATED,
                Json(DownloadResponse {
                    status: "accepted".to_string(),
                    hash: Some(hash),
                    error: None,
                }),
            )
        }
        Err(e) => {
            tracing::error!("Download failed after retries: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DownloadResponse {
                    status: "error".to_string(),
                    hash: None,
                    error: Some(e.to_string()),
                }),
            )
        }
    }
}
```

**Step 2: 驗證編譯通過**

Run: `cargo check -p downloader-qbittorrent`
Expected: 編譯成功

**Step 3: Commit**

```bash
git add downloaders/qbittorrent/src/handlers.rs
git commit -m "feat(downloader): accept both magnet links and .torrent URLs"
```

---

### Task 7: Downloader — 更新測試

**Files:**
- Modify: `downloaders/qbittorrent/tests/integration/handler_tests.rs`
- Modify: `downloaders/qbittorrent/tests/unit/hash_extraction_tests.rs`

**Step 1: 更新 handler_tests.rs 中複製的 handler 程式碼**

`handler_tests.rs` 中 `mod handlers` 區塊有一份 handler 的複製。需要同步更新：
- 將 `add_magnet` 改為 `add_torrent`
- 將 magnet-only 檢查改為 magnet + http 檢查
- 更新 error message

**Step 2: 更新 `test_download_non_magnet_returns_400`**

這個測試需要改名為 `test_download_unsupported_protocol_returns_400`，因為 http .torrent URL 現在是合法的。測試改為驗證 `ftp://` 等不支援的協議：

```rust
#[tokio::test]
async fn test_download_unsupported_protocol_returns_400() {
    let mock = MockDownloaderClient::new();
    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"ftp://example.com/file.torrent"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

**Step 3: 新增 .torrent URL 接受測試**

```rust
#[tokio::test]
async fn test_download_torrent_url_returns_201() {
    let mock = MockDownloaderClient::new()
        .with_add_torrent_result(Ok("ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc".to_string()));

    let app = create_test_app(mock);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/download")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"link_id":1,"url":"https://mikanani.me/Download/20241222/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc.torrent"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}
```

**Step 4: 更新 `test_download_unsupported_response_format`**

這個測試原本驗證 http URL 被拒絕時的回應格式，需要改為測試不支援協議的回應格式：

```rust
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
                    r#"{"link_id":1,"url":"ftp://not-supported.com"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = parse_response(response).await;
    assert_eq!(body.status, "unsupported");
    assert!(body.error.is_some());
}
```

**Step 5: 在 `hash_extraction_tests.rs` 新增 URL hash 提取測試**

```rust
// ============ URL Hash Extraction Tests ============

#[test]
fn test_extract_hash_from_torrent_url() {
    let client = create_client();
    let url = "https://mikanani.me/Download/20241222/ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc.torrent";

    let hash = client.extract_hash_from_url(url).unwrap();
    assert_eq!(hash, "ced9cfe5ba04d2caadc1ff5366a07a939d25a0bc");
}

#[test]
fn test_extract_hash_from_url_magnet_delegates() {
    let client = create_client();
    let magnet = "magnet:?xt=urn:btih:1234567890abcdef1234567890abcdef&dn=test";

    let hash = client.extract_hash_from_url(magnet).unwrap();
    assert_eq!(hash, "1234567890abcdef1234567890abcdef");
}

#[test]
fn test_extract_hash_from_url_unsupported_fails() {
    let client = create_client();
    let url = "ftp://example.com/something";

    assert!(client.extract_hash_from_url(url).is_err());
}
```

**Step 6: 執行所有 downloader 測試**

Run: `cargo test -p downloader-qbittorrent`
Expected: 所有測試通過

**Step 7: Commit**

```bash
git add downloaders/qbittorrent/tests/
git commit -m "test(downloader): update tests for magnet + torrent URL support"
```

---

### Task 8: 全局驗證和格式化

**Files:** 無新增

**Step 1: 格式化全部程式碼**

Run: `cargo fmt --all`

**Step 2: 跑全部測試**

Run: `cargo test --workspace`
Expected: 所有測試通過（包含 fetcher 和 downloader 的新測試）

**Step 3: 如果有格式變更，commit**

```bash
git add -A
git commit -m "style: cargo fmt"
```
