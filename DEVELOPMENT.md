# 開發指南

本文件說明如何在本地開發環境中設置和運行 rustBangumi 專案。

## 目錄

- [開發環境設置](#開發環境設置)
- [專案結構](#專案結構)
- [啟動開發環境](#啟動開發環境)
- [運行服務](#運行服務)
- [測試](#測試)
- [編碼規範](#編碼規範)
- [資料庫操作](#資料庫操作)
- [Frontend 開發](#frontend-開發)
- [常見問題](#常見問題)

---

## 開發環境設置

### 前置條件

- **Rust 1.75+**
- **Bun**（Frontend 開發）
- **Docker & Docker Compose v2+**
- **diesel_cli**（資料庫遷移）

### 安裝 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update
```

### 安裝開發工具

```bash
# Diesel CLI（僅需 postgres feature）
cargo install diesel_cli --no-default-features --features postgres

# 選用：檔案監視與自動重新編譯
cargo install cargo-watch
```

---

## 專案結構

```
rustBangumi/
├── Cargo.toml                          # Workspace 配置
├── shared/                             # 共享型別（DownloaderClient trait 等）
├── core-service/                       # 核心服務（協調器）
│   ├── src/
│   │   ├── main.rs                     # Axum 路由、AppState
│   │   ├── handlers/                   # HTTP 處理器（每功能一個模組）
│   │   ├── services/                   # 業務邏輯（Scheduler、WebhookService 等）
│   │   ├── models/                     # Diesel 模型
│   │   └── db/                         # Repository 層
│   └── migrations/                     # Diesel 遷移檔
├── fetchers/mikanani/                  # Mikanani RSS Fetcher
├── downloaders/qbittorrent/            # qBittorrent Downloader
├── downloaders/pikpak/                 # PikPak Downloader（雲端離線 + streaming）
├── viewers/jellyfin/                   # Jellyfin Viewer（檔案整理 + NFO metadata）
├── metadata/                           # Metadata Service（bangumi.tv 查詢）
├── frontend/                           # React SPA 管理介面
│   ├── src/
│   │   ├── pages/                      # 頁面元件
│   │   ├── components/                 # 共用元件（Shadcn/UI）
│   │   ├── services/CoreApi.ts         # Effect.js API 介面定義
│   │   ├── layers/ApiLayer.ts          # Effect.js Layer（HTTP 實作）
│   │   ├── schemas/                    # 型別定義
│   │   └── runtime/AppRuntime.ts       # ManagedRuntime 初始化
│   ├── Dockerfile                      # 多階段建構（Bun + Caddy）
│   └── Caddyfile                       # 反向代理設定
├── cli/                                # CLI 工具
├── docker-compose.yaml                 # 生產環境
├── docker-compose.dev.yaml             # 開發環境 ← 使用這個
├── docker-compose.override.yaml        # 生產環境可選服務
└── docs/                               # 文檔
```

---

## 啟動開發環境

### 1. 啟動基礎設施

```bash
docker compose -f docker-compose.dev.yaml up -d

# 確認服務已就緒
docker compose -f docker-compose.dev.yaml ps
```

開發環境包含的服務：

| 服務 | URL | 說明 |
|------|-----|------|
| PostgreSQL | `localhost:5432` | 資料庫（`bangumi` + `viewer_jellyfin`） |
| Adminer | `http://localhost:8081` | 資料庫管理介面 |
| qBittorrent | `http://localhost:8080` | BT 下載客戶端（admin/adminadmin） |
| Jellyfin | `http://localhost:8096` | 媒體伺服器（首次需完成設定嚮導） |

> **DOOD 環境**（在容器內透過 `/var/run/docker.sock` 使用 host Docker）：需設定 `HOST_PROJECT_PATH` 環境變數指向 host 上的專案路徑。

### 2. 設置環境變數

```bash
cp .env.dev .env
```

`.env.dev` 關鍵變數：

```env
# Core 資料庫
DATABASE_URL=postgresql://bangumi:bangumi_dev_password@localhost:5432/bangumi

# Viewer 獨立資料庫
VIEWER_DATABASE_URL=postgresql://bangumi:bangumi_dev_password@localhost:5432/viewer_jellyfin

# 服務通信
CORE_SERVICE_URL=http://localhost:8000
QBITTORRENT_URL=http://localhost:8080
QBITTORRENT_USER=admin
QBITTORRENT_PASSWORD=adminadmin

# 本地開發用暫存目錄
DOWNLOADS_DIR=/tmp/bangumi-downloads
JELLYFIN_LIBRARY_DIR=/tmp/bangumi-media

RUST_LOG=debug
```

### 3. 執行資料庫遷移

```bash
cd core-service
diesel migration run
```

Viewer 資料庫（`viewer_jellyfin`）的遷移內嵌於 binary，啟動時自動執行。

---

## 運行服務

### 各服務啟動指令

```bash
# Core Service (port 8000)
cargo run -p core-service

# Fetcher - Mikanani (port 8001)
cargo run -p fetcher-mikanani

# Downloader - qBittorrent (port 8002)
cargo run -p downloader-qbittorrent

# Downloader - PikPak (port 8006)
cargo run -p downloader-pikpak

# Viewer - Jellyfin (port 8003)
DATABASE_URL=postgresql://bangumi:bangumi_dev_password@localhost:5432/viewer_jellyfin \
  cargo run -p viewer-jellyfin

# Metadata Service (port 8005，stateless)
CORE_SERVICE_URL=http://localhost:8000 \
SERVICE_HOST=localhost \
SERVICE_PORT=8005 \
  cargo run -p metadata-service
```

### 監視模式（自動重新編譯）

```bash
cargo watch -x 'run -p core-service'
```

---

## 測試

```bash
# 全部測試
cargo test

# 特定套件
cargo test -p core-service
cargo test -p downloader-qbittorrent
cargo test -p viewer-jellyfin

# 顯示標準輸出
cargo test -- --nocapture

# 特定測試名稱
cargo test <test_name>
```

---

## 編碼規範

```bash
# 格式化
cargo fmt

# Lint 檢查
cargo clippy

# 確認編譯
cargo check
```

### 函數式設計原則

優先使用 `map`、`filter_map`、`and_then`、`or_else` 等組合器，避免過多 `match` 與可變狀態。詳見 [CLAUDE.md](CLAUDE.md) 的函數式設計原則章節。

### 錯誤處理

使用 `anyhow` 處理應用層錯誤，`thiserror` 定義領域錯誤型別：

```rust
use anyhow::{anyhow, Result};

async fn fetch_data() -> Result<String> {
    let resp = client.get(url).send().await
        .map_err(|e| anyhow!("fetch failed: {}", e))?;
    Ok(resp.text().await?)
}
```

### 日誌

```rust
tracing::debug!("parsing item: {:?}", item);
tracing::info!(subscription_id, "fetch completed");
tracing::warn!("downloader not registered");
tracing::error!("database error: {}", e);
```

---

## 資料庫操作

| 資料庫 | 用途 | 使用者 |
|--------|------|--------|
| `bangumi` | Core 主資料庫（動畫、訂閱、下載、Webhook 等） | Core Service |
| `viewer_jellyfin` | Viewer 資料庫（同步任務、bangumi.tv metadata 快取） | Viewer Jellyfin |

### 常用 Diesel 指令

```bash
# 建立新遷移（在 core-service 目錄執行）
diesel migration generate <name>

# 執行遷移
diesel migration run

# 回滾
diesel migration revert

# 回滾後重新執行（驗證 down.sql 正確性）
diesel migration redo
```

### Adminer 連線

開啟 `http://localhost:8081`，輸入：
- Server: `postgres`
- Username: `bangumi`
- Password: `bangumi_dev_password`
- Database: `bangumi` 或 `viewer_jellyfin`

---

## Frontend 開發

### 技術棧

| 項目 | 說明 |
|------|------|
| React 19 + TypeScript + Vite 7 | 主框架 |
| Effect.js | API 呼叫、型別安全的錯誤處理、依賴注入 |
| Shadcn/UI | Radix UI + Tailwind CSS 元件庫（New York 風格） |
| i18next | 多語系支援（繁中 / 英文 / 日文） |
| Caddy | 生產環境反向代理與靜態檔案服務 |

### 安裝與啟動

```bash
cd frontend
bun install
bun run dev   # 開發伺服器 port 8004
```

開發伺服器的 API 代理（`vite.config.ts`）：

| URL 前綴 | 代理至 |
|----------|--------|
| `/api/core/` | `http://localhost:8000` |
| `/api/downloader/` | `http://localhost:8002` |
| `/api/downloader-pikpak/` | `http://localhost:8006` |
| `/api/viewer/` | `http://localhost:8003` |

> 啟動前端開發伺服器前，請確認後端服務已啟動。

### 建構

```bash
bun run build    # TypeScript 型別檢查 + 生產建構
bun run preview  # 預覽建構結果
```

### 頁面一覽

| 路由 | 頁面 | 功能 |
|------|------|------|
| `/` | Dashboard | 服務健康狀態、系統統計 |
| `/search` | 搜尋資源 | 線上搜尋動畫 RSS 資源 |
| `/subscriptions` | 訂閱管理 | RSS 訂閱 CRUD、篩選規則、建立精靈 |
| `/anime` | 動畫系列 | 系列管理、集數、AnimeLink |
| `/raw-items` | 最新更新 | 原始 RSS 項目（狀態篩選、分頁） |
| `/pending` | 待確認 | AI 生成結果的人工審核佇列 |
| `/settings` | 設定 | 下載器優先級、AI 設定、Prompt 設定、Webhook |
| `/anime-works` | 動畫作品 | 動畫作品 CRUD |
| `/subtitle-groups` | 字幕組 | 字幕組管理 |
| `/parsers` | 解析器 | Title Parser CRUD + 即時解析預覽 |
| `/filters` | 篩選器 | 全域 Filter 規則 + before/after 預覽 |

### 新增 API 端點

1. 在 `src/services/CoreApi.ts` 的 interface 中新增方法宣告
2. 在 `src/layers/ApiLayer.ts` 中實作對應的 HTTP 呼叫
3. 若有新型別，在 `src/schemas/` 下建立對應 interface

---

## 常見問題

### Port 已被佔用

```bash
lsof -i :8000   # 同樣適用於 8001, 8002, 8003, 8004, 5432
docker compose -f docker-compose.dev.yaml down
```

### 資料庫連接失敗

```bash
docker compose -f docker-compose.dev.yaml ps postgres
docker compose -f docker-compose.dev.yaml logs postgres
```

### 清理開發環境

```bash
# 停止服務（保留資料）
docker compose -f docker-compose.dev.yaml down

# 停止並刪除所有資料
docker compose -f docker-compose.dev.yaml down -v

# 清理 Rust 建構快取
cargo clean
```

---

## 相關文檔

| 文件 | 說明 |
|------|------|
| [docs/API-SPECIFICATIONS.md](docs/API-SPECIFICATIONS.md) | 完整 API 規格 |
| [docs/api/openapi.yaml](docs/api/openapi.yaml) | OpenAPI 3.0 規格（v0.3.0） |
| [docs/ARCHITECTURE_RSS_SUBSCRIPTIONS.md](docs/ARCHITECTURE_RSS_SUBSCRIPTIONS.md) | RSS 訂閱架構 |
| [docs/CORS-CONFIGURATION.md](docs/CORS-CONFIGURATION.md) | CORS 設定說明 |
| [viewers/jellyfin/README.md](viewers/jellyfin/README.md) | Viewer Jellyfin 詳細文件 |
