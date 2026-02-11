# 開發指南

本文件說明如何在本地開發環境中設置和運行 Rust Bangumi 專案。

## 目錄

- [開發環境設置](#開發環境設置)
- [專案結構](#專案結構)
- [啟動開發環境](#啟動開發環境)
- [運行服務](#運行服務)
- [測試](#測試)
- [編碼規範](#編碼規範)
- [資料庫操作](#資料庫操作)
- [常見問題](#常見問題)

---

## 開發環境設置

### 前置條件

- **Rust 1.75+**
- **Node.js 22+**（Frontend 開發）
- **Docker & Docker Compose v2+**
- **PostgreSQL client** (可選，用於 CLI 操作)

### 安裝 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update
```

### 安裝開發工具

```bash
# Diesel CLI（資料庫遷移）
cargo install diesel_cli --no-default-features --features postgres

# 開發輔助工具（可選）
cargo install cargo-watch    # 檔案監視
cargo install cargo-tarpaulin  # 測試覆蓋率
```

---

## 專案結構

```
rust-bangumi/
├── Cargo.toml                          # Workspace 配置
├── shared/                             # 共享庫（API types、models）
├── core-service/                       # 核心服務（協調器）
│   └── src/
│       ├── main.rs
│       ├── handlers/                   # HTTP 處理器
│       ├── services/                   # 業務邏輯（FetchScheduler, DownloadScheduler）
│       └── db/                         # 數據庫操作
├── fetchers/mikanani/                  # Mikanani RSS Fetcher
├── downloaders/qbittorrent/            # qBittorrent Downloader
├── viewers/jellyfin/                   # Jellyfin Viewer（檔案同步 + NFO metadata）
├── frontend/                           # React SPA 前端管理介面
│   ├── src/
│   │   ├── pages/                      # 頁面元件
│   │   ├── components/                 # UI 元件（Shadcn/UI + 共用元件）
│   │   ├── services/                   # Effect-TS API 服務層
│   │   ├── schemas/                    # Effect Schema 型別定義
│   │   └── hooks/                      # React hooks
│   ├── Dockerfile                      # 多階段建構（Node + Caddy）
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

開發環境包含：PostgreSQL、Adminer、qBittorrent、Jellyfin

```bash
# 啟動所有開發服務
docker compose -f docker-compose.dev.yaml up -d

# 檢查狀態
docker compose -f docker-compose.dev.yaml ps
```

**DOOD 環境（在容器內使用 Docker）：**

如果你在容器內透過掛載 `/var/run/docker.sock` 使用 host Docker，需要設定 `HOST_PROJECT_PATH`：

```bash
# 設定 host 上的專案路徑
export HOST_PROJECT_PATH=/path/to/rust-bangumi/on/host

# 然後啟動服務
docker compose -f docker-compose.dev.yaml up -d
```

**服務端點：**

| 服務 | URL | 說明 |
|------|-----|------|
| PostgreSQL | `localhost:5432` | 資料庫（`bangumi` + `viewer_jellyfin`） |
| Adminer | `http://localhost:8081` | 資料庫管理介面 |
| qBittorrent | `http://localhost:8080` | BT 下載客戶端（admin/adminadmin） |
| Jellyfin | `http://localhost:8096` | 媒體伺服器（首次需設定嚮導） |

### 2. 設置環境變數

```bash
# 複製開發環境模板
cp .env.dev .env

# DOOD 環境需編輯 .env 設定 HOST_PROJECT_PATH
```

`.env.dev` 關鍵變數：
```env
# DOOD 環境必填（host 上的專案絕對路徑）
HOST_PROJECT_PATH=/path/to/rust-bangumi

# Core 資料庫
DATABASE_URL=postgresql://bangumi:bangumi_dev_password@localhost:5432/bangumi

# Viewer 獨立資料庫
VIEWER_DATABASE_URL=postgresql://bangumi:bangumi_dev_password@localhost:5432/viewer_jellyfin

# 服務通信
CORE_SERVICE_URL=http://localhost:8000
QBITTORRENT_URL=http://localhost:8080
QBITTORRENT_USER=admin
QBITTORRENT_PASSWORD=adminadmin

# Viewer 目錄（本地開發用暫存目錄）
DOWNLOADS_DIR=/tmp/bangumi-downloads
JELLYFIN_LIBRARY_DIR=/tmp/bangumi-media

RUST_LOG=debug
```

### 3. 執行資料庫遷移

```bash
# Core 資料庫遷移
diesel migration run

# Viewer 資料庫會在 viewer-jellyfin 啟動時自動遷移，無需手動操作
# 但需先建立資料庫（docker-compose.dev.yaml 的 init script 會自動建立）
```

---

## 運行服務

### 運行單一服務

```bash
# Core Service (port 8000)
cargo run -p core-service

# Fetcher (port 8001)
cargo run -p fetcher-mikanani

# Downloader (port 8002)
cargo run -p downloader-qbittorrent

# Viewer (port 8003) - 需指定 Viewer 專用資料庫
DATABASE_URL=postgresql://bangumi:bangumi_dev_password@localhost:5432/viewer_jellyfin \
  cargo run -p viewer-jellyfin

# CLI
cargo run -p bangumi-cli -- --help
```

### 監視模式（自動重新編譯）

```bash
# 監視並自動重新運行
cargo watch -x 'run -p core-service'

# 監視並運行測試
cargo watch -x 'test -p core-service'
```

### 同時運行多個服務

開多個終端分別執行，或使用 `tmux`：

```bash
# 終端 1 - Core
cargo run -p core-service

# 終端 2 - Fetcher
cargo run -p fetcher-mikanani

# 終端 3 - Downloader
cargo run -p downloader-qbittorrent

# 終端 4 - Viewer
DATABASE_URL=postgresql://bangumi:bangumi_dev_password@localhost:5432/viewer_jellyfin \
  cargo run -p viewer-jellyfin
```

---

## 測試

### 運行測試

```bash
# 所有測試
cargo test

# 特定套件
cargo test -p core-service
cargo test -p downloader-qbittorrent
cargo test -p viewer-jellyfin

# 帶輸出
cargo test -- --nocapture

# 特定測試
cargo test test_name

# 集成測試
cargo test --test integration_test
```

### 測試覆蓋率

```bash
cargo tarpaulin --out Html
open tarpaulin-report.html
```

### 測試開發環境連線

```bash
# 測試 PostgreSQL
psql postgresql://bangumi:bangumi_dev_password@localhost:5432/bangumi -c "SELECT 1"

# 測試 qBittorrent
curl -X POST http://localhost:8080/api/v2/auth/login \
  -d "username=admin&password=adminadmin"

# 測試 Core Service
curl http://localhost:8000/health

# 測試 Downloader
curl http://localhost:8002/health

# 測試 Viewer
curl http://localhost:8003/health

# 測試 Jellyfin
curl http://localhost:8096/health

# 手動觸發 Viewer 同步（測試用）
mkdir -p /tmp/bangumi-downloads
echo "test" > /tmp/bangumi-downloads/test.mkv
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
```

---

## 編碼規範

### Rust 風格

```bash
# 格式化
cargo fmt

# Lint 檢查
cargo clippy

# 檢查編譯
cargo check
```

### 命名慣例

| 類型 | 風格 | 範例 |
|------|------|------|
| 模塊 | snake_case | `qbittorrent_client` |
| 類型 | PascalCase | `TorrentInfo` |
| 函數 | snake_case | `get_torrent_info` |
| 常數 | SCREAMING_SNAKE_CASE | `MAX_RETRIES` |

### 錯誤處理

使用 `anyhow` 和 `thiserror`：

```rust
use anyhow::{anyhow, Result};

async fn my_function() -> Result<String> {
    let data = fetch_data()
        .await
        .map_err(|e| anyhow!("Failed to fetch: {}", e))?;
    Ok(data)
}
```

### 日誌

使用 `tracing`：

```rust
tracing::debug!("Debug message");
tracing::info!("Info message");
tracing::warn!("Warning");
tracing::error!("Error: {}", e);
```

---

## 資料庫操作

### 資料庫說明

| 資料庫 | 用途 | 使用者 |
|--------|------|--------|
| `bangumi` | Core 主資料庫（動畫、訂閱、下載等） | Core Service |
| `viewer_jellyfin` | Viewer 獨立資料庫（同步任務、bangumi.tv metadata 快取） | Viewer Jellyfin |

### 創建遷移

```bash
diesel migration generate my_migration_name
```

### 執行遷移

```bash
# 執行 Core 資料庫遷移
diesel migration run

# Viewer 資料庫遷移（內嵌於 binary，啟動時自動執行）

# 回滾
diesel migration revert

# 重做（回滾後執行）
diesel migration redo
```

### 使用 Adminer

1. 開啟 `http://localhost:8081`
2. 選擇 PostgreSQL
3. 輸入：
   - Server: `postgres`（Docker 內）或 `localhost`（本地）
   - Username: `bangumi`
   - Password: `bangumi_dev_password`
   - Database: `bangumi` 或 `viewer_jellyfin`

---

## 常見問題

### Port 已被佔用

```bash
# 找出佔用 port 的程式
lsof -i :8000
lsof -i :5432
lsof -i :8080
lsof -i :8096

# 停止佔用的服務
docker compose -f docker-compose.dev.yaml down
```

### 資料庫連接失敗

```bash
# 確認 PostgreSQL 運行中
docker compose -f docker-compose.dev.yaml ps postgres

# 重啟 PostgreSQL
docker compose -f docker-compose.dev.yaml restart postgres

# 檢查日誌
docker compose -f docker-compose.dev.yaml logs postgres
```

### qBittorrent 無法連接

```bash
# 檢查服務狀態
docker compose -f docker-compose.dev.yaml ps qbittorrent

# 查看日誌
docker compose -f docker-compose.dev.yaml logs qbittorrent

# 測試 WebUI
curl http://localhost:8080
```

如果出現認證錯誤，請確認使用 `admin` / `adminadmin` 登入。

### Jellyfin 無法連接

```bash
# 檢查服務狀態
docker compose -f docker-compose.dev.yaml ps jellyfin

# 查看日誌
docker compose -f docker-compose.dev.yaml logs jellyfin

# 測試健康檢查
curl http://localhost:8096/health
```

首次啟動需完成 Jellyfin 設定嚮導（http://localhost:8096）。

### 清理開發環境

```bash
# 停止服務
docker compose -f docker-compose.dev.yaml down

# 停止並刪除資料
docker compose -f docker-compose.dev.yaml down -v

# 清理 Rust build
cargo clean
```

---

## 開發流程

### 建議的開發流程

1. **創建 feature branch**
   ```bash
   git checkout -b feature/my-feature
   ```

2. **啟動開發環境**
   ```bash
   docker compose -f docker-compose.dev.yaml up -d
   ```

3. **開發與測試**
   ```bash
   cargo watch -x 'test -p my-package'
   ```

4. **格式化與檢查**
   ```bash
   cargo fmt
   cargo clippy
   cargo test
   ```

5. **提交**
   ```bash
   git add .
   git commit -m "feat: add my feature"
   ```

### 使用 Git Worktree 隔離開發

對於大型功能開發，建議使用 worktree：

```bash
# 創建 worktree
git worktree add .worktrees/my-feature -b feature/my-feature

# 進入 worktree
cd .worktrees/my-feature

# 完成後清理
git worktree remove .worktrees/my-feature
```

---

## Frontend 開發

### 技術棧

- **React 19** + TypeScript + Vite 7
- **Effect-TS** — 類型安全的 API 呼叫與錯誤處理
- **Shadcn/UI** — Radix UI + Tailwind CSS 元件庫（New York 風格）
- **Caddy** — 生產環境反向代理

### 安裝與啟動

```bash
cd frontend

# 安裝依賴
npm install

# 啟動開發伺服器（port 5173）
npm run dev
```

開發伺服器會自動代理 API 請求：

| URL 前綴 | 代理至 |
|----------|--------|
| `/api/core/` | `http://localhost:8000` |
| `/api/downloader/` | `http://localhost:8002` |
| `/api/viewer/` | `http://localhost:8003` |

> 確保後端服務已啟動後再開啟前端開發伺服器。

### 建構

```bash
cd frontend

# TypeScript 檢查 + 建構生產版本
npm run build

# 預覽建構結果
npm run preview
```

### 頁面一覽

| 路由 | 頁面 | 功能 |
|------|------|------|
| `/` | Dashboard | 服務健康狀態、系統總覽 |
| `/anime` | Anime 管理 | 新增/刪除動畫 |
| `/anime/:id` | Anime 詳情 | 系列管理、Filter 規則 |
| `/subscriptions` | 訂閱管理 | 瀏覽 RSS 訂閱 |
| `/raw-items` | Raw Items | 原始 RSS 項目（含狀態篩選、分頁） |
| `/downloads` | 下載管理 | 下載進度（自動 5 秒刷新） |
| `/filters` | Filter 規則 | CRUD + 即時 before/after 預覽 |
| `/parsers` | Title Parser | CRUD + 即時解析預覽 |
| `/conflicts` | 衝突解決 | Fetcher 衝突解決 |

### 前端架構

```
src/
├── services/CoreApi.ts    # Effect.Context.Tag API 介面（17 個端點）
├── schemas/               # Effect Schema 型別驗證
├── hooks/                 # useEffectQuery / useEffectMutation
├── layers/ApiLayer.ts     # Effect-TS Layer（HttpClient → CoreApi）
├── runtime/AppRuntime.ts  # ManagedRuntime 初始化
├── components/
│   ├── ui/                # Shadcn/UI 元件（15 個）
│   ├── layout/            # AppLayout, Sidebar, Header
│   └── shared/            # DataTable, StatusBadge, ConfirmDialog, RegexInput
└── pages/                 # 各功能頁面
```

### Docker 建構

```bash
# 單獨建構 Frontend 映像
docker compose build frontend

# 啟動 Frontend（依賴 core-service）
docker compose up -d frontend
```

Frontend Docker 映像使用多階段建構：
1. **Builder**: Node 22 Alpine — `npm ci` + `npm run build`
2. **Runtime**: Caddy Alpine — 提供靜態檔案 + 反向代理

---

## 相關文檔

- [API 規格](docs/API-SPECIFICATIONS.md)
- [架構設計](docs/plans/2025-01-21-rust-bangumi-architecture-design.md)
- [CORS 設定](docs/CORS-CONFIGURATION.md)
- [Viewer Jellyfin](viewers/jellyfin/README.md)
- [Frontend 實作計劃](docs/plans/frontend-implementation-plan.md)
