# Rust Bangumi 開發指引

## 項目概覽

**Rust Bangumi** 是一個完整的動畫內容管理系統，從 RSS 訂閱到下載、整理、媒體伺服器同步的全端點解決方案。

### 核心流程
```
RSS 訂閱 → Fetcher 爬取 → Core 解析標題 → 自動下載派送 → Viewer 同步至 Jellyfin
```

### 微服務架構
- **Core** (port 8000): 主服務，API 樞紐，數據管理，調度
- **Fetcher** (port 8001): Mikanani RSS 爬取，返回 raw items
- **Downloader** (port 8002): qBittorrent + PikPak 客戶端，支援 magnet/torrent
- **Viewer** (port 8003): Jellyfin 同步，NFO 生成，bangumi.tv metadata
- **CLI** (binary): 命令行工具，開發與運維

## 技術棧

| 層級 | 選擇 |
|------|------|
| Web Framework | Axum (tokio-based) |
| ORM | Diesel 2.1 + r2d2 (connection pool) |
| Database | PostgreSQL 15+ |
| Async Runtime | Tokio |
| Logging | Tracing |
| HTTP Client | reqwest |

## 代碼組織

```
├── core-service/           # 主服務 (Axum + Diesel)
│   ├── handlers/           # 請求處理層
│   ├── services/           # 業務邏輯層
│   ├── schema.rs           # Diesel schema (自動生成)
│   ├── models/             # 數據模型
│   └── migrations/         # Diesel 遷移
├── shared/                 # 共用 crate
│   ├── lib.rs              # 導出公共類型 (DownloaderClient trait 等)
├── fetcher-service/        # Mikanani RSS 爬取器
├── downloader-service/     # qBittorrent + PikPak 下載器
├── viewer-service/         # Jellyfin 同步
├── bangumi-cli/            # CLI 工具
└── docs/                   # 架構、API 規格、計劃
```

## 函數式設計原則

實作盡可能遵守函數式編程：優先使用 `map`, `filter`, `fold`, `Option`/`Result` 組合器而非 `match` 和可變狀態。

**關鍵實踐**：
- 使用迭代器鏈而非 `for` 迴圈
- 鏈式組合 `Option`/`Result` (`and_then`, `map`, `or_else`) 避免多層 `match`
- `filter_map` 同時過濾和轉換，減少中間集合
- 實現 `From`/`Into` traits 進行聲明式轉換
- 將複雜邏輯拆分為可組合的單元函數

**在項目中的應用**：RSS 解析 (`and_then` 鏈), 衝突解析 (`filter_map`), 訂閱列表轉換 (`map` 管道)

**例外**：複雜邏輯流控制或副作用密集時，命令式代碼更清晰。優先函數式，但要以可讀性為最高目標。

---

## 核心設計模式

### 1. Trait-Based Abstraction
```rust
// shared crate 中定義，被多個服務實現
pub trait DownloaderClient {
    async fn add_torrent(&self, ...) -> Result<String>;
    // ...
}

// qBittorrent + PikPak 各自實現此 trait
// 測試用 MockDownloaderClient（公開，非 #[cfg(test)]）
```

**原則**：在 `shared` crate 定義，讓多個服務共用介面。

### 2. HTTP 客戶端設計
- **Core** 向 Fetcher、Downloader、Viewer 發起 HTTP 請求
- 用 reqwest 異步 client，支援重試
- 202 Accepted 用於長時間操作（併回呼）

### 3. 數據流：Raw → Parsed
- **Raw Items**：Fetcher 返回的原始 RSS item（標題、URL、日期）
- **Parsed**：Core 用正則解析器提取 anime/episode/group 信息
- **Title Parsers**：可配置的正則引擎，可新增/修改解析規則

### 4. Soft Delete
- 大多表有 `is_active` / `soft_deleted_at` 欄位
- 優先使用 soft delete，保留歷史記錄

## 開發工作流

### 環境設置
```bash
# 啟動開發環境（含 PostgreSQL, Adminer, qBittorrent, Jellyfin）
docker-compose -f docker-compose.dev.yaml up -d

# 執行 Diesel 遷移（首次或新遷移時）
cd core-service
diesel migration run

# 運行各服務（各自開終端）
cargo run --bin core-service
cargo run --bin fetcher-service
cargo run --bin downloader-service
cargo run --bin viewer-service
```

### 添加新功能的流程

1. **查看 docs/**：了解架構、API 規格、計劃
2. **檢查 schema.rs**：確認數據表結構
3. **編寫 handler**：Axum 路由 + 請求/響應
4. **添加 service 邏輯**：業務邏輯與數據庫交互
5. **撰寫測試**：
   - Unit tests (模型層)
   - Integration tests (handler 層，使用 MockDownloaderClient)
   - 注意：handler 邏輯無法直接在 integration tests 中重用（二進制 crate 限制）

### 測試策略

- **Unit 測試**：services/, models/ 層級，不涉及 HTTP
- **Integration 測試**：`tests/integration/` 目錄
  - handler 邏輯需要在測試中複製（或移到 library crate）
  - Mock trait 實現：`MockDownloaderClient`（公開可用）
  - 使用 `tokio::test` 進行異步測試

### Diesel 遷移

```bash
# 在 core-service 目錄
diesel migration generate feature_name
# 編輯 up.sql / down.sql
diesel migration run
diesel migration redo  # 測試 down 邏輯
```

新表結構後，Diesel 自動生成 schema.rs 和模型。

## 常見模式

### API 錯誤處理

使用統一的錯誤響應格式：
```json
{
  "error": "error_code",
  "message": "人類可讀的訊息"
}
```

常見錯誤碼：`not_found`, `duplicate_url`, `invalid_fetcher`, `database_error`, `conflict`

### 廣播機制

Core 用 `tokio::sync::broadcast` 通知服務：
- 新訂閱事件
- 下載完成事件
- 訂閱衝突事件

### 調度系統

- **FetchScheduler**：定期觸發 Fetcher 爬取 RSS
- **DownloadScheduler**：監測待下載連結，派送至 Downloader，輪詢進度
- 都在 Core 中運行，間隔可配置

## 關鍵欄位說明

### rss_subscriptions 表
- `subscription_id`: PK
- `fetcher_id`: 指定處理此訂閱的 Fetcher
- `rss_url`: RSS 源 URL（UNIQUE）
- `is_active`: soft delete 旗標

### downloads 表
- `download_id`: PK
- `link_id`: 指向 anime_links 記錄
- `status`: pending → downloading → completed
- `downloader_client_id`: 指定的下載器（qBittorrent / PikPak）

### title_parsers 表
- `parser_id`: PK
- `name`: 解析器名稱（e.g., "mikanani_anime"）
- `pattern`: 正則表達式
- `anime_id`, `season_id`, `group_id`: 提取規則

## 常見命令

```bash
# 檢查編譯
cargo check

# 執行所有測試
cargo test

# 執行特定服務測試
cargo test -p core-service
cargo test -p fetcher-service

# 格式化代碼
cargo fmt

# 檢查代碼
cargo clippy

# 構建 Docker 鏡像
docker build -f core-service/Dockerfile -t bangumi-core:latest .
```

## 調試技巧

### 1. 查看數據庫
- Adminer: `http://localhost:8081`
- 用戶: `postgres`, 密碼: `postgres`
- 檢查 `bangumi` 和 `viewer_jellyfin` 數據庫

### 2. 查看日誌
```bash
# 增加日誌等級
RUST_LOG=debug cargo run --bin core-service
RUST_LOG=fetcher_service=trace cargo run --bin fetcher-service
```

### 3. 測試 API 端點
```bash
# 查詢訂閱
curl http://localhost:8000/subscriptions

# 創建訂閱
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{"fetcher_id": 1, "rss_url": "...", "name": "..."}'
```

## CORS 配置

通過環境變數設定 CORS：
```bash
CORS_ALLOWED_ORIGINS=http://localhost:3000,http://localhost:5173
CORS_ALLOWED_METHODS=GET,POST,DELETE,PUT
CORS_ALLOW_CREDENTIALS=true
```

詳見 `docs/CORS-CONFIGURATION.md`

## 重要限制與注意事項

### 1. DOOD (Docker Outside of Docker)
- volume 掛載路徑必須參考 **HOST 檔案系統**
- Downloader 下載檔案存儲在 HOST `/downloads`，Viewer 從同一路徑讀取

### 2. Integration Tests 中的 Handler 邏輯複製
- Binary crate (core-service, fetcher-service 等) 的 handler 邏輯無法被 tests/ 目錄導入
- 解決方案：將核心邏輯移到 library crate，或在測試中複製 handler 代碼

### 3. Service Registration
- 服務啟動時向 Core 註冊（deferred registration - 成功綁定 port 後才註冊）
- Core 在接收到 Downloader/Viewer 的註冊時，自動派送待處理任務

### 4. Raw Items 生命週期
- Fetcher 返回 raw_items 記錄（source_title, source_url 等）
- Core 用 title_parsers 解析，成功則創建 anime_links
- 未被解析的 items 保留，可手動 reparse 或標記 skip

## 文檔快速查詢

| 需求 | 文檔 |
|------|------|
| 完整架構設計 | `docs/plans/2025-01-21-rust-bangumi-architecture-design.md` |
| API 規格 | `docs/API-SPECIFICATIONS.md` + `docs/api/*.yaml` |
| RSS 訂閱架構 | `docs/ARCHITECTURE_RSS_SUBSCRIPTIONS.md` |
| CORS 設定 | `docs/CORS-QUICK-REFERENCE.md` |
| Viewer 設計 | `docs/plans/2026-02-06-viewer-jellyfin-design.md` |
| 進度日誌 | `docs/PROGRESS.md` |

## 提交代碼前的檢查清單

- [ ] 代碼通過 `cargo fmt` 格式化
- [ ] 代碼通過 `cargo clippy` 檢查
- [ ] 所有測試通過 (`cargo test`)
- [ ] 若新增 API 端點，已更新 OpenAPI spec (`docs/api/openapi.yaml`)
- [ ] 若修改數據庫，已創建 Diesel 遷移
- [ ] 若涉及多個服務，已驗證服務間通信正常
- [ ] 提交訊息清晰，說明變更原因而非細節

---

**最後更新**: 2026-03-04
**維護者**: Bangumi Project
