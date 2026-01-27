# Rust Bangumi - 動畫 RSS 聚合與下載管理系統

這是一個使用 Rust + PostgreSQL 構建的微服務架構動畫 RSS 聚合、下載與媒體庫管理系統。

## 系統概述

系統由 5 個主要微服務組成，各自獨立部署在 Docker 容器中：

1. **核心服務 (Core Service)** - 主協調器，管理數據庫、調度任務
2. **擷取區塊 (Fetcher)** - 從 RSS/爬蟲取數據（支持多個實例）
3. **下載區塊 (Downloader)** - 執行下載任務（支持多個實例）
4. **顯示區塊 (Viewer)** - 文件同步與組織（通常一個實例）
5. **CLI 工具** - 用戶交互界面

詳細架構設計見 `docs/plans/2025-01-21-rust-bangumi-architecture-design.md`

## 項目結構

```
rust-bangumi/
├── Cargo.toml                          # Workspace 配置
├── shared/                             # 共享庫
│   ├── src/
│   │   ├── lib.rs
│   │   ├── models.rs                   # 共享數據結構
│   │   ├── errors.rs                   # 錯誤類型
│   │   └── api.rs                      # API 常數
├── core-service/                       # 核心服務
│   ├── src/
│   │   ├── main.rs
│   │   ├── config.rs
│   │   ├── handlers/                   # HTTP 處理器
│   │   ├── services/                   # 業務邏輯
│   │   ├── models/
│   │   └── db/                         # 數據庫操作
├── fetchers/
│   └── mikanani/                       # Mikanani RSS 擷取區塊
│       ├── src/
│       │   ├── main.rs
│       │   ├── handlers.rs
│       │   └── rss_parser.rs
├── downloaders/
│   └── qbittorrent/                    # qBittorrent 下載區塊
│       ├── src/
│       │   ├── main.rs
│       │   ├── handlers.rs
│       │   └── qbittorrent_client.rs
├── viewers/
│   └── jellyfin/                       # Jellyfin 顯示區塊
│       ├── src/
│       │   ├── main.rs
│       │   ├── handlers.rs
│       │   └── file_organizer.rs
├── cli/                                # CLI 工具
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands.rs
│   │   └── client.rs
├── docker-compose.yaml                 # 生產環境 Docker 編排
├── docker-compose.dev.yaml             # 開發環境（僅 PostgreSQL + Adminer）
├── Dockerfile.*                        # 各服務 Dockerfile
├── .env.example                        # 環境變數範例
├── .env.dev                            # 開發環境配置模板
├── .env.prod                           # 生產環境配置模板
└── docs/
    └── plans/
        └── 2025-01-21-*-design.md      # 架構設計文檔
```

## 快速開始

### 前置條件

- Rust 1.75+ (或使用 Docker)
- Docker & Docker Compose
- PostgreSQL 15+ (或使用 docker compose 自動拉起)

### 使用 Docker Compose 啟動

```bash
# 構建所有服務
docker compose build

# 啟動所有服務
docker compose up -d

# 查看日誌
docker compose logs -f

# 停止所有服務
docker compose down
```

所有服務將在以下端口監聽：
- Core Service: `http://localhost:8000`
- Fetcher (Mikanani): `http://localhost:8001`
- Downloader (qBittorrent): `http://localhost:8002`
- Viewer (Jellyfin): `http://localhost:8003`

### 本地開發

```bash
# 1. 啟動開發數據庫（PostgreSQL + Adminer）
docker compose -f docker-compose.dev.yaml up -d

# 2. 使用開發環境配置（已預設，或複製模板）
cp .env.dev .env

# 3. 啟動服務（各開一個終端）
cargo run -p core-service
cargo run -p fetcher-mikanani

# 4. 運行 CLI
cargo run -p bangumi-cli -- --help
```

**開發環境服務：**
- PostgreSQL: `localhost:5432`
- Adminer (DB 管理介面): `http://localhost:8081`

### 環境配置

| 檔案 | 用途 |
|------|------|
| `.env` | 當前使用的配置（已在 .gitignore） |
| `.env.dev` | 開發環境模板（cargo run） |
| `.env.prod` | 生產環境模板（docker compose） |
| `.env.example` | 完整變數說明 |

**關鍵環境變數：**

| 變數 | 開發環境 | 生產環境 (Docker) |
|------|----------|-------------------|
| `DATABASE_URL` | `...@localhost:5432/...` | 由 docker-compose 自動構建 |
| `CORE_SERVICE_URL` | `http://localhost:8000` | `http://core-service:8000` |
| `SERVICE_HOST` | `localhost` | 不需設定（使用容器名） |

## CLI 使用範例

```bash
# 訂閱 RSS
cargo run --package bangumi-cli -- subscribe \
  "https://mikanani.me/RSS/Classic" \
  --fetcher mikanani

# 列出動畫
cargo run --package bangumi-cli -- list --season 2025/冬

# 添加過濾規則
cargo run --package bangumi-cli -- filter add \
  1 "HorribleSubs" \
  positive "1080p"

# 查看狀態
cargo run --package bangumi-cli -- status
```

## API 端點

### 核心服務

- `POST /services/register` - 服務註冊
- `GET /services` - 列出所有服務
- `GET /anime` - 列出動畫
- `POST /filters` - 添加過濾規則
- `GET /health` - 健康檢查

詳細 API 文檔見架構設計文檔

## 數據庫架構

核心表結構：
- `seasons` - 季度（冬/春/夏/秋）
- `animes` - 動畫基本信息
- `anime_series` - 動畫季數
- `subtitle_groups` - 字幕組
- `anime_links` - 動畫下載連結
- `filter_rules` - 過濾規則
- `downloads` - 下載記錄
- `cron_logs` - Cron 執行日誌

詳見架構設計文檔的 Section 2

## 開發進展

### 已完成
- [x] 架構設計與驗證
- [x] 項目結構與 Cargo workspace 設置
- [x] 共享庫 (shared) 實現
- [x] Docker & Docker Compose 配置

### 進行中
- [ ] 數據庫 schema 與 migrations
- [ ] 核心服務完整實現
- [ ] 各擷取/下載/顯示區塊實現
- [ ] CLI 工具完整實現
- [ ] 單元測試與集成測試

### 計劃中
- [ ] Web UI 前端
- [ ] 更多擷取源支持
- [ ] 更多下載器支持
- [ ] 通知功能（郵件/Telegram）
- [ ] 性能優化與監控

## 許可證

MIT

## 開發聯絡

如有問題或建議，歡迎提交 Issue 或 PR
