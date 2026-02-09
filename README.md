# Rust Bangumi - 動畫 RSS 聚合與下載管理系統

使用 Rust + PostgreSQL 構建的微服務架構動畫 RSS 聚合、下載與媒體庫管理系統。

## 系統架構

```
┌──────────┐     ┌──────────────┐     ┌───────────────────┐     ┌────────────────┐
│  Fetcher │◄────│ Core Service │────►│    Downloader     │────►│    Viewer      │
│ Mikanani │     │  (協調器)    │     │  (qBittorrent)    │     │  (Jellyfin)    │
│  :8001   │     │    :8000     │     │      :8002        │     │    :8003       │
└──────────┘     └──────┬───────┘     └───────────────────┘     └────────────────┘
                        │
                   ┌────┴────┐
                   │PostgreSQL│
                   └─────────┘
```

系統由 4 個微服務 + CLI 工具組成：

| 服務 | 說明 | Port |
|------|------|------|
| Core Service | 主協調器，管理資料庫、調度抓取與下載、接收回呼 | 8000 |
| Fetcher (Mikanani) | 從 Mikanani RSS 抓取動畫資訊，解析標題 | 8001 |
| Downloader (qBittorrent) | 批次管理 BT 下載任務（magnet/torrent） | 8002 |
| Viewer (Jellyfin) | 將下載完成的檔案整理至 Jellyfin 媒體庫，產生 NFO metadata | 8003 |
| CLI | 命令列工具，管理訂閱、動畫、下載等 | - |

外部依賴：

- **PostgreSQL** - 資料庫（Core 使用 `bangumi`，Viewer 使用 `viewer_jellyfin`）
- **qBittorrent** - BT 下載客戶端（WebUI :8080）
- **Jellyfin** - 媒體伺服器（WebUI :8096，可選）

## 完整資料流

```
1. 使用者透過 CLI 或 API 新增 RSS 訂閱
2. Core FetchScheduler 定期觸發 Fetcher 抓取 RSS
3. Fetcher 回呼 Core 提交原始結果，Core 解析標題並存入資料庫
4. Core DownloadScheduler 偵測新連結，派送至 Downloader 下載
5. Downloader 定期回報進度，Core 更新下載狀態
6. 下載完成後，Core 通知 Viewer 同步檔案至 Jellyfin 媒體庫
7. Viewer 搬移檔案、從 bangumi.tv 取得 metadata、產生 NFO
8. Viewer 回呼 Core 回報同步結果
```

## 快速部署

### 前置條件

- Docker & Docker Compose v2+
- 至少 2GB RAM

### 1. 設定環境變數

```bash
# 複製生產環境模板
cp .env.prod .env

# 編輯必要變數
vim .env
```

**必須設定的變數：**

```env
POSTGRES_DB=bangumi
POSTGRES_USER=bangumi
POSTGRES_PASSWORD=<your-secure-password>
```

### 2. 啟動服務

```bash
# 啟動核心服務（不含外部依賴）
docker compose up -d

# 啟動含 qBittorrent 和 Jellyfin
docker compose -f docker-compose.yaml -f docker-compose.override.yaml up -d
```

### 3. 驗證部署

```bash
# 檢查服務狀態
docker compose ps

# 檢查健康狀態
curl http://localhost:8000/health
curl http://localhost:8001/health
curl http://localhost:8002/health
curl http://localhost:8003/health

# 查看日誌
docker compose logs -f core-service
```

## 服務端點

### Core Service (8000)

| Method | Endpoint | 說明 |
|--------|----------|------|
| GET | `/health` | 健康檢查 |
| POST | `/services/register` | 註冊服務（Fetcher/Downloader/Viewer） |
| GET | `/services` | 列出已註冊服務 |
| GET | `/anime` | 列出動畫 |
| POST | `/anime` | 建立動畫 |
| GET | `/subscriptions` | 列出 RSS 訂閱 |
| POST | `/subscriptions` | 新增 RSS 訂閱 |
| GET | `/parsers` | 列出標題解析器 |
| POST | `/parsers` | 建立標題解析器 |
| GET | `/raw-items` | 列出原始 RSS 項目 |
| POST | `/fetcher-results` | 接收結構化 Fetcher 結果 |
| POST | `/raw-fetcher-results` | 接收原始 Fetcher 結果（自動解析） |
| POST | `/sync-callback` | 接收 Viewer 同步回呼 |
| GET | `/conflicts` | 列出待解決衝突 |

> 完整 API 文件見 [docs/API-SPECIFICATIONS.md](docs/API-SPECIFICATIONS.md) 和 [docs/api/](docs/api/)

### Fetcher (8001)

| Method | Endpoint | 說明 |
|--------|----------|------|
| GET | `/health` | 健康檢查 |
| POST | `/fetch` | 觸發 RSS 抓取（非同步，回傳 202） |
| POST | `/can-handle-subscription` | 檢查 URL 歸屬 |

### Downloader (8002)

| Method | Endpoint | 說明 |
|--------|----------|------|
| GET | `/health` | 健康檢查 |
| POST | `/downloads` | 批次新增下載任務（magnet/torrent） |
| GET | `/downloads` | 查詢下載狀態（`?hashes=h1,h2`） |
| POST | `/downloads/cancel` | 批次取消下載 |
| POST | `/downloads/:hash/pause` | 暫停下載 |
| POST | `/downloads/:hash/resume` | 恢復下載 |
| DELETE | `/downloads/:hash` | 刪除下載（`?delete_files=true`） |

### Viewer (8003)

| Method | Endpoint | 說明 |
|--------|----------|------|
| GET | `/health` | 健康檢查 |
| POST | `/sync` | 接收同步請求（非同步，回傳 202） |

## 配置說明

### 環境變數

| 變數 | 預設值 | 說明 |
|------|--------|------|
| `POSTGRES_DB` | - | 資料庫名稱（必填） |
| `POSTGRES_USER` | - | 資料庫使用者（必填） |
| `POSTGRES_PASSWORD` | - | 資料庫密碼（必填） |
| `CORE_PORT` | 8000 | Core Service port |
| `FETCHER_PORT` | 8001 | Fetcher port |
| `DOWNLOADER_PORT` | 8002 | Downloader port |
| `VIEWER_PORT` | 8003 | Viewer port |
| `QBITTORRENT_URL` | http://qbittorrent:8080 | qBittorrent WebUI |
| `DOWNLOADS_DIR` | /downloads | 下載檔案目錄 |
| `JELLYFIN_LIBRARY_DIR` | /media/jellyfin | Jellyfin 媒體庫目錄 |
| `RUST_LOG` | info | 日誌等級 |

### Docker Compose 檔案

| 檔案 | 用途 |
|------|------|
| `docker-compose.yaml` | 主要服務配置（Core, Fetcher, Downloader, Viewer） |
| `docker-compose.override.yaml` | 外部服務（qBittorrent + Jellyfin） |
| `docker-compose.dev.yaml` | 開發環境（PostgreSQL, Adminer, qBittorrent, Jellyfin） |

## CLI 工具

```bash
# 使用 Docker
docker run --rm --network bangumi-network bangumi-cli --help

# 訂閱 RSS
docker run --rm --network bangumi-network bangumi-cli subscribe \
  "https://mikanani.me/RSS/Classic" \
  --fetcher mikanani

# 列出動畫
docker run --rm --network bangumi-network bangumi-cli list

# 查看系統狀態
docker run --rm --network bangumi-network bangumi-cli status
```

## 維運指南

### 日誌查看

```bash
# 所有服務
docker compose logs -f

# 特定服務
docker compose logs -f core-service
docker compose logs -f downloader-qbittorrent
docker compose logs -f viewer-jellyfin
```

### 資料庫備份

```bash
# 備份 Core 資料庫
docker compose exec postgres pg_dump -U bangumi bangumi > backup-core.sql

# 備份 Viewer 資料庫
docker compose exec postgres pg_dump -U bangumi viewer_jellyfin > backup-viewer.sql

# 還原
docker compose exec -T postgres psql -U bangumi bangumi < backup-core.sql
```

### 更新服務

```bash
# 拉取最新代碼
git pull

# 重新構建並部署
docker compose build --no-cache
docker compose up -d
```

### 清理

```bash
# 停止所有服務
docker compose down

# 停止並刪除所有資料（危險！）
docker compose down -v
```

## 故障排除

### 服務無法啟動

```bash
# 檢查 port 是否被佔用
lsof -i :8000
lsof -i :5432

# 檢查容器日誌
docker compose logs core-service
```

### 資料庫連接失敗

```bash
# 確認 PostgreSQL 運行中
docker compose exec postgres pg_isready

# 測試連接
docker compose exec postgres psql -U bangumi -d bangumi -c "SELECT 1"
```

### Downloader 無法連接 qBittorrent

```bash
# 確認 qBittorrent 運行中
curl http://localhost:8080

# 檢查認證設定
docker compose logs qbittorrent
```

### Viewer 同步失敗

```bash
# 確認 Viewer 健康
curl http://localhost:8003/health

# 確認已向 Core 註冊
curl http://localhost:8000/services | jq '.[] | select(.service_type == "viewer")'

# 檢查 Viewer 日誌
docker compose logs viewer-jellyfin
```

## 開發

開發環境設置請參考 [DEVELOPMENT.md](DEVELOPMENT.md)

## 許可證

MIT
