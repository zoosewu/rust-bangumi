# Bangumi API 規格文檔

本文檔說明 Bangumi 項目中所有 API 規格文件的組織結構和用途。

## API 規格文件概覽

### 1. 核心服務 API (`/docs/api/openapi.yaml`)

**目的：** 定義核心服務的完整 REST API 規格

**涵蓋範圍：**
- 服務註冊和管理
- 動畫、季度、系列管理
- 字幕組管理
- 過濾規則管理
- 動畫連結管理
- RSS 訂閱管理
- 標題解析器管理（Title Parsers）
- 原始 RSS 項目管理（Raw Items）
- Fetcher 結果接收（結構化 + 原始）
- Viewer 同步回呼
- 衝突解決
- 健康檢查

**服務器：**
- 開發環境：`http://localhost:8000`
- 生產環境：`http://core-service:8000`（Docker）

**使用場景：**
- 其他微服務（Fetcher、Downloader、Viewer）與核心服務通信
- Fetcher 向核心服務提交爬取結果
- Viewer 向核心服務回呼同步結果
- 前端應用調用核心服務 API

### 2. 通用 Fetcher API (`/docs/api/fetcher-openapi.yaml`)

**目的：** 定義所有 Fetcher 服務的標準 API 介面

**涵蓋範圍：**
- 健康檢查 (`GET /health`)
- RSS 爬取功能 (`POST /fetch`)
- URL 歸屬檢查 (`POST /can-handle-subscription`)

**特點：**
- 通用的請求/響應格式
- 所有 Fetcher 服務應實現的標準端點
- 與具體 Fetcher 實現無關

### 3. Mikanani Fetcher API (`/docs/api/mikanani-fetcher-openapi.yaml`)

**目的：** 定義 Mikanani 特化 Fetcher 服務的詳細 API 規格

**涵蓋範圍：**
- 健康檢查 (`GET /health`)
- Mikanani 專用的 RSS 爬取 (`POST /fetch`)
- URL 歸屬檢查 (`POST /can-handle-subscription`)

**服務器：**
- Docker 生產環境：`http://fetcher-mikanani:8001`
- 本地開發環境：`http://localhost:8001`

**Mikanani 特性：**
- 支援 Mikanani RSS 格式解析
- 指數退避重試機制（最多 3 次）
- 非同步爬取（回呼模式）
- 優先提取 magnet link，fallback 到 .torrent URL

### 4. Downloader API (`/docs/api/downloader-openapi.yaml`)

**目的：** 定義 qBittorrent Downloader 服務的 API 規格

**涵蓋範圍：**
- 批次新增下載 (`POST /downloads`)
- 查詢下載狀態 (`GET /downloads`)
- 批次取消下載 (`POST /downloads/cancel`)
- 暫停/恢復下載 (`POST /downloads/:hash/pause`, `/resume`)
- 刪除下載 (`DELETE /downloads/:hash`)
- 健康檢查 (`GET /health`)

**服務器：**
- Docker 生產環境：`http://downloader-qbittorrent:8002`
- 本地開發環境：`http://localhost:8002`

**特點：**
- 支援 magnet link（優先）和 .torrent HTTP URL
- 批次操作（新增、取消）
- 進度追蹤（progress 0-1、size、content_path）
- 啟動時自動向 Core 註冊，宣告支援的 DownloadType

### 5. Viewer API (`/docs/api/viewer-openapi.yaml`)

**目的：** 定義 Jellyfin Viewer 服務的 API 規格

**涵蓋範圍：**
- 同步請求 (`POST /sync`)
- 健康檢查 (`GET /health`)

**服務器：**
- Docker 生產環境：`http://viewer-jellyfin:8003`
- 本地開發環境：`http://localhost:8003`

**特點：**
- 非同步處理：接收 Core 同步請求，立即回傳 202
- 背景處理：檔案搬移 → bangumi.tv metadata → NFO 產生
- 完成後回呼 Core 的 `/sync-callback`

## API 規格之間的關係

```
┌─────────────────────────────────────────────────────┐
│         核心服務 (Core Service) :8000                │
│  ┌───────────────────────────────────────────────┐  │
│  │ FetchScheduler  → 定期觸發 Fetcher 爬取       │  │
│  │ DownloadScheduler → 偵測新連結，派送下載      │  │
│  │ SyncService → 下載完成後通知 Viewer 同步      │  │
│  └───────────────────────────────────────────────┘  │
│                                                      │
│  接收端點：                                          │
│  POST /raw-fetcher-results  ← Fetcher 回呼          │
│  POST /sync-callback        ← Viewer 回呼           │
└──────────┬──────────┬──────────────┬─────────────────┘
           │          │              │
     ①觸發爬取  ②派送下載     ③通知同步
     POST /fetch POST /downloads POST /sync
           │          │              │
           ▼          ▼              ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────────┐
│   Fetcher    │ │  Downloader  │ │     Viewer       │
│  Mikanani    │ │ qBittorrent  │ │    Jellyfin      │
│    :8001     │ │    :8002     │ │     :8003        │
│              │ │              │ │                    │
│ ④回呼結果   │ │ ⑤回報狀態   │ │ ⑥回呼結果        │
│ POST /raw-  │ │ (Core 輪詢)  │ │ POST /sync-      │
│ fetcher-    │ │              │ │ callback          │
│ results     │ │              │ │                    │
└──────────────┘ └──────────────┘ └──────────────────┘
```

## API 數據流

### 1. Fetcher 爬取流程（Core-Driven）

```
1. POST /subscriptions → 建立 RSS 訂閱
2. FetchScheduler 每 60 秒檢查到期訂閱
3. Core POST /fetch → Fetcher（非同步，Fetcher 回傳 202）
4. Fetcher 背景爬取 RSS 並解析
5. Fetcher POST /raw-fetcher-results → Core（回呼提交原始結果）
6. Core 使用 TitleParser 解析標題，存入 raw_items 表
7. 成功解析的項目建立動畫、季度、字幕組、連結
```

### 2. 下載派送流程（Auto-Download）

```
1. Core DownloadScheduler 偵測 status=pending 的連結
2. Core 查詢已註冊的 Downloader 服務
3. Core POST /downloads → Downloader（批次發送 magnet/torrent URL）
4. Downloader 返回每個任務的 hash 和狀態
5. Core 定期 GET /downloads → Downloader 查詢進度
6. 下載完成後（progress=1.0, status=completed），Core 更新 downloads 表
```

### 3. Viewer 同步流程

```
1. Core 偵測 downloads.status=completed 且 file_path 已填入
2. Core POST /sync → Viewer（含 anime_title, episode_no, file_path, callback_url）
3. Viewer 回傳 202 Accepted
4. Viewer 背景處理：
   a. fs::rename 搬移檔案至 {anime_title}/Season {XX}/ 結構
   b. 搜尋 bangumi.tv metadata（best-effort）
   c. 產生 tvshow.nfo + poster.jpg + episode.nfo
   d. 記錄 sync_tasks 表
5. Viewer POST /sync-callback → Core（回報 synced/failed）
6. Core 更新 downloads.status = synced（或重試 → sync_failed）
```

### 4. 結果結構對應

**Fetcher 端（RawFetcherResultsPayload）：**
- `subscription_id` - 來源訂閱
- `items[]` - 原始 RSS 項目（title, description, download_url, pub_date）
- `fetcher_source` - 來源識別（如 "mikanani"）

**Core 端：**
- 接收原始結果 → 使用 TitleParser 解析標題
- 轉換為數據庫模型：`Anime`, `AnimeSeries`, `AnimeLink`, `SubtitleGroup`

**Downloader 端（BatchDownloadRequest）：**
- `items[]` - 批次下載項目（url, save_path）
- 回傳每個項目的 hash、status

**Viewer 端（ViewerSyncRequest）：**
- 包含完整的動畫資訊（anime_title, series_no, episode_no, subtitle_group）
- `file_path` - 下載完成的檔案位置
- `callback_url` - 完成後回呼的 URL

## 開發指南

### 開發 Fetcher 服務時

1. 實現通用 Fetcher API (`fetcher-openapi.yaml`) 中的所有端點
2. 實現 `POST /can-handle-subscription` 來聲明 URL 歸屬
3. 確保 `POST /fetch` 為非同步操作（立即回傳 202，背景執行）
4. 完成後回呼到 `callback_url`（即 Core 的 `POST /raw-fetcher-results`）

### 開發 Downloader 服務時

1. 實現 Downloader API (`downloader-openapi.yaml`) 中的所有端點
2. 啟動時向 Core 註冊，宣告支援的 `DownloadType`（magnet/torrent/http）
3. `POST /downloads` 接受批次操作
4. 提供 `GET /downloads?hashes=...` 讓 Core 輪詢狀態

### 開發 Viewer 服務時

1. 實現 Viewer API (`viewer-openapi.yaml`) 中的所有端點
2. `POST /sync` 為非同步操作（立即回傳 202，背景執行）
3. 完成後回呼到 `callback_url`（即 Core 的 `POST /sync-callback`）
4. 啟動時向 Core 的 `POST /services/register` 註冊

### 整合新的 Fetcher / Downloader / Viewer 服務

1. 建立新的服務目錄
2. 實現對應的通用 API 規格
3. 建立特化的 OpenAPI 規格文件
4. 向核心服務註冊 (`POST /services/register`)
5. 視服務類型實現 URL 歸屬/下載類型/同步能力的宣告

## 規格驗證

所有 API 規格均遵守 OpenAPI 3.0.0 標準，可使用以下工具驗證：

### 使用 Swagger UI 檢視

```bash
# 核心服務 API
docker run -p 9090:8080 -e SWAGGER_JSON=/docs/api/openapi.yaml \
  -v $(pwd)/docs/api:/docs/api swaggerapi/swagger-ui

# Downloader API
docker run -p 9090:8080 -e SWAGGER_JSON=/docs/api/downloader-openapi.yaml \
  -v $(pwd)/docs/api:/docs/api swaggerapi/swagger-ui

# Viewer API
docker run -p 9090:8080 -e SWAGGER_JSON=/docs/api/viewer-openapi.yaml \
  -v $(pwd)/docs/api:/docs/api swaggerapi/swagger-ui
```

### 本地驗證（使用 swagger-cli）

```bash
npm install -g swagger-cli
swagger-cli validate docs/api/openapi.yaml
swagger-cli validate docs/api/fetcher-openapi.yaml
swagger-cli validate docs/api/mikanani-fetcher-openapi.yaml
swagger-cli validate docs/api/downloader-openapi.yaml
swagger-cli validate docs/api/viewer-openapi.yaml
```

## 端點統計

| 服務 | 端點數量 | 主要端點 |
|------|---------|---------|
| 核心服務 | 45 | `/services`, `/anime`, `/subscriptions`, `/parsers`, `/raw-items`, `/sync-callback` |
| Fetcher (通用) | 3 | `/health`, `/fetch`, `/can-handle-subscription` |
| Mikanani Fetcher | 3 | `/health`, `/fetch`, `/can-handle-subscription` |
| Downloader (qBittorrent) | 7 | `/health`, `/downloads`, `/downloads/cancel`, `pause/resume/delete` |
| Viewer (Jellyfin) | 2 | `/health`, `/sync` |

## 版本管理

- **核心服務版本：** 0.2.0
- **Fetcher API 版本：** 0.2.0
- **Mikanani Fetcher 版本：** 0.2.0
- **Downloader API 版本：** 0.1.0
- **Viewer API 版本：** 0.1.0

---

**最後更新：** 2026-02-09
**維護者：** Bangumi Project
