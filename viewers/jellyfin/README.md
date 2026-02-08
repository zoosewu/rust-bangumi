# Viewer Jellyfin Service

接收 Core Service 的同步通知，將已下載的動畫檔案搬移至 Jellyfin 媒體庫目錄結構，並從 bangumi.tv 取得 metadata 產生 NFO 檔案。

## 功能

- **非同步同步管線**：收到 Core 通知後立即回應 202，背景處理檔案搬移與 metadata
- **檔案搬移與重新命名**：將下載完成的檔案移動至 `{anime_title}/Season XX/` 結構
- **bangumi.tv Metadata**：自動搜尋並快取動畫/集數資訊
- **NFO 產生**：產生 Jellyfin 相容的 `tvshow.nfo`、`episode.nfo`、`poster.jpg`
- **獨立資料庫**：使用 `viewer_jellyfin` PostgreSQL 資料庫，與 Core 完全分離
- **自動向 Core 註冊**：啟動後自動向 Core 的 `/services/register` 註冊

## 架構概觀

```
Core Service (DownloadScheduler)
    │  偵測下載完成 (status=completed, file_path 已填入)
    ▼
POST /sync  (ViewerSyncRequest)
    │
    ▼
Viewer Jellyfin (返回 202 ACCEPTED)
    │  背景非同步處理：
    │  1. fs::rename 搬移檔案（跨檔案系統 fallback: copy + delete）
    │  2. 查詢/搜尋 bangumi.tv metadata（best-effort，失敗不影響同步成功）
    │  3. 產生 tvshow.nfo + poster.jpg + episode.nfo
    │  4. 更新本地 sync_tasks 表
    ▼
POST {callback_url}  (ViewerSyncCallback → Core /sync-callback)
    │  回報 status=synced/failed
    ▼
Core Service (SyncService.handle_callback)
    │  synced → downloads.status = "synced"
    │  failed → 重試（最多 3 次），超過則 status = "sync_failed"
```

## 媒體庫目錄結構

```
/media/jellyfin/
├── 進擊的巨人/
│   ├── tvshow.nfo           ← bangumi.tv metadata
│   ├── poster.jpg           ← bangumi.tv 封面圖
│   ├── Season 01/
│   │   ├── 進擊的巨人 - S01E01.mkv
│   │   ├── 進擊的巨人 - S01E01.nfo   ← 集數 metadata
│   │   ├── 進擊的巨人 - S01E02.mkv
│   │   └── ...
│   └── Season 02/
│       └── ...
└── 咒術迴戰/
    ├── tvshow.nfo
    ├── poster.jpg
    └── Season 01/
        └── ...
```

## API

### POST /sync

接收 Core Service 的同步請求，立即返回 202，背景處理。

**Request Body** (`ViewerSyncRequest`，定義於 `shared` crate)：

```json
{
  "download_id": 42,
  "series_id": 5,
  "anime_title": "進擊的巨人",
  "series_no": 1,
  "episode_no": 3,
  "subtitle_group": "LoliHouse",
  "file_path": "/downloads/[LoliHouse] 進擊的巨人 - 03 [1080p].mkv",
  "callback_url": "http://core-service:8000/sync-callback"
}
```

**Response**：`202 ACCEPTED`（無 body）

處理完成後，Viewer 會主動 POST 到 `callback_url`：

```json
{
  "download_id": 42,
  "status": "synced",
  "target_path": "/media/jellyfin/進擊的巨人/Season 01/進擊的巨人 - S01E03.mkv",
  "error_message": null
}
```

### GET /health

```json
{
  "status": "healthy",
  "service": "jellyfin-viewer",
  "version": "0.1.0"
}
```

## 資料庫

Viewer 使用獨立的 PostgreSQL 資料庫 `viewer_jellyfin`，與 Core 的 `bangumi` 資料庫分離。Migration 內嵌於 binary，啟動時自動執行。

### Schema

| 表名 | 用途 |
|------|------|
| `bangumi_subjects` | bangumi.tv 動畫 metadata 快取 |
| `bangumi_episodes` | bangumi.tv 集數 metadata 快取 |
| `bangumi_mapping` | Core `series_id` ↔ bangumi.tv `subject_id` 對應 |
| `sync_tasks` | 同步任務歷史記錄 |

### bangumi_mapping.source 欄位值

| 值 | 說明 |
|----|------|
| `auto_search` | 自動搜尋 bangumi.tv 的第一筆結果 |
| `manual` | 使用者手動指定 bangumi_id（尚未實作 API） |

## 環境變數

| 變數 | 預設值 | 說明 |
|------|--------|------|
| `DATABASE_URL` | `postgresql://bangumi:bangumi_dev_password@localhost:5432/viewer_jellyfin` | Viewer 專用資料庫連線 |
| `DOWNLOADS_DIR` | `/downloads` | 下載檔案來源目錄 |
| `JELLYFIN_LIBRARY_DIR` | `/media/jellyfin` | Jellyfin 媒體庫目標目錄 |
| `CORE_SERVICE_URL` | `http://core-service:8000` | Core Service URL（用於註冊和回呼） |
| `SERVICE_HOST` | `viewer-jellyfin` | 向 Core 註冊時使用的 hostname |
| `RUST_LOG` | `viewer_jellyfin=debug` | 日誌層級 |

## 開發環境設置

### 前置需求

- Rust toolchain
- PostgreSQL（可使用 `docker-compose.dev.yaml` 啟動）
- Diesel CLI：`cargo install diesel_cli --no-default-features --features postgres`

### 1. 啟動 PostgreSQL

```bash
docker compose -f docker-compose.dev.yaml up -d postgres
```

### 2. 建立 Viewer 資料庫

Core 使用 `bangumi` 資料庫，Viewer 使用獨立的 `viewer_jellyfin` 資料庫：

```bash
# 連線到 PostgreSQL 建立資料庫
psql -h localhost -U bangumi -d bangumi -c "CREATE DATABASE viewer_jellyfin OWNER bangumi;"
```

或透過 Adminer（http://localhost:8081）操作。

### 3. 設定環境變數

```bash
cp .env.dev .env

# .env.dev 中已包含 Core 的 DATABASE_URL
# Viewer 需要額外設定（或在啟動時指定）：
export DATABASE_URL=postgresql://bangumi:bangumi_dev_password@localhost:5432/viewer_jellyfin
export DOWNLOADS_DIR=/tmp/bangumi-downloads    # 本地開發用的暫存目錄
export JELLYFIN_LIBRARY_DIR=/tmp/bangumi-media  # 本地開發用的輸出目錄
export CORE_SERVICE_URL=http://localhost:8000
export SERVICE_HOST=localhost
```

### 4. 啟動服務

```bash
# 先啟動 Core（Viewer 會向 Core 註冊）
cargo run -p core-service

# 另一個終端啟動 Viewer
DATABASE_URL=postgresql://bangumi:bangumi_dev_password@localhost:5432/viewer_jellyfin \
  cargo run -p viewer-jellyfin
```

Migration 會在啟動時自動執行，無需手動 `diesel migration run`。

### 5. 驗證

```bash
# 健康檢查
curl http://localhost:8003/health

# 確認已向 Core 註冊
curl http://localhost:8000/services | jq '.[] | select(.service_type == "viewer")'
```

### 6. 手動觸發同步（測試用）

```bash
# 建立測試用的下載檔案
mkdir -p /tmp/bangumi-downloads
echo "test" > /tmp/bangumi-downloads/test.mkv

# 發送同步請求
curl -X POST http://localhost:8003/sync \
  -H "Content-Type: application/json" \
  -d '{
    "download_id": 1,
    "series_id": 1,
    "anime_title": "Test Anime",
    "series_no": 1,
    "episode_no": 1,
    "subtitle_group": "TestGroup",
    "file_path": "/tmp/bangumi-downloads/test.mkv",
    "callback_url": "http://localhost:8000/sync-callback"
  }'
# 預期回應：202 ACCEPTED
```

## Docker 部署

### Volume 掛載

Viewer 需要同時存取下載目錄和媒體庫目錄：

| Container Path | 說明 | 共享對象 |
|----------------|------|---------|
| `/downloads` | 下載完成的檔案（讀寫） | 與 qBittorrent 共享 |
| `/media/jellyfin` | Jellyfin 媒體庫（讀寫） | 與 Jellyfin 共享（Jellyfin 為唯讀） |

```yaml
viewer-jellyfin:
  build:
    context: .
    dockerfile: Dockerfile.viewer-jellyfin
  environment:
    DATABASE_URL: postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/viewer_jellyfin
    DOWNLOADS_DIR: /downloads
    JELLYFIN_LIBRARY_DIR: /media/jellyfin
    CORE_SERVICE_URL: http://core-service:8000
  volumes:
    - qbittorrent_downloads:/downloads       # 與 qBittorrent 共享
    - jellyfin_media:/media/jellyfin         # 與 Jellyfin 共享
  depends_on:
    core-service:
      condition: service_healthy
    postgres:
      condition: service_healthy
```

### 啟動順序

```
PostgreSQL → Core Service → Viewer Jellyfin
                         → Fetcher Mikanani
                         → Downloader qBittorrent
```

Viewer 在 port binding 成功後才向 Core 註冊（deferred registration），避免 Core 在 Viewer 未就緒時就轉發請求。

## 檔案搬移策略

1. **嘗試 `fs::rename`**：同一檔案系統下為 O(1) 原子操作
2. **Fallback `copy + delete`**：跨檔案系統時複製後刪除原始檔案

> 注意：不使用 hard link，因為 Docker volume 和跨 filesystem 場景下 hard link 經常失敗。

## bangumi.tv API 整合

| Endpoint | 用途 |
|----------|------|
| `GET /search/subject/{keyword}?type=2` | 搜尋動畫，取第一筆結果 |
| `GET /v0/subjects/{id}` | 取得動畫詳細資訊（標題、評分、封面、總集數） |
| `GET /v0/episodes?subject_id={id}&type=0` | 取得集數列表（標題、播出日期） |

- 所有請求帶 `User-Agent: bangumi-viewer/1.0`
- 分頁請求間隔 1 秒（rate limiting）
- API 失敗為 non-fatal：檔案搬移成功即視為 `synced`

## NFO 格式

### tvshow.nfo（每部動畫一個，放在動畫根目錄）

```xml
<?xml version="1.0" encoding="UTF-8"?>
<tvshow>
    <title>進擊的巨人</title>
    <originaltitle>進撃の巨人</originaltitle>
    <plot>故事概要...</plot>
    <rating>8.5</rating>
    <year>2013</year>
    <uniqueid type="bangumi">12345</uniqueid>
</tvshow>
```

### episode.nfo（與影片同名，副檔名改為 .nfo）

```xml
<?xml version="1.0" encoding="UTF-8"?>
<episodedetails>
    <title>第三話標題</title>
    <season>1</season>
    <episode>3</episode>
    <aired>2013-04-21</aired>
    <plot>本集概要...</plot>
    <uniqueid type="bangumi">67890</uniqueid>
</episodedetails>
```

## 測試

```bash
# 執行所有測試
cargo test -p viewer-jellyfin

# 顯示輸出
cargo test -p viewer-jellyfin -- --nocapture

# 執行特定測試
cargo test -p viewer-jellyfin test_sanitize_filename
```

測試涵蓋：檔案名稱清理、集數 regex 匹配、JSON 序列化/反序列化、NFO XML 跳脫、服務註冊結構驗證。

## 原始碼結構

```
viewers/jellyfin/
├── Cargo.toml
├── diesel.toml
├── migrations/
│   └── 2026-02-08-000001-viewer-schema/
│       ├── up.sql              # 4 張表：subjects, episodes, mapping, sync_tasks
│       └── down.sql
├── src/
│   ├── main.rs                 # Axum server、AppState、migration、service registration
│   ├── handlers.rs             # POST /sync（async）、GET /health、metadata 處理
│   ├── file_organizer.rs       # 檔案搬移、目錄建立、檔名清理
│   ├── bangumi_client.rs       # bangumi.tv API client
│   ├── nfo_generator.rs        # tvshow.nfo / episode.nfo 產生器
│   ├── models.rs               # Diesel ORM models（Queryable + Insertable）
│   ├── db.rs                   # Connection pool
│   └── schema.rs               # Diesel 自動產生的 schema
└── tests/
    └── viewer_tests.rs         # Integration tests
```
