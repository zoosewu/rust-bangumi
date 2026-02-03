# qBittorrent Downloader 測試架構重構設計

## 背景

目前 downloader-qbittorrent 模組的核心 API 串接方法（login, add_magnet, get_torrent_info 等）完全沒有測試覆蓋。現有的 36+ 個測試僅測試輔助功能（hash 提取、重試邏輯、結構體序列化），實際核心功能覆蓋率為 0%。

主要問題：
- `QBittorrentClient` 是具體實作，無法在測試中替換成 mock
- Handler 直接依賴具體的 HTTP client
- 必須連接真實的 qBittorrent 才能測試 API 操作

## 設計目標

- 所有測試不依賴實際的 qBittorrent 服務即可執行
- 採用 Trait-based 抽象，支援 mock 注入
- 重新組織測試結構，分門別類

## 架構設計

### Trait 定義

新增 `DownloaderClient` trait（使用 Rust 1.75+ 原生 async trait）：

```rust
// src/traits.rs

pub trait DownloaderClient: Send + Sync {
    async fn login(&self, username: &str, password: &str) -> Result<()>;
    async fn add_magnet(&self, magnet_url: &str, save_path: Option<&str>) -> Result<String>;
    async fn get_torrent_info(&self, hash: &str) -> Result<Option<TorrentInfo>>;
    async fn get_all_torrents(&self) -> Result<Vec<TorrentInfo>>;
    async fn pause_torrent(&self, hash: &str) -> Result<()>;
    async fn resume_torrent(&self, hash: &str) -> Result<()>;
    async fn delete_torrent(&self, hash: &str, delete_files: bool) -> Result<()>;
    fn extract_hash_from_magnet(&self, magnet_url: &str) -> Result<String>;
}
```

### Mock 實作

手寫 `MockDownloaderClient`，支援 builder 風格設定：

```rust
// src/mock.rs (#[cfg(test)])

#[derive(Default)]
pub struct MockDownloaderClient {
    // 預設回傳值
    pub login_result: RefCell<Result<()>>,
    pub add_magnet_result: RefCell<Result<String>>,
    pub get_torrent_info_result: RefCell<Result<Option<TorrentInfo>>>,
    pub get_all_torrents_result: RefCell<Result<Vec<TorrentInfo>>>,
    pub pause_result: RefCell<Result<()>>,
    pub resume_result: RefCell<Result<()>>,
    pub delete_result: RefCell<Result<()>>,

    // 記錄呼叫參數
    pub login_calls: RefCell<Vec<(String, String)>>,
    pub add_magnet_calls: RefCell<Vec<String>>,
    pub pause_calls: RefCell<Vec<String>>,
    pub delete_calls: RefCell<Vec<(String, bool)>>,
}

impl MockDownloaderClient {
    pub fn new() -> Self { Default::default() }

    pub fn with_login_result(self, result: Result<()>) -> Self {
        *self.login_result.borrow_mut() = result;
        self
    }

    pub fn with_add_magnet_result(self, result: Result<String>) -> Self {
        *self.add_magnet_result.borrow_mut() = result;
        self
    }
    // ... 其他 with_xxx 方法
}
```

### Handler 泛型化

```rust
// handlers.rs
pub async fn download<C: DownloaderClient>(
    State(client): State<Arc<C>>,
    Json(req): Json<DownloadRequest>,
) -> (StatusCode, Json<DownloadResponse>)

// main.rs
let app = Router::new()
    .route("/download", post(handlers::download::<QBittorrentClient>))
    .route("/health", get(handlers::health_check))
    .with_state(client);
```

## 測試檔案結構

```
/workspace/downloaders/qbittorrent/
├── src/
│   ├── lib.rs              # 加入 pub mod traits; pub mod mock;
│   ├── traits.rs           # [新增] DownloaderClient trait
│   ├── mock.rs             # [新增] #[cfg(test)] MockDownloaderClient
│   ├── qbittorrent_client.rs  # [修改] impl DownloaderClient for ...
│   ├── handlers.rs         # [修改] 泛型化 <C: DownloaderClient>
│   └── retry.rs
└── tests/
    ├── unit/
    │   ├── mod.rs
    │   ├── hash_extraction_tests.rs
    │   ├── retry_tests.rs
    │   └── serialization_tests.rs
    │
    ├── integration/
    │   ├── mod.rs
    │   ├── client_tests.rs
    │   └── handler_tests.rs
    │
    └── common/
        └── mod.rs
```

## 測試案例清單

### 單元測試

#### hash_extraction_tests.rs
- test_extract_hash_from_valid_magnet
- test_extract_hash_with_uppercase_converts_to_lowercase
- test_extract_hash_without_tracker_params
- test_extract_hash_with_multiple_trackers
- test_extract_hash_invalid_url_no_btih
- test_extract_hash_empty_string
- test_extract_hash_short_hash_rejected
- test_extract_hash_non_magnet_protocol
- test_extract_hash_idempotent

#### retry_tests.rs
- test_retry_succeeds_first_attempt
- test_retry_succeeds_second_attempt
- test_retry_succeeds_after_multiple_failures
- test_retry_exhausts_all_attempts
- test_retry_exponential_backoff_timing
- test_retry_preserves_final_error_message

#### serialization_tests.rs
- test_torrent_info_serialize_json
- test_torrent_info_deserialize_json
- test_torrent_info_all_states
- test_torrent_info_progress_boundaries
- test_download_request_deserialize
- test_download_request_missing_field_error
- test_download_response_accepted
- test_download_response_error
- test_download_response_unsupported

### 集成測試

#### client_tests.rs
- test_login_success
- test_login_wrong_credentials_returns_error
- test_login_connection_failed_returns_error
- test_add_magnet_success_returns_hash
- test_add_magnet_invalid_url_rejected
- test_add_magnet_duplicate_torrent_error
- test_add_magnet_records_call_parameters
- test_get_torrent_info_found
- test_get_torrent_info_not_found_returns_none
- test_get_torrent_info_with_correct_hash
- test_get_all_torrents_returns_list
- test_get_all_torrents_empty_list
- test_pause_torrent_success
- test_pause_torrent_not_found_error
- test_resume_torrent_success
- test_delete_torrent_with_files
- test_delete_torrent_without_files
- test_delete_records_delete_files_flag

#### handler_tests.rs
- test_download_valid_magnet_returns_201
- test_download_non_magnet_returns_400
- test_download_invalid_json_returns_400
- test_download_success_response_has_hash
- test_download_error_response_has_message
- test_download_unsupported_response_format
- test_download_client_error_returns_500
- test_download_logs_link_id
- test_download_retries_on_first_failure
- test_download_fails_after_max_retries
- test_health_check_returns_200
- test_download_after_session_expired

## 檔案變更清單

### 新增檔案
| 檔案 | 用途 |
|------|------|
| `src/traits.rs` | `DownloaderClient` trait 定義 |
| `src/mock.rs` | `MockDownloaderClient` 實作 |
| `tests/unit/mod.rs` | 單元測試模組入口 |
| `tests/unit/hash_extraction_tests.rs` | hash 提取測試 |
| `tests/unit/retry_tests.rs` | 重試邏輯測試 |
| `tests/unit/serialization_tests.rs` | 結構體序列化測試 |
| `tests/integration/mod.rs` | 集成測試模組入口 |
| `tests/integration/client_tests.rs` | API 操作測試 |
| `tests/integration/handler_tests.rs` | HTTP handler 測試 |
| `tests/common/mod.rs` | 共用 fixtures |

### 修改檔案
| 檔案 | 變更 |
|------|------|
| `src/lib.rs` | 加入 `pub mod traits;` 和 `#[cfg(test)] pub mod mock;` |
| `src/qbittorrent_client.rs` | 加入 `impl DownloaderClient for QBittorrentClient`，移除內部測試 |
| `src/handlers.rs` | 泛型化 `download<C: DownloaderClient>` |
| `src/retry.rs` | 移除內部測試 |
| `src/main.rs` | 明確指定泛型型別 |
| `Cargo.toml` | 新增 dev-dependencies |

### 刪除檔案
| 檔案 | 原因 |
|------|------|
| `tests/downloader_tests.rs` | 拆分並重新組織到新結構 |

## 依賴變更

```toml
# Cargo.toml
[dev-dependencies]
tower = { workspace = true, features = ["util"] }
http-body-util = "0.1"
```

## 測試數量估計

| 類別 | 數量 |
|------|------|
| 單元測試 | ~18 個 |
| 集成測試（client） | ~16 個 |
| 集成測試（handler） | ~12 個 |
| **總計** | **~46 個** |

## 未來擴展

本設計不包含 core-service 對接（service registration、download progress callback），該部分將在後續設計中處理。
