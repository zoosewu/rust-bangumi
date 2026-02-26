# Torrent File Extraction Design

**Date:** 2026-02-26
**Status:** Approved

## Problem

Downloaded torrents may contain various packaging structures:
1. Single video file — `[SubGroup] Title - 01.mkv`
2. Folder with video + subtitles — `video.mkv` + `TC.ass`
3. Folder with video + multiple subtitle languages — `video.mkv` + `TC.ass` + `SC.ass`
4. Nested folders — `[SubGroup] Title/01/video.mkv`

Future packaging structures are expected. The system needs an extensible way to identify video and subtitle files before sending paths to the viewer.

## Constraints

- Core remains a pure coordinator — no filesystem access, no download folder mount
- Extract/classify logic must not be duplicated across viewers
- Multiple viewers and downloaders are anticipated in the future

## Chosen Approach

**Downloader reports standardized file list → `shared` crate classifies → Core stores results → Viewer receives pre-resolved paths**

## Data Flow

```
qBittorrent completes download
  │
  ▼
downloader-qbittorrent polls qBittorrent API
  ├─ content_path: "/downloads/[SubGroup] Title - 01"
  └─ files: ["/downloads/.../video.mkv", "/downloads/.../TC.ass", "/downloads/.../SC.ass"]
     (via qBittorrent files API; future downloaders without API fall back to directory scan)
  │
  ▼
Core DownloadScheduler receives download status
  ├─ calls shared::classify_files() → identifies Video / Subtitle / Other
  ├─ stores classification results in DB (downloads table)
  └─ builds ViewerSyncRequest with video_path + subtitle_paths
  │
  ▼
viewer-jellyfin
  ├─ moves video   → /media/jellyfin/{title}/Season XX/{title} - SxxExx.mkv
  └─ moves subtitles → /media/jellyfin/{title}/Season XX/{title} - SxxExx.{lang-tag}.ass
```

## Schema Changes

### `downloads` table — two new columns

```sql
ALTER TABLE downloads
  ADD COLUMN video_file TEXT,        -- classified video absolute path
  ADD COLUMN subtitle_files JSONB;   -- ["path1.ass", "path2.ass"]
```

`file_path` (existing) is retained as the top-level torrent content path (`content_path`).
`video_file` and `subtitle_files` hold the post-classification results.

## Shared Crate Changes (`shared/src/`)

### New types and functions

```rust
pub enum FileType { Video, Subtitle, Other }

pub struct MediaFile {
    pub path: String,
    pub file_type: FileType,
}

/// Classifies files by extension.
/// Video:    .mkv .mp4 .avi .ts .m2ts
/// Subtitle: .ass .ssa .srt .vtt .sup
pub fn classify_files(files: Vec<String>) -> Vec<MediaFile>;

pub struct LanguageCodeMap(HashMap<String, String>);

impl LanguageCodeMap {
    pub fn load_from_file(path: &Path) -> Result<Self>;
    /// Returns normalized tag (e.g. "TC" → "zh-TW"), or original tag if not found.
    pub fn normalize(&self, tag: &str) -> String;
}
```

### Language code JSON file

Location in repo: `shared/assets/language_codes.json`

```json
{
  "TC":  "zh-TW",
  "CHT": "zh-TW",
  "SC":  "zh-CN",
  "CHS": "zh-CN",
  "JP":  "ja",
  "EN":  "en"
}
```

Loaded at runtime by viewer-jellyfin on startup via env var `LANGUAGE_CODES_PATH` (default: `/etc/bangumi/language_codes.json`).

## Downloader API Changes

### `DownloadStatusItem` — new `files` field

```rust
pub struct DownloadStatusItem {
    pub hash: String,
    pub status: String,
    pub content_path: Option<String>,
    pub files: Vec<String>,   // NEW: absolute paths of all files in the torrent
}
```

**downloader-qbittorrent implementation:**
- Uses `GET /api/v2/torrents/files?hashes={hash}` to get relative filenames
- Prepends save path to construct absolute paths
- Future downloaders without a files API fall back to recursive `read_dir` on `content_path`

## ViewerSyncRequest Changes

```rust
// Before:
file_path: String,

// After:
video_path: String,
subtitle_paths: Vec<String>,
```

## Viewer File Organization Changes

### Video
Same logic as before, but source path changes from `content_path` to `video_path`.

### Subtitles (new)

Subtitle language tag is extracted from the original filename stem:

```
sub.TC.ass        →  {Title} - SxxExx.zh-TW.ass   (after LanguageCodeMap normalize)
subtitle.CHS.srt  →  {Title} - SxxExx.zh-CN.srt
sub.ass           →  {Title} - SxxExx.ass          (no tag found)
```

Extraction rule: split filename by `.`, take the second-to-last segment as the raw tag, pass through `LanguageCodeMap::normalize()`. If the stem has only one segment (no dot before extension), no language tag is added.

Fallback (multiple subtitles with unresolvable tags): append sequential index `1`, `2`, etc.

### Docker configuration

```dockerfile
# Dockerfile.viewer-jellyfin
COPY shared/assets/language_codes.json /etc/bangumi/language_codes.json
```

```yaml
# docker-compose.yaml (optional override)
viewer-jellyfin:
  environment:
    LANGUAGE_CODES_PATH: /etc/bangumi/language_codes.json
  volumes:
    - ./custom_language_codes.json:/etc/bangumi/language_codes.json
```

## Affected Files Summary

| File | Change |
|---|---|
| `shared/src/models.rs` | Add `FileType`, `MediaFile`, `LanguageCodeMap`, `classify_files()` |
| `shared/assets/language_codes.json` | New file |
| `core-service/migrations/` | New migration: add `video_file`, `subtitle_files` to `downloads` |
| `core-service/src/models/db.rs` | Update `Download` struct |
| `core-service/src/services/download_scheduler.rs` | Call `classify_files()`, populate new fields, update `ViewerSyncRequest` |
| `core-service/src/services/sync_service.rs` | Build `ViewerSyncRequest` with `video_path` + `subtitle_paths` |
| `downloaders/qbittorrent/src/` | Add `files` to `DownloadStatusItem`, fetch from qBittorrent files API |
| `viewers/jellyfin/src/file_organizer.rs` | Handle `video_path`, move subtitles with language tag naming |
| `viewers/jellyfin/src/main.rs` | Load `LanguageCodeMap` on startup |
| `Dockerfile.viewer-jellyfin` | Copy `language_codes.json` |
| `docker-compose.yaml` | Add `LANGUAGE_CODES_PATH` env var |
