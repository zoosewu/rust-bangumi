# Torrent File Extraction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 讓系統能處理包含影片和多個字幕的 torrent，正確識別各檔案類型並傳送給 viewer 處理。

**Architecture:** Downloader service 回報 torrent 內所有檔案的完整路徑；`shared` crate 提供檔案分類函式（依副檔名）和語言代碼對照表；Core 分類後儲存到 DB，送出結構化的 `ViewerSyncRequest`；Viewer 依分類結果分別搬移影片和字幕。

**Tech Stack:** Rust, Diesel (PostgreSQL), serde/serde_json, tokio::fs, axum

---

## 變更範圍總覽

| 檔案 | 變更類型 |
|------|---------|
| `shared/src/file_classifier.rs` | 新建 |
| `shared/assets/language_codes.json` | 新建 |
| `shared/src/lib.rs` | 加入 `pub mod file_classifier` |
| `shared/src/models.rs` | 修改 `DownloadStatusItem`, `ViewerSyncRequest` |
| `shared/Cargo.toml` | 加入 `serde_json` |
| `core-service/migrations/2026-02-26-000000-add-download-file-fields/` | 新建 migration |
| `core-service/src/schema.rs` | 加入 `video_file`, `subtitle_files` |
| `core-service/src/models/db.rs` | 更新 `Download` struct |
| `core-service/src/services/download_scheduler.rs` | 呼叫 classify，儲存到 DB |
| `core-service/src/services/sync_service.rs` | 用新欄位建 `ViewerSyncRequest` |
| `downloaders/qbittorrent/src/qbittorrent_client.rs` | query_status 回報檔案列表 |
| `downloaders/qbittorrent/src/mock.rs` | 更新 DownloadStatusItem |
| `viewers/jellyfin/src/file_organizer.rs` | 新增字幕搬移邏輯 |
| `viewers/jellyfin/src/handlers.rs` | 用 `video_path`, `subtitle_paths` |
| `viewers/jellyfin/src/main.rs` | 載入 `LanguageCodeMap` |
| `Dockerfile.viewer-jellyfin` | COPY language_codes.json |
| `docker-compose.yaml` | 加入 `LANGUAGE_CODES_PATH` env |

---

## Task 1: shared — 新建 file_classifier 模組

**Files:**
- Create: `shared/src/file_classifier.rs`
- Modify: `shared/src/lib.rs`
- Modify: `shared/Cargo.toml`

### Step 1: 確認 shared/Cargo.toml 是否有 serde_json

```bash
grep -n "serde_json" shared/Cargo.toml
```

若沒有，在 `[dependencies]` 區塊加入：

```toml
serde_json = "1"
```

### Step 2: 撰寫失敗測試

在 `shared/src/file_classifier.rs` 開頭加入測試：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_video_extensions() {
        let files = vec![
            "/downloads/video.mkv".to_string(),
            "/downloads/video.mp4".to_string(),
            "/downloads/video.avi".to_string(),
            "/downloads/video.ts".to_string(),
        ];
        let result = classify_files(files);
        assert!(result.iter().all(|f| f.file_type == FileType::Video));
    }

    #[test]
    fn test_classify_subtitle_extensions() {
        let files = vec![
            "/downloads/sub.ass".to_string(),
            "/downloads/sub.ssa".to_string(),
            "/downloads/sub.srt".to_string(),
            "/downloads/sub.vtt".to_string(),
        ];
        let result = classify_files(files);
        assert!(result.iter().all(|f| f.file_type == FileType::Subtitle));
    }

    #[test]
    fn test_classify_other_extensions() {
        let files = vec![
            "/downloads/thumb.jpg".to_string(),
            "/downloads/readme.txt".to_string(),
        ];
        let result = classify_files(files);
        assert!(result.iter().all(|f| f.file_type == FileType::Other));
    }

    #[test]
    fn test_classify_mixed_files() {
        let files = vec![
            "/downloads/video.mkv".to_string(),
            "/downloads/sub.TC.ass".to_string(),
            "/downloads/sub.SC.ass".to_string(),
            "/downloads/thumb.jpg".to_string(),
        ];
        let result = classify_files(files);
        assert_eq!(result[0].file_type, FileType::Video);
        assert_eq!(result[1].file_type, FileType::Subtitle);
        assert_eq!(result[2].file_type, FileType::Subtitle);
        assert_eq!(result[3].file_type, FileType::Other);
    }

    #[test]
    fn test_collect_files_single_file() {
        // collect_files_recursive on a non-directory path returns empty
        // (actual filesystem tests handled in integration)
        let result = classify_files(vec!["/downloads/video.mkv".to_string()]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_language_code_map_normalize_known() {
        let map = LanguageCodeMap::from_entries(vec![
            ("TC".to_string(), "zh-TW".to_string()),
            ("SC".to_string(), "zh-CN".to_string()),
        ]);
        assert_eq!(map.normalize("TC"), "zh-TW");
        assert_eq!(map.normalize("SC"), "zh-CN");
    }

    #[test]
    fn test_language_code_map_normalize_unknown() {
        let map = LanguageCodeMap::from_entries(vec![]);
        assert_eq!(map.normalize("XX"), "XX");
    }

    #[test]
    fn test_language_code_map_case_insensitive() {
        let map = LanguageCodeMap::from_entries(vec![
            ("TC".to_string(), "zh-TW".to_string()),
        ]);
        assert_eq!(map.normalize("tc"), "zh-TW");
    }

    #[test]
    fn test_extract_language_tag_dotted_stem() {
        // "sub.TC.ass" → Some("TC")
        assert_eq!(extract_language_tag("/downloads/sub.TC.ass"), Some("TC".to_string()));
    }

    #[test]
    fn test_extract_language_tag_simple_stem() {
        // "subtitle.ass" → None (only one segment)
        assert_eq!(extract_language_tag("/downloads/subtitle.ass"), None);
    }

    #[test]
    fn test_extract_language_tag_nested() {
        // "subtitle.CHS.srt" → Some("CHS")
        assert_eq!(extract_language_tag("/downloads/subtitle.CHS.srt"), Some("CHS".to_string()));
    }
}
```

### Step 3: 執行測試確認失敗

```bash
cargo test -p shared --lib file_classifier 2>&1 | tail -20
```

預期：編譯錯誤（模組不存在）

### Step 4: 實作 file_classifier.rs

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    Video,
    Subtitle,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub path: String,
    pub file_type: FileType,
}

const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "ts", "m2ts", "mov", "wmv", "flv", "webm"];
const SUBTITLE_EXTENSIONS: &[&str] = &["ass", "ssa", "srt", "vtt", "sup", "sub", "idx"];

pub fn classify_files(files: Vec<String>) -> Vec<MediaFile> {
    files.into_iter().map(|path| {
        let ext = Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let file_type = if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
            FileType::Video
        } else if SUBTITLE_EXTENSIONS.contains(&ext.as_str()) {
            FileType::Subtitle
        } else {
            FileType::Other
        };

        MediaFile { path, file_type }
    }).collect()
}

/// 從字幕檔路徑中提取語言標記。
/// 例如 "/downloads/sub.TC.ass" → Some("TC")
/// 例如 "/downloads/subtitle.ass" → None
pub fn extract_language_tag(path: &str) -> Option<String> {
    let stem = Path::new(path).file_stem()?.to_str()?;
    let parts: Vec<&str> = stem.split('.').collect();
    if parts.len() >= 2 {
        Some(parts.last()?.to_string())
    } else {
        None
    }
}

/// 遞迴收集目錄下所有檔案的完整路徑。
/// 若 path 是檔案，直接回傳 vec![path]。
/// 供 downloader service 使用。
pub fn collect_files_recursive(path: &Path) -> Vec<String> {
    if path.is_file() {
        return path.to_str()
            .map(|s| vec![s.to_string()])
            .unwrap_or_default();
    }
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_file() {
                if let Some(s) = child.to_str() {
                    files.push(s.to_string());
                }
            } else if child.is_dir() {
                files.extend(collect_files_recursive(&child));
            }
        }
    }
    files
}

#[derive(Debug, Clone)]
pub struct LanguageCodeMap(HashMap<String, String>);

impl LanguageCodeMap {
    pub fn from_entries(entries: Vec<(String, String)>) -> Self {
        Self(entries.into_iter().collect())
    }

    /// JSON 格式：{ "TC": "zh-TW", "SC": "zh-CN", ... }
    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let map: HashMap<String, String> = serde_json::from_str(&content)?;
        // 全部 key 轉大寫，方便 case-insensitive 查找
        let normalized = map.into_iter()
            .map(|(k, v)| (k.to_uppercase(), v))
            .collect();
        Ok(Self(normalized))
    }

    /// 查找語言代碼，找不到則回傳原始 tag（大小寫不敏感）。
    pub fn normalize(&self, tag: &str) -> String {
        self.0.get(&tag.to_uppercase())
            .cloned()
            .unwrap_or_else(|| tag.to_string())
    }
}

#[cfg(test)]
mod tests {
    // （測試已在上方 Step 2 中定義）
    use super::*;

    #[test]
    fn test_classify_video_extensions() {
        let files = vec![
            "/downloads/video.mkv".to_string(),
            "/downloads/video.mp4".to_string(),
            "/downloads/video.avi".to_string(),
            "/downloads/video.ts".to_string(),
        ];
        let result = classify_files(files);
        assert!(result.iter().all(|f| f.file_type == FileType::Video));
    }

    #[test]
    fn test_classify_subtitle_extensions() {
        let files = vec![
            "/downloads/sub.ass".to_string(),
            "/downloads/sub.ssa".to_string(),
            "/downloads/sub.srt".to_string(),
            "/downloads/sub.vtt".to_string(),
        ];
        let result = classify_files(files);
        assert!(result.iter().all(|f| f.file_type == FileType::Subtitle));
    }

    #[test]
    fn test_classify_other_extensions() {
        let files = vec![
            "/downloads/thumb.jpg".to_string(),
            "/downloads/readme.txt".to_string(),
        ];
        let result = classify_files(files);
        assert!(result.iter().all(|f| f.file_type == FileType::Other));
    }

    #[test]
    fn test_classify_mixed_files() {
        let files = vec![
            "/downloads/video.mkv".to_string(),
            "/downloads/sub.TC.ass".to_string(),
            "/downloads/sub.SC.ass".to_string(),
            "/downloads/thumb.jpg".to_string(),
        ];
        let result = classify_files(files);
        assert_eq!(result[0].file_type, FileType::Video);
        assert_eq!(result[1].file_type, FileType::Subtitle);
        assert_eq!(result[2].file_type, FileType::Subtitle);
        assert_eq!(result[3].file_type, FileType::Other);
    }

    #[test]
    fn test_language_code_map_normalize_known() {
        let map = LanguageCodeMap::from_entries(vec![
            ("TC".to_string(), "zh-TW".to_string()),
            ("SC".to_string(), "zh-CN".to_string()),
        ]);
        assert_eq!(map.normalize("TC"), "zh-TW");
        assert_eq!(map.normalize("SC"), "zh-CN");
    }

    #[test]
    fn test_language_code_map_normalize_unknown() {
        let map = LanguageCodeMap::from_entries(vec![]);
        assert_eq!(map.normalize("XX"), "XX");
    }

    #[test]
    fn test_language_code_map_case_insensitive() {
        let map = LanguageCodeMap::from_entries(vec![
            ("TC".to_string(), "zh-TW".to_string()),
        ]);
        assert_eq!(map.normalize("tc"), "zh-TW");
    }

    #[test]
    fn test_extract_language_tag_dotted_stem() {
        assert_eq!(extract_language_tag("/downloads/sub.TC.ass"), Some("TC".to_string()));
    }

    #[test]
    fn test_extract_language_tag_simple_stem() {
        assert_eq!(extract_language_tag("/downloads/subtitle.ass"), None);
    }

    #[test]
    fn test_extract_language_tag_nested() {
        assert_eq!(extract_language_tag("/downloads/subtitle.CHS.srt"), Some("CHS".to_string()));
    }
}
```

### Step 5: 在 shared/src/lib.rs 加入模組宣告

在 `shared/src/lib.rs` 加入：
```rust
pub mod file_classifier;
pub use file_classifier::{
    classify_files, collect_files_recursive, extract_language_tag,
    FileType, MediaFile, LanguageCodeMap,
};
```

### Step 6: 執行測試確認通過

```bash
cargo test -p shared 2>&1 | tail -30
```

預期：所有 `file_classifier` 測試通過

### Step 7: Commit

```bash
git add shared/src/file_classifier.rs shared/src/lib.rs shared/Cargo.toml
git commit -m "feat(shared): add file_classifier module with classify_files and LanguageCodeMap"
```

---

## Task 2: 建立 language_codes.json

**Files:**
- Create: `shared/assets/language_codes.json`

### Step 1: 建立 assets 目錄和 JSON 檔

```bash
mkdir -p shared/assets
```

建立 `shared/assets/language_codes.json`：

```json
{
  "TC":  "zh-TW",
  "CHT": "zh-TW",
  "BIG5": "zh-TW",
  "SC":  "zh-CN",
  "CHS": "zh-CN",
  "GB":  "zh-CN",
  "JP":  "ja",
  "JPN": "ja",
  "EN":  "en",
  "ENG": "en",
  "KR":  "ko",
  "KOR": "ko"
}
```

### Step 2: Commit

```bash
git add shared/assets/language_codes.json
git commit -m "feat(shared): add language_codes.json for subtitle language tag normalization"
```

---

## Task 3: 更新 shared models — DownloadStatusItem 加入 files 欄位

**Files:**
- Modify: `shared/src/models.rs`
- Modify: `downloaders/qbittorrent/src/qbittorrent_client.rs`
- Modify: `downloaders/qbittorrent/src/mock.rs`

### Step 1: 讀取目前的 DownloadStatusItem 定義

```bash
grep -n "DownloadStatusItem" shared/src/models.rs
```

確認目前結構體位置。

### Step 2: 修改 DownloadStatusItem

在 `shared/src/models.rs` 的 `DownloadStatusItem` struct，加入 `files` 欄位：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStatusItem {
    pub hash: String,
    pub status: String,
    pub progress: f64,
    pub size: u64,
    pub content_path: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,  // 新增：torrent 內所有檔案的完整路徑
}
```

`#[serde(default)]` 確保舊版 downloader 回應（無此欄位）仍可反序列化。

### Step 3: 修復 qbittorrent_client.rs 的建構子

在 `downloaders/qbittorrent/src/qbittorrent_client.rs` 中，找到建構 `DownloadStatusItem` 的地方（在 `query_status` 方法），補上 `files: vec![]`（暫時；Task 7 會填入真實值）：

```rust
DownloadStatusItem {
    hash: info.hash.clone(),
    status: status_str.to_string(),
    progress: info.progress,
    size: info.size as u64,
    content_path: info.content_path.clone(),
    files: vec![],  // 暫時：Task 7 填入
}
```

### Step 4: 修復 mock.rs

在 `mock.rs` 的所有 `DownloadStatusItem { ... }` literal，補上 `files: vec![]`。

### Step 5: 確認編譯

```bash
cargo build -p downloader-qbittorrent 2>&1 | grep -E "^error"
```

預期：無 error

### Step 6: Commit

```bash
git add shared/src/models.rs downloaders/qbittorrent/src/qbittorrent_client.rs downloaders/qbittorrent/src/mock.rs
git commit -m "feat(shared): add files field to DownloadStatusItem"
```

---

## Task 4: 更新 shared models — ViewerSyncRequest 改版

**Files:**
- Modify: `shared/src/models.rs`
- Modify: `core-service/src/services/sync_service.rs`
- Modify: `viewers/jellyfin/src/handlers.rs`

### Step 1: 修改 ViewerSyncRequest

在 `shared/src/models.rs`，將 `file_path: String` 替換為 `video_path` 和 `subtitle_paths`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerSyncRequest {
    pub download_id: i32,
    pub series_id: i32,
    pub anime_title: String,
    pub series_no: i32,
    pub episode_no: i32,
    pub subtitle_group: String,
    pub video_path: String,           // 原 file_path，現在是已分類的影片路徑
    pub subtitle_paths: Vec<String>,  // 所有字幕檔的完整路徑
    pub callback_url: String,
    pub bangumi_id: Option<i32>,
    pub cover_image_url: Option<String>,
}
```

### Step 2: 修復 sync_service.rs 中的 build_sync_request

在 `core-service/src/services/sync_service.rs` 的 `build_sync_request()` 函式，找到建構 `ViewerSyncRequest` 的地方，暫時用 `file_path` 作為 `video_path`，`subtitle_paths` 先給空陣列（Task 9 會用 DB 新欄位正確填入）：

```rust
ViewerSyncRequest {
    download_id: download.download_id,
    series_id: link.anime_id,
    anime_title: work.title.clone(),
    series_no: anime.series_no,
    episode_no: link.episode_no,
    subtitle_group: group.group_name.clone(),
    video_path: download.file_path.clone().unwrap_or_default(),  // 暫時
    subtitle_paths: vec![],  // Task 9 會填入 download.subtitle_files
    callback_url: format!("{}/sync-callback", self.core_service_url),
    bangumi_id: None,
    cover_image_url: None,
}
```

### Step 3: 修復 viewers/jellyfin/src/handlers.rs

在 `handlers.rs` 的 `do_sync` 函式，將 `&req.file_path` 改為 `&req.video_path`（line 151）：

```rust
// 修改前：
let source = organizer.resolve_download_path(&req.file_path);

// 修改後：
let source = organizer.resolve_download_path(&req.video_path);
```

字幕搬移邏輯在 Task 10 加入，此處先只改這一行確保編譯通過。

### Step 4: 確認全部編譯

```bash
cargo build -p core-service -p viewer-jellyfin 2>&1 | grep -E "^error"
```

預期：無 error

### Step 5: Commit

```bash
git add shared/src/models.rs core-service/src/services/sync_service.rs viewers/jellyfin/src/handlers.rs
git commit -m "feat(shared): replace file_path with video_path+subtitle_paths in ViewerSyncRequest"
```

---

## Task 5: Core DB migration — 新增 video_file, subtitle_files

**Files:**
- Create: `core-service/migrations/2026-02-26-000000-add-download-file-fields/up.sql`
- Create: `core-service/migrations/2026-02-26-000000-add-download-file-fields/down.sql`

### Step 1: 建立 migration 目錄和 up.sql

```bash
mkdir -p core-service/migrations/2026-02-26-000000-add-download-file-fields
```

`up.sql`：
```sql
ALTER TABLE downloads
    ADD COLUMN video_file TEXT,
    ADD COLUMN subtitle_files TEXT;

COMMENT ON COLUMN downloads.video_file IS '已分類的影片檔案絕對路徑';
COMMENT ON COLUMN downloads.subtitle_files IS '已分類的字幕檔案路徑清單（JSON 字串陣列）';
```

`down.sql`：
```sql
ALTER TABLE downloads
    DROP COLUMN IF EXISTS video_file,
    DROP COLUMN IF EXISTS subtitle_files;
```

### Step 2: 套用 migration（確保 Postgres 在執行）

```bash
cd core-service && diesel migration run && cd ..
```

預期輸出：`Running migration 2026-02-26-000000-add-download-file-fields`

### Step 3: Commit

```bash
git add core-service/migrations/2026-02-26-000000-add-download-file-fields/
git commit -m "feat(core): add video_file and subtitle_files columns to downloads"
```

---

## Task 6: 更新 Core schema.rs 和 db.rs

**Files:**
- Modify: `core-service/src/schema.rs`
- Modify: `core-service/src/models/db.rs`

### Step 1: 更新 schema.rs

在 `schema.rs` 的 `downloads` table 定義加入兩個新欄位（在 `file_path` 和 `sync_retry_count` 之間）：

```rust
diesel::table! {
    downloads (download_id) {
        // ... 現有欄位 ...
        file_path -> Nullable<Text>,
        video_file -> Nullable<Text>,       // 新增
        subtitle_files -> Nullable<Text>,   // 新增（JSON 字串）
        sync_retry_count -> Int4,
    }
}
```

> **注意：** 欄位順序必須與資料庫一致。可執行 `diesel print-schema` 確認正確順序：
> ```bash
> cd core-service && diesel print-schema 2>&1 | grep -A 20 "downloads ("
> ```

### Step 2: 更新 db.rs 的 Download struct

在 `core-service/src/models/db.rs` 的 `Download` struct 加入兩個新欄位：

```rust
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::downloads)]
pub struct Download {
    pub download_id: i32,
    pub link_id: i32,
    pub downloader_type: String,
    pub status: String,
    pub progress: Option<f32>,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub module_id: Option<i32>,
    pub torrent_hash: Option<String>,
    pub file_path: Option<String>,
    pub video_file: Option<String>,       // 新增
    pub subtitle_files: Option<String>,   // 新增（JSON 字串）
    pub sync_retry_count: i32,
}
```

### Step 3: 確認編譯

```bash
cargo build -p core-service 2>&1 | grep -E "^error"
```

預期：無 error

### Step 4: Commit

```bash
git add core-service/src/schema.rs core-service/src/models/db.rs
git commit -m "feat(core): update schema and Download struct for video_file/subtitle_files"
```

---

## Task 7: downloader-qbittorrent — 回報檔案列表

**Files:**
- Modify: `downloaders/qbittorrent/src/qbittorrent_client.rs`

### Step 1: 了解現有 query_status 結構

```bash
grep -n "content_path\|DownloadStatusItem\|TorrentInfo" downloaders/qbittorrent/src/qbittorrent_client.rs | head -30
```

### Step 2: 撰寫失敗測試（加在 mock.rs 的測試區塊）

在 `downloaders/qbittorrent/src/mock.rs`，確認 mock 的 `query_status_result` 測試資料包含 `files` 欄位：

```rust
// 確認現有測試（或新增）：
let status = DownloadStatusItem {
    hash: "abc123".to_string(),
    status: "completed".to_string(),
    progress: 1.0,
    size: 1024,
    content_path: Some("/downloads/video.mkv".to_string()),
    files: vec!["/downloads/video.mkv".to_string()],  // 確認有此欄位
};
```

### Step 3: 修改 query_status 填入 files

在 `qbittorrent_client.rs` 的 `query_status` 方法中，找到建構 `DownloadStatusItem` 的地方，替換為：

```rust
// 用 content_path 建立檔案列表
let files = match &info.content_path {
    Some(path) => {
        let p = std::path::Path::new(path);
        shared::collect_files_recursive(p)
    }
    None => vec![],
};

DownloadStatusItem {
    hash: info.hash.clone(),
    status: status_str.to_string(),
    progress: info.progress,
    size: info.size as u64,
    content_path: info.content_path.clone(),
    files,
}
```

> **說明：** `collect_files_recursive` 來自 `shared` crate。若 `content_path` 是單一檔案，直接回傳 `vec![該路徑]`；若是目錄則遞迴列出所有檔案。downloader 容器已 mount `/downloads`，所以可直接存取。

### Step 4: 確認編譯

```bash
cargo build -p downloader-qbittorrent 2>&1 | grep -E "^error"
```

### Step 5: Commit

```bash
git add downloaders/qbittorrent/src/qbittorrent_client.rs
git commit -m "feat(downloader-qbt): populate files list in query_status via filesystem scan"
```

---

## Task 8: Core download_scheduler — 分類並儲存檔案資訊

**Files:**
- Modify: `core-service/src/services/download_scheduler.rs`

### Step 1: 了解現有 "completed" 處理邏輯

讀取並定位 `poll_downloader` 中更新 downloads 狀態為 "completed" 的程式碼：

```bash
grep -n "completed\|file_path\|diesel::update" core-service/src/services/download_scheduler.rs | head -30
```

### Step 2: 在 completed 處理區塊加入分類邏輯

找到設定 `status = "completed"` 和 `file_path = content_path` 的 `diesel::update` 呼叫，在其中加入 `video_file` 和 `subtitle_files`：

```rust
use shared::{classify_files, FileType};

// 在判斷 status == "completed" 的區塊中：
let (video_file, subtitle_files_json) = if !item.files.is_empty() {
    let classified = classify_files(item.files.clone());
    let video = classified.iter()
        .find(|f| f.file_type == FileType::Video)
        .map(|f| f.path.clone());
    let subtitles: Vec<String> = classified.iter()
        .filter(|f| f.file_type == FileType::Subtitle)
        .map(|f| f.path.clone())
        .collect();
    let subtitle_json = if subtitles.is_empty() {
        None
    } else {
        serde_json::to_string(&subtitles).ok()
    };
    (video, subtitle_json)
} else {
    (None, None)
};

// 加入到 diesel::update 的 set() 呼叫：
diesel::update(downloads::table.filter(downloads::download_id.eq(download_id)))
    .set((
        downloads::status.eq("completed"),
        downloads::file_path.eq(content_path.as_deref()),
        downloads::video_file.eq(video_file.as_deref()),
        downloads::subtitle_files.eq(subtitle_files_json.as_deref()),
        downloads::updated_at.eq(now),
    ))
    .execute(&mut conn)?;
```

> **注意：** 需要在 `core-service/Cargo.toml` 確認有 `serde_json`：
> ```bash
> grep "serde_json" core-service/Cargo.toml
> ```

### Step 3: 確認編譯

```bash
cargo build -p core-service 2>&1 | grep -E "^error"
```

### Step 4: Commit

```bash
git add core-service/src/services/download_scheduler.rs
git commit -m "feat(core): classify files and store video_file/subtitle_files on download completion"
```

---

## Task 9: Core sync_service — 用 DB 新欄位建 ViewerSyncRequest

**Files:**
- Modify: `core-service/src/services/sync_service.rs`

### Step 1: 找到 build_sync_request 函式

```bash
grep -n "build_sync_request\|video_path\|file_path\|subtitle" core-service/src/services/sync_service.rs | head -20
```

### Step 2: 更新 build_sync_request

將 Task 4 中的暫時 `video_path: download.file_path.clone().unwrap_or_default()` 替換為使用 DB 新欄位：

```rust
// 解析 subtitle_files JSON 字串
let subtitle_paths: Vec<String> = download.subtitle_files
    .as_deref()
    .and_then(|s| serde_json::from_str(s).ok())
    .unwrap_or_default();

ViewerSyncRequest {
    download_id: download.download_id,
    series_id: link.anime_id,
    anime_title: work.title.clone(),
    series_no: anime.series_no,
    episode_no: link.episode_no,
    subtitle_group: group.group_name.clone(),
    video_path: download.video_file
        .clone()
        .unwrap_or_else(|| download.file_path.clone().unwrap_or_default()),
    subtitle_paths,
    callback_url: format!("{}/sync-callback", self.core_service_url),
    bangumi_id: None,
    cover_image_url: None,
}
```

> **說明：** `video_file` 優先；若為 None（舊資料無此欄位），fallback 到 `file_path`，確保向後相容。

### Step 3: 確認編譯

```bash
cargo build -p core-service 2>&1 | grep -E "^error"
```

### Step 4: Commit

```bash
git add core-service/src/services/sync_service.rs
git commit -m "feat(core): use video_file/subtitle_files from DB in ViewerSyncRequest"
```

---

## Task 10: viewer-jellyfin — 字幕搬移邏輯

**Files:**
- Modify: `viewers/jellyfin/src/file_organizer.rs`

### Step 1: 撰寫失敗測試

在 `viewers/jellyfin/src/file_organizer.rs` 的 `mod tests` 區塊加入：

```rust
#[test]
fn test_build_subtitle_dest_name_with_lang() {
    // "sub.TC.ass" → "Title - S01E01.zh-TW.ass"
    let map = shared::LanguageCodeMap::from_entries(vec![
        ("TC".to_string(), "zh-TW".to_string()),
    ]);
    let name = FileOrganizer::build_subtitle_dest_name(
        "Title", 1, 1, "/downloads/sub.TC.ass", &map
    );
    assert_eq!(name, "Title - S01E01.zh-TW.ass");
}

#[test]
fn test_build_subtitle_dest_name_no_lang() {
    // "subtitle.ass" → "Title - S01E01.ass"
    let map = shared::LanguageCodeMap::from_entries(vec![]);
    let name = FileOrganizer::build_subtitle_dest_name(
        "Title", 1, 1, "/downloads/subtitle.ass", &map
    );
    assert_eq!(name, "Title - S01E01.ass");
}

#[test]
fn test_build_subtitle_dest_name_unknown_lang() {
    // "sub.XX.srt" → "Title - S01E01.XX.srt"（保留原始 tag）
    let map = shared::LanguageCodeMap::from_entries(vec![]);
    let name = FileOrganizer::build_subtitle_dest_name(
        "Title", 1, 1, "/downloads/sub.XX.srt", &map
    );
    assert_eq!(name, "Title - S01E01.XX.srt");
}
```

### Step 2: 執行測試確認失敗

```bash
cargo test -p viewer-jellyfin 2>&1 | tail -20
```

預期：`build_subtitle_dest_name` 不存在，編譯錯誤

### Step 3: 更新 FileOrganizer struct 加入 LanguageCodeMap

在 `file_organizer.rs` 中：

```rust
use shared::{extract_language_tag, LanguageCodeMap};

pub struct FileOrganizer {
    source_dir: PathBuf,
    library_dir: PathBuf,
    language_codes: LanguageCodeMap,
}

impl FileOrganizer {
    pub fn new(source_dir: PathBuf, library_dir: PathBuf, language_codes: LanguageCodeMap) -> Self {
        Self { source_dir, library_dir, language_codes }
    }

    // ... 現有方法 ...
}
```

### Step 4: 實作 build_subtitle_dest_name（pub(crate) 供測試用）

```rust
impl FileOrganizer {
    /// 建構字幕檔的目標檔名。
    /// 例：source="/downloads/sub.TC.ass", title="Title", season=1, episode=1
    ///     → "Title - S01E01.zh-TW.ass"（若 TC→zh-TW 在 map 中）
    pub(crate) fn build_subtitle_dest_name(
        title: &str,
        season: u32,
        episode: u32,
        source_path: &str,
        language_codes: &LanguageCodeMap,
    ) -> String {
        let path = std::path::Path::new(source_path);
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("ass");

        let base = format!("{} - S{:02}E{:02}", Self::sanitize_filename(title), season, episode);

        match extract_language_tag(source_path) {
            Some(raw_tag) => {
                let normalized = language_codes.normalize(&raw_tag);
                format!("{}.{}.{}", base, normalized, ext)
            }
            None => format!("{}.{}", base, ext),
        }
    }
}
```

### Step 5: 實作 organize_subtitles 方法

```rust
impl FileOrganizer {
    /// 搬移所有字幕檔到 Jellyfin library。
    /// 若有重複的語言 tag（同一語言多個字幕），加上序號後綴。
    pub async fn organize_subtitles(
        &self,
        subtitle_paths: &[String],
        anime_title: &str,
        season: u32,
        episode: u32,
    ) -> Vec<PathBuf> {
        let mut results = Vec::new();
        let season_dir = self.library_dir
            .join(Self::sanitize_filename(anime_title))
            .join(format!("Season {:02}", season));

        // 追蹤已使用的目標檔名，避免衝突
        let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();

        for source_path in subtitle_paths {
            let source = self.resolve_download_path(source_path);
            if !source.exists() {
                tracing::warn!("Subtitle file not found: {}", source_path);
                continue;
            }

            let mut dest_name = Self::build_subtitle_dest_name(
                anime_title, season, episode, source_path, &self.language_codes,
            );

            // 若檔名衝突，加上序號
            if used_names.contains(&dest_name) {
                let ext = std::path::Path::new(&dest_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("ass")
                    .to_string();
                let stem = dest_name.trim_end_matches(&format!(".{}", ext));
                let mut i = 2;
                loop {
                    let candidate = format!("{}.{}.{}", stem, i, ext);
                    if !used_names.contains(&candidate) {
                        dest_name = candidate;
                        break;
                    }
                    i += 1;
                }
            }
            used_names.insert(dest_name.clone());

            let dest = season_dir.join(&dest_name);
            match tokio::fs::rename(&source, &dest).await {
                Ok(()) => {
                    tracing::info!("Moved subtitle: {} → {}", source_path, dest.display());
                    results.push(dest);
                }
                Err(e) => {
                    // cross-device fallback
                    if let Ok(()) = tokio::fs::copy(&source, &dest).await {
                        let _ = tokio::fs::remove_file(&source).await;
                        results.push(dest);
                    } else {
                        tracing::warn!("Failed to move subtitle {}: {}", source_path, e);
                    }
                }
            }
        }
        results
    }
}
```

### Step 6: 執行測試確認通過

```bash
cargo test -p viewer-jellyfin 2>&1 | tail -20
```

預期：所有測試通過

### Step 7: Commit

```bash
git add viewers/jellyfin/src/file_organizer.rs
git commit -m "feat(viewer): add subtitle file organization with language tag normalization"
```

---

## Task 11: viewer-jellyfin main.rs 和 handlers.rs

**Files:**
- Modify: `viewers/jellyfin/src/main.rs`
- Modify: `viewers/jellyfin/src/handlers.rs`

### Step 1: 更新 main.rs — 載入 LanguageCodeMap

在 `viewers/jellyfin/src/main.rs` 加入：

```rust
use shared::LanguageCodeMap;
use std::path::Path;

// 在 main() 函式中，建立 FileOrganizer 之前：
let language_codes_path = std::env::var("LANGUAGE_CODES_PATH")
    .unwrap_or_else(|_| "/etc/bangumi/language_codes.json".to_string());

let language_codes = LanguageCodeMap::load_from_file(Path::new(&language_codes_path))
    .unwrap_or_else(|e| {
        tracing::warn!(
            "Failed to load language codes from {}: {}. Using empty map.",
            language_codes_path, e
        );
        LanguageCodeMap::from_entries(vec![])
    });

// 更新 FileOrganizer::new() 呼叫，加入 language_codes：
let organizer = Arc::new(FileOrganizer::new(
    PathBuf::from(&downloads_dir),
    PathBuf::from(&library_dir),
    language_codes,
));
```

> **說明：** `load_from_file` 失敗時 fallback 到空 map（不影響功能，只是語言 tag 不會被標準化），並打 warn log 提醒。

### Step 2: 更新 handlers.rs — 加入字幕搬移

在 `do_sync` 函式中，在影片搬移成功之後，加入字幕搬移：

```rust
async fn do_sync(
    organizer: &FileOrganizer,
    db: &DbPool,
    metadata: &MetadataClient,
    req: &ViewerSyncRequest,
) -> anyhow::Result<String> {
    // 1. 搬移影片（改用 video_path）
    let source = organizer.resolve_download_path(&req.video_path);
    let target_path = organizer
        .organize_episode(
            &req.anime_title,
            req.series_no as u32,
            req.episode_no as u32,
            &source,
        )
        .await?;

    // 2. 搬移字幕（新增）
    if !req.subtitle_paths.is_empty() {
        organizer.organize_subtitles(
            &req.subtitle_paths,
            &req.anime_title,
            req.series_no as u32,
            req.episode_no as u32,
        ).await;
        // 字幕搬移失敗不影響主流程（非 fatal）
    }

    // 3. Fetch metadata 和生成 NFO（現有邏輯不變）
    if let Err(e) = fetch_and_generate_metadata(
        db, metadata, organizer,
        req.bangumi_id, req.cover_image_url.as_deref(),
        &req.anime_title, req.series_no, req.episode_no,
        &target_path, false,
    ).await {
        tracing::warn!("Metadata fetch failed for download {} (non-fatal): {}", req.download_id, e);
    }

    Ok(target_path.display().to_string())
}
```

### Step 3: 更新 FileOrganizer 現有測試

在 `file_organizer.rs` 中，`FileOrganizer::new` 的測試需要更新 constructor 呼叫：

```rust
#[test]
fn test_file_organizer_creation() {
    let organizer = FileOrganizer::new(
        PathBuf::from("/downloads"),
        PathBuf::from("/media/jellyfin"),
        shared::LanguageCodeMap::from_entries(vec![]),
    );
    assert!(organizer.get_library_dir().to_str().unwrap().contains("jellyfin"));
}
```

### Step 4: 確認全部編譯和測試

```bash
cargo build -p viewer-jellyfin 2>&1 | grep -E "^error"
cargo test -p viewer-jellyfin 2>&1 | tail -20
```

預期：無 error，所有測試通過

### Step 5: Commit

```bash
git add viewers/jellyfin/src/main.rs viewers/jellyfin/src/handlers.rs viewers/jellyfin/src/file_organizer.rs
git commit -m "feat(viewer): load LanguageCodeMap at startup and organize subtitles in sync handler"
```

---

## Task 12: Docker 與 docker-compose 設定

**Files:**
- Modify: `Dockerfile.viewer-jellyfin`
- Modify: `docker-compose.yaml`

### Step 1: 更新 Dockerfile.viewer-jellyfin

在最終 stage（`FROM debian...` 之後），加入複製 JSON 檔的指令：

```dockerfile
# 複製語言代碼對照表
COPY shared/assets/language_codes.json /etc/bangumi/language_codes.json
```

確認路徑（加在 CMD 之前）。

### Step 2: 更新 docker-compose.yaml

在 `viewer-jellyfin` service 的 `environment` 區塊加入：

```yaml
viewer-jellyfin:
  environment:
    - LANGUAGE_CODES_PATH=/etc/bangumi/language_codes.json
  # 可選：如果使用者想用自訂的 JSON，可在 volumes 中覆蓋：
  # volumes:
  #   - ./custom_language_codes.json:/etc/bangumi/language_codes.json
```

### Step 3: 確認 docker-compose 語法

```bash
docker compose config --quiet 2>&1 | head -20
```

### Step 4: 確認整個 workspace 編譯

```bash
cargo build --workspace 2>&1 | grep -E "^error"
```

預期：無 error

### Step 5: Commit

```bash
git add Dockerfile.viewer-jellyfin docker-compose.yaml
git commit -m "feat(docker): add language_codes.json to viewer-jellyfin image"
```

---

## 驗收標準

1. `cargo build --workspace` 無 error
2. `cargo test -p shared` 所有 file_classifier 測試通過
3. `cargo test -p viewer-jellyfin` 所有測試通過（包含新的 subtitle 命名測試）
4. 單一影片 torrent：`video_path` 正確，`subtitle_paths` 為空陣列
5. 資料夾型 torrent（影片 + 多字幕）：`subtitle_paths` 包含所有字幕，搬移後命名格式為 `{Title} - SxxExx.{lang}.{ext}`
6. 語言代碼標準化：TC → zh-TW、SC → zh-CN
7. 未知語言代碼：保留原始 tag（不報錯）
