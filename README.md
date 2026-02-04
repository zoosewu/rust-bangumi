# Rust Bangumi - 動畫 RSS 聚合與下載管理系統

使用 Rust + PostgreSQL 構建的微服務架構動畫 RSS 聚合、下載與媒體庫管理系統。

## 系統架構

系統由 5 個主要微服務組成：

| 服務 | 說明 | Port |
|------|------|------|
| Core Service | 主協調器，管理數據庫、調度任務 | 8000 |
| Fetcher (Mikanani) | 從 RSS 取得動畫資訊 | 8001 |
| Downloader (qBittorrent) | 執行 BT 下載任務 | 8002 |
| Viewer (Jellyfin) | 檔案同步與組織 | 8003 |

外部依賴：
- **PostgreSQL** - 資料庫
- **qBittorrent** - BT 下載客戶端
- **Jellyfin** - 媒體伺服器（可選）

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

# 查看日誌
docker compose logs -f core-service
```

## 服務端點

### Core Service (8000)

| Method | Endpoint | 說明 |
|--------|----------|------|
| GET | `/health` | 健康檢查 |
| GET | `/anime` | 列出動畫 |
| GET | `/subscriptions` | 列出訂閱 |
| POST | `/subscriptions` | 新增訂閱 |
| GET | `/services` | 列出已註冊服務 |

### Fetcher (8001)

| Method | Endpoint | 說明 |
|--------|----------|------|
| GET | `/health` | 健康檢查 |
| POST | `/fetch` | 執行 RSS 抓取 |

### Downloader (8002)

| Method | Endpoint | 說明 |
|--------|----------|------|
| GET | `/health` | 健康檢查 |
| POST | `/download` | 新增下載任務 |

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
| `QBITTORRENT_URL` | http://qbittorrent:8080 | qBittorrent WebUI |
| `RUST_LOG` | info | 日誌等級 |

### Docker Compose 檔案

| 檔案 | 用途 |
|------|------|
| `docker-compose.yaml` | 主要服務配置 |
| `docker-compose.override.yaml` | qBittorrent + Jellyfin |
| `docker-compose.dev.yaml` | 開發環境（見 DEVELOPMENT.md） |

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
```

## 維運指南

### 日誌查看

```bash
# 所有服務
docker compose logs -f

# 特定服務
docker compose logs -f core-service
docker compose logs -f downloader-qbittorrent
```

### 資料庫備份

```bash
# 備份
docker compose exec postgres pg_dump -U bangumi bangumi > backup.sql

# 還原
docker compose exec -T postgres psql -U bangumi bangumi < backup.sql
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

## 開發

開發環境設置請參考 [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)

## 許可證

MIT
