# Viewer Jellyfin 設計文件

> 日期：2026-02-06

## 概述

當 Core 偵測到下載完成後，通知 Viewer 進行檔案整理和 metadata 抓取。Viewer 將檔案搬移到 Jellyfin 媒體庫目錄，從 bangumi.tv 取得 metadata 並產生 Jellyfin 相容的 NFO 檔案，最後回報 Core 處理結果。整個過程是非同步的。

## 架構與資料流

```
DownloadScheduler 偵測到 completed
       │
       ▼
從 qBittorrent API 查詢檔案路徑，存入 downloads.file_path
       │
       ▼
更新 status = syncing，POST /sync 給 Viewer
       │
       ▼
Viewer 收到 → 回傳 202 Accepted（ACK）
       │
       ▼
Viewer 非同步處理：
  1. 移動檔案 /downloads → /media/jellyfin/{title}/Season XX/
  2. 查 bangumi_mapping 取得 bangumi_id（沒有就搜尋 bangumi.tv）
  3. 產生 tvshow.nfo + poster.jpg（如果還沒有）
  4. 產生 episode.nfo
       │
       ▼
Viewer 回呼 Core：POST /sync-callback
  { download_id, status: "synced"/"failed", target_path, error_message }
       │
       ▼
Core 更新 downloads.status = synced / sync_failed
  如果 sync_failed → 自動重試（最多 3 次）
```

## Docker Volume 掛載

```
qBittorrent  → /downloads (rw)
Viewer       → /downloads (rw) + /media (rw)
Jellyfin     → /media (ro)
```

Viewer 使用 `std::fs::rename` 移動檔案（同 volume 下為原子操作），搬完後原檔不保留。

## Core 側變更

### downloads 表新增欄位

```sql
ALTER TABLE downloads ADD COLUMN file_path TEXT;
ALTER TABLE downloads ADD COLUMN sync_retry_count INT DEFAULT 0;
```

### 狀態擴充

```
pending → downloading → completed → syncing → synced
                ↓            ↓           ↓
             failed    downloader_error  sync_failed (retry ≤ 3 → syncing)
```

### DownloadScheduler 擴充

偵測到 `completed` 時：

1. 從 qBittorrent API 用 `torrent_hash` 查詢檔案路徑
2. 存入 `downloads.file_path`
3. 從 `service_modules` 查找 `module_type = viewer` 的模組
4. POST `/sync` 給 Viewer
5. 更新 `status = syncing`

### Core 通知 Viewer 的 payload

```json
{
    "download_id": 42,
    "series_id": 1,
    "anime_title": "葬送的芙莉蓮",
    "series_no": 1,
    "episode_no": 5,
    "subtitle_group": "桜都字幕组",
    "file_path": "/downloads/[桜都字幕组] 葬送的芙莉蓮 05.mkv",
    "callback_url": "http://core-service:8000/sync-callback"
}
```

### 新增 callback endpoint

```
POST /sync-callback
{
    "download_id": 42,
    "status": "synced" | "failed",
    "target_path": "/media/jellyfin/葬送的芙莉蓮/Season 01/葬送的芙莉蓮 - S01E05.mkv",
    "error_message": null
}
```

處理邏輯：
- `synced` → 更新 status = synced，存 target_path
- `failed` → sync_retry_count += 1，若 < 3 則重新通知 Viewer（status 回到 syncing），否則 status = sync_failed

## Viewer 側設計

### 獨立資料庫：viewer_jellyfin

與 Core 的 `bangumi` 資料庫完全隔離，共用同一個 PostgreSQL server。

```sql
-- 1. Core anime_series 到 bangumi.tv 的映射
CREATE TABLE bangumi_mapping (
    core_series_id  INT PRIMARY KEY,
    bangumi_id      INT NOT NULL,
    title_cache     TEXT,
    source          VARCHAR(20) NOT NULL,     -- 'auto_search' | 'manual'
    created_at      TIMESTAMP DEFAULT NOW(),
    updated_at      TIMESTAMP DEFAULT NOW()
);

-- 2. bangumi.tv metadata 快取
CREATE TABLE bangumi_subjects (
    bangumi_id      INT PRIMARY KEY,
    title           TEXT NOT NULL,
    title_cn        TEXT,
    summary         TEXT,
    rating          REAL,
    cover_url       TEXT,
    air_date        DATE,
    episode_count   INT,
    raw_json        JSONB,
    fetched_at      TIMESTAMP DEFAULT NOW()
);

-- 3. 單集 metadata 快取
CREATE TABLE bangumi_episodes (
    bangumi_ep_id   INT PRIMARY KEY,
    bangumi_id      INT REFERENCES bangumi_subjects(bangumi_id),
    episode_no      INT NOT NULL,
    title           TEXT,
    title_cn        TEXT,
    air_date        DATE,
    summary         TEXT,
    fetched_at      TIMESTAMP DEFAULT NOW()
);

-- 4. 同步處理紀錄
CREATE TABLE sync_tasks (
    task_id         SERIAL PRIMARY KEY,
    download_id     INT NOT NULL,
    core_series_id  INT NOT NULL,
    episode_no      INT NOT NULL,
    source_path     TEXT NOT NULL,
    target_path     TEXT,
    status          VARCHAR(20) DEFAULT 'pending',  -- pending | processing | completed | failed
    error_message   TEXT,
    created_at      TIMESTAMP DEFAULT NOW(),
    completed_at    TIMESTAMP
);
```

### /sync endpoint

收到 Core 的通知後立即回傳 `202 Accepted`，`tokio::spawn` 非同步處理。

### 檔案搬移

```
/downloads/[桜都字幕组] 葬送的芙莉蓮 05.mkv
       │
       ▼  std::fs::rename
/media/jellyfin/葬送的芙莉蓮/Season 01/葬送的芙莉蓮 - S01E05.mkv
```

命名規則：`{anime_title} - S{season:02}E{episode:02}.{ext}`

### bangumi.tv Metadata 流程

```
收到 series_id
    │
    ▼
查 bangumi_mapping 表 → 有 bangumi_id？
    │                         │
    否                        是
    ▼                         ▼
用 anime_title 搜尋        直接用 bangumi_id
GET /search/subject/{kw}?type=2
    │                         │
    ▼                         ▼
取第一筆結果 → 存入映射表 → GET /v0/subjects/{bangumi_id}
                           GET /v0/episodes?subject_id={bangumi_id}&type=0
                              │
                              ▼
                     產生 NFO + 下載封面圖
```

### bangumi.tv API

```
1. 搜尋動畫
   GET https://api.bgm.tv/search/subject/{keyword}?type=2

2. 取得動畫詳情
   GET https://api.bgm.tv/v0/subjects/{bangumi_id}

3. 取得單集列表
   GET https://api.bgm.tv/v0/episodes?subject_id={bangumi_id}&type=0
```

Rate limiting：每次請求間隔 1 秒。同一部動畫只在首次處理時抓取。

### 錯誤處理

- bangumi.tv 不可用 → 檔案照搬，NFO 不產生，回報 Core `synced`
- 搜尋不到結果 → 同上，記 log 警告
- 後續可透過用戶手動指定 `bangumi_id` 補救
- bangumi.tv metadata 缺失不影響 `synced` 狀態（檔案已到位即算成功）

### Jellyfin NFO 格式

**tvshow.nfo（每部動畫一個）：**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<tvshow>
    <title>葬送的芙莉蓮</title>
    <originaltitle>葬送のフリーレン</originaltitle>
    <plot>勇者一行人打倒了魔王...</plot>
    <rating>8.9</rating>
    <year>2023</year>
    <uniqueid type="bangumi">424883</uniqueid>
</tvshow>
```

**episode.nfo（每集一個，與影片同名）：**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<episodedetails>
    <title>冒險的結束</title>
    <season>1</season>
    <episode>1</episode>
    <aired>2023-09-29</aired>
    <plot>打倒魔王的勇者一行人凱旋歸來...</plot>
    <uniqueid type="bangumi">424883</uniqueid>
</episodedetails>
```

**poster.jpg：**
- 從 bangumi.tv 的 `images.large` URL 下載
- 存放於動畫根目錄

**產生時機：**
- `tvshow.nfo` + `poster.jpg` → 該動畫目錄首次建立時產生，已存在則跳過
- `episode.nfo` → 每次搬移新集數時產生

## 產生的目錄結構

```
/media/jellyfin/葬送的芙莉蓮/
├── tvshow.nfo
├── poster.jpg
├── Season 01/
│   ├── 葬送的芙莉蓮 - S01E01.mkv
│   ├── 葬送的芙莉蓮 - S01E01.nfo
│   ├── 葬送的芙莉蓮 - S01E05.mkv
│   └── 葬送的芙莉蓮 - S01E05.nfo
```

## 變更範圍總覽

### Core 側
1. 新增遷移 — downloads 加 file_path、sync_retry_count，擴充 status 約束
2. DownloadScheduler 擴充 — completed 時查 qBittorrent 取 file_path，通知 Viewer
3. 新增 /sync-callback endpoint — 接收 Viewer 回報，處理重試邏輯

### Viewer 側
4. 新增 viewer_jellyfin 資料庫 — 4 張表
5. 改造 /sync endpoint — 新 payload，202 ACK + 非同步處理
6. 檔案搬移模組 — std::fs::rename
7. bangumi.tv 客戶端 — 搜尋、取 subject、取 episodes
8. NFO 產生器 — tvshow.nfo, episode.nfo, poster.jpg
9. 回呼 Core — POST callback_url

### Docker
10. Viewer 容器 — 掛載 /downloads(rw) + /media(rw)
11. Jellyfin 容器（可選）— 掛載 /media(ro)
