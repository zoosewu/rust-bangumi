# Auto-Download Dispatch System Design

> Date: 2026-02-06
> Status: Approved

## 1. Overview & Data Model

### Problem
Core service 目前沒有自動下載派發機制。Fetcher 回傳的 RSS 結果被儲存後，沒有任何邏輯將下載連結發送給 Downloader。

### Solution
Core 作為中央調度器，在 Fetcher 結果全部處理完畢後，批次將下載連結派發給合適的 Downloader。

### New Enum: DownloadType

在 `shared` crate 定義：

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadType {
    Magnet,
    Torrent,
    Http,
}
```

對應 PostgreSQL enum：

```sql
CREATE TYPE download_type AS ENUM ('magnet', 'torrent', 'http');
```

### Schema Changes

**新增 junction table：**

```sql
CREATE TABLE downloader_capabilities (
    module_id INT REFERENCES service_modules(module_id) ON DELETE CASCADE,
    download_type download_type NOT NULL,
    PRIMARY KEY (module_id, download_type)
);
```

**修改 `anime_links`：**

```sql
ALTER TABLE anime_links ADD COLUMN download_type download_type;
```

**修改 `downloads`：**

```sql
ALTER TABLE downloads ADD COLUMN module_id INT REFERENCES service_modules(module_id);
-- status 擴展為：
-- pending, downloading, completed, failed, cancelled, downloader_error, no_downloader
```

### Priority Direction

全系統統一：**數字越大 = 優先權越高**，查詢時使用 `ORDER BY priority DESC`，預設值 50。

---

## 2. Chain of Responsibility — URL Type Detection

Core 使用 Chain of Responsibility 模式判斷 `download_type`。Fetcher 完全不參與類型判斷。

### Trait 定義

```rust
trait DownloadTypeDetector {
    fn detect(&self, url: &str) -> Option<DownloadType>;
}
```

### Detector Chain

```rust
struct MagnetDetector;
impl DownloadTypeDetector for MagnetDetector {
    fn detect(&self, url: &str) -> Option<DownloadType> {
        url.starts_with("magnet:").then_some(DownloadType::Magnet)
    }
}

struct TorrentDetector;
impl DownloadTypeDetector for TorrentDetector {
    fn detect(&self, url: &str) -> Option<DownloadType> {
        (url.starts_with("http") && url.contains(".torrent"))
            .then_some(DownloadType::Torrent)
    }
}

struct HttpDetector;
impl DownloadTypeDetector for HttpDetector {
    fn detect(&self, url: &str) -> Option<DownloadType> {
        url.starts_with("http").then_some(DownloadType::Http)
    }
}
```

### Public API

```rust
pub fn detect_download_type(url: &str) -> Option<DownloadType> {
    let chain: Vec<Box<dyn DownloadTypeDetector>> = vec![
        Box::new(MagnetDetector),
        Box::new(TorrentDetector),
        Box::new(HttpDetector),
    ];
    chain.iter().find_map(|d| d.detect(url))
}
```

**順序很重要**：MagnetDetector → TorrentDetector → HttpDetector（從最嚴格到最寬鬆）。

### Integration Point

在 `receive_raw_fetcher_results` 處理流程中，建立 `anime_link` 時呼叫 `detect_download_type(url)` 填入 `download_type` 欄位。

---

## 3. Downloader Registration & Capabilities

### Capabilities Extension

```rust
pub struct Capabilities {
    pub fetch_endpoint: Option<String>,
    pub download_endpoint: Option<String>,
    pub sync_endpoint: Option<String>,
    pub supported_download_types: Vec<DownloadType>,  // NEW
}
```

### Registration Flow

1. Downloader 啟動時呼叫 `POST /services/register`，payload 包含 `supported_download_types`
2. Core 的 `register()` handler 除了現有 UPSERT `service_modules` 外，同步寫入 `downloader_capabilities`：
   - 先 DELETE 該 module_id 的舊 capabilities
   - 再 INSERT 新的 capabilities

### Query Pattern

查詢支援特定 download_type 的 downloader，按優先權排序：

```sql
SELECT sm.*
FROM service_modules sm
JOIN downloader_capabilities dc ON sm.module_id = dc.module_id
WHERE dc.download_type = $1
  AND sm.is_enabled = true
ORDER BY sm.priority DESC;
```

### Auto-Recovery Trigger

當新 Downloader 註冊時，檢查是否有 `no_downloader` 狀態的下載連結可以被處理：

```rust
// 在 register() handler 最後：
if payload.service_type == ServiceType::Downloader {
    dispatch_service.retry_no_downloader_links(&new_capabilities).await;
}
```

---

## 4. Batch Download Dispatch Pipeline

### Trigger

`receive_raw_fetcher_results` 完成所有 raw items 處理後，呼叫批次派發。

### Pipeline Steps

```
1. 收集所有新建的 anime_links（本次 batch）
2. 按 download_type 分組
3. 對每個 download_type：
   a. 查詢支援的 downloaders（按 priority DESC）
   b. 對最高優先權的 downloader 發送批次請求
   c. 處理回傳結果：
      - accepted → 建立 downloads 記錄（status: downloading）
      - rejected → 放入「待重試」清單
   d. 「待重試」清單發送給下一優先權的 downloader
   e. 重複直到所有連結都被處理或沒有更多 downloader
   f. 剩餘未處理的連結 → 建立 downloads 記錄（status: no_downloader）
```

### Downloader Batch API

```
POST /downloads
Content-Type: application/json

{
    "items": [
        { "url": "magnet:?xt=urn:btih:...", "save_path": "/downloads/anime-name" },
        { "url": "magnet:?xt=urn:btih:...", "save_path": "/downloads/anime-name" }
    ]
}

Response 200:
{
    "results": [
        { "url": "magnet:...", "hash": "abc123", "status": "accepted" },
        { "url": "magnet:...", "hash": null, "status": "rejected", "reason": "unsupported" }
    ]
}
```

### Core Side Data Flow

```rust
// 建立 downloads 記錄
for result in batch_results {
    match result.status {
        "accepted" => {
            create_download(link_id, module_id, "downloading", result.hash);
        }
        "rejected" => {
            // 加入下一個 downloader 的待處理清單
            retry_queue.push(link_id);
        }
    }
}
```

---

## 5. Filter Change Handling

當使用者修改 filter rules 時，需要重新評估受影響的下載連結。

### Scope

- 只評估目標 `target_type` 範圍內的連結
- 排除已完成下載（`completed`）的連結

### Flow

```
Filter rule changed (target_type = X)
    ↓
重新載入 target_type = X 的 filter rules
    ↓
對 anime_links WHERE target matches X AND NOT EXISTS completed download：
    ↓
┌─────────────────────────────────────────┐
│ 原本 filtered=false, 現在 filtered=true │ → 取消下載（如有進行中）
│ 原本 filtered=true,  現在 filtered=false│ → 派發下載
│ 無變化                                   │ → 不動作
└─────────────────────────────────────────┘
```

### Cancel Flow

```
POST /downloads/cancel
Content-Type: application/json

{
    "hashes": ["abc123", "def456"]
}

Response 200:
{
    "results": [
        { "hash": "abc123", "status": "cancelled" },
        { "hash": "def456", "status": "not_found" }
    ]
}
```

Core 收到取消結果後，將 downloads 記錄更新為 `cancelled`。

---

## 6. DownloadScheduler — Status Polling

### Design

仿照 `FetchScheduler` 設計，定期輪詢所有 Downloader 取得下載狀態更新。

### Configuration

```
DOWNLOAD_POLL_INTERVAL=60  # 環境變數，單位秒，預設 60
```

### Poll Flow

```
每 N 秒：
    ↓
取得所有 is_enabled=true 的 downloader modules
    ↓
對每個 downloader：
    ├─ GET /downloads?hashes=hash1,hash2,...
    │  （只查詢 status=downloading 的 hash）
    │
    ├─ 成功 → 更新 downloads 表：
    │   - progress, downloaded_bytes
    │   - 若 downloader 回報 completed → status = completed
    │   - 若 downloader 回報 error → status = failed
    │
    └─ 連線失敗 → 將該 downloader 的所有 downloading 記錄標為 downloader_error
```

### Downloader Status API

```
GET /downloads?hashes=hash1,hash2,...

Response 200:
{
    "statuses": [
        { "hash": "abc123", "status": "downloading", "progress": 0.45, "size": 1073741824 },
        { "hash": "def456", "status": "completed", "progress": 1.0, "size": 524288000 }
    ]
}
```

### Downloader Error Recovery

當 downloader 恢復連線（下次 poll 成功）時：
- 查詢所有 `downloader_error` 狀態且 `module_id` 為該 downloader 的記錄
- 用 `query_status` 確認實際狀態
- 更新為正確狀態（downloading / completed / failed）

---

## 7. Downloader API & Trait

### Revised DownloaderClient Trait

移除所有過時方法，僅保留實際需求的 API：

```rust
#[async_trait]
pub trait DownloaderClient: Send + Sync {
    /// 登入 downloader
    async fn login(&self, username: &str, password: &str) -> Result<()>;

    /// 批次新增下載任務
    async fn add_torrents(&self, items: Vec<DownloadItem>) -> Result<Vec<DownloadItemResult>>;

    /// 批次取消下載任務
    async fn cancel_torrents(&self, hashes: Vec<String>) -> Result<Vec<CancelResult>>;

    /// 批次查詢下載狀態
    async fn query_status(&self, hashes: Vec<String>) -> Result<Vec<DownloadStatus>>;

    /// 暫停下載
    async fn pause_torrent(&self, hash: &str) -> Result<()>;

    /// 恢復下載
    async fn resume_torrent(&self, hash: &str) -> Result<()>;

    /// 刪除下載（可選刪除檔案）
    async fn delete_torrent(&self, hash: &str, delete_files: bool) -> Result<()>;
}
```

### 移除的方法

以下舊方法將被移除，不為了測試相容性而保留：

- `add_magnet` → 被 `add_torrents` 取代
- `add_torrent` → 被 `add_torrents` 取代
- `get_torrent_info` → 被 `query_status` 取代
- `get_all_torrents` → 被 `query_status` 取代
- `extract_hash_from_magnet` → 內部使用，不暴露在 trait 上
- `extract_hash_from_url` → 內部使用，不暴露在 trait 上

### Data Types

```rust
pub struct DownloadItem {
    pub url: String,
    pub save_path: String,
}

pub struct DownloadItemResult {
    pub url: String,
    pub hash: Option<String>,
    pub status: DownloadItemStatus, // Accepted, Rejected
    pub reason: Option<String>,
}

pub struct CancelResult {
    pub hash: String,
    pub status: CancelStatus, // Cancelled, NotFound
}

pub struct DownloadStatus {
    pub hash: String,
    pub status: TorrentState, // Downloading, Completed, Error, ...
    pub progress: f64,
    pub size: u64,
}
```

### RESTful Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/downloads` | 批次新增下載任務 |
| `GET` | `/downloads` | 批次查詢下載狀態（`?hashes=...`） |
| `POST` | `/downloads/cancel` | 批次取消下載 |
| `POST` | `/downloads/:hash/pause` | 暫停單一下載 |
| `POST` | `/downloads/:hash/resume` | 恢復單一下載 |
| `DELETE` | `/downloads/:hash` | 刪除下載（`?delete_files=true`） |

所有測試必須依據新的 trait 和 API 重寫。

---

## 8. Complete Data Flow & Error Handling

### End-to-End Flow

```
Fetcher (mikanani)
    │
    │ POST /fetcher-results
    ▼
Core: receive_raw_fetcher_results()
    │
    ├─ 1. 儲存 raw_anime_items
    ├─ 2. TitleParser 解析標題
    ├─ 3. 建立 anime_links
    │      └─ detect_download_type(url) → 填入 download_type
    ├─ 4. FilterEngine 過濾
    │
    └─ 5. 批次派發（本次新增的未過濾連結）
           │
           ├─ 按 download_type 分組
           ├─ 查詢 downloader_capabilities + priority
           ├─ Cascade: 最高優先 → 次高 → ...
           │
           ▼
       Downloader (qbittorrent)
           │ POST /downloads
           ▼
       Core 建立 downloads 記錄
           │
           │ (每 60 秒)
           ▼
       DownloadScheduler
           │ GET /downloads?hashes=...
           ▼
       更新 downloads 狀態
```

### Error Handling Matrix

| Scenario | Action | downloads.status |
|----------|--------|------------------|
| Downloader 接受連結 | 建立記錄 | `downloading` |
| Downloader 拒絕連結 | Cascade 到下一個 downloader | — |
| 所有 Downloader 都拒絕 | 建立記錄 | `failed` |
| 沒有支援該類型的 Downloader | 建立記錄 | `no_downloader` |
| 新 Downloader 註冊 | 重新派發 `no_downloader` 連結 | → `downloading` |
| Downloader 回報完成 | 更新記錄 | `completed` |
| Downloader 回報錯誤 | 更新記錄 | `failed` |
| 輪詢時 Downloader 離線 | 標記該 downloader 的所有 downloading | `downloader_error` |
| Downloader 恢復連線 | 重新查詢實際狀態並更新 | → 實際狀態 |
| Filter 變更：新增過濾 | 取消進行中的下載 | `cancelled` |
| Filter 變更：移除過濾 | 派發新的下載 | `downloading` |
| 派發時網路錯誤 | Cascade 到下一個 downloader | — |

### Status State Machine

```
                    ┌──────────────┐
                    │   pending    │
                    └──────┬───────┘
                           │ dispatch
              ┌────────────┼────────────┐
              ▼            ▼            ▼
    ┌──────────────┐ ┌───────────┐ ┌──────────────┐
    │ downloading  │ │  failed   │ │ no_downloader│
    └──────┬───────┘ └───────────┘ └──────┬───────┘
           │                              │
     ┌─────┼─────┐                 new downloader
     ▼     ▼     ▼                 registers
┌─────┐ ┌────┐ ┌────────────────┐     │
│done │ │fail│ │downloader_error│     │
└─────┘ └────┘ └───────┬────────┘     │
                        │              │
                   recover             │
                     ┌─────────────────┘
                     ▼
              ┌──────────────┐
              │ downloading  │
              └──────────────┘

  任何非 completed 狀態 ──filter──→ cancelled
```
