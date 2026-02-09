# Rust Bangumi 實現進度

**最後更新：** 2026-02-09
**當前狀態：** Phase 1-9 完成，端到端管線已實現
**完成百分比：** 9/11 階段 (82%)

---

## 系統概覽

```
RSS 訂閱 → Fetcher 抓取 → Core 解析標題 → 自動下載派送 → Viewer 同步至 Jellyfin
```

4 個微服務 + CLI，總 API 端點 60 個，5 個 OpenAPI 規格文件。

---

## 已完成的所有階段

### Phase 1: 數據庫與 Diesel 遷移

- Diesel CLI 安裝和配置
- 8 張數據庫表：seasons, animes, anime_series, subtitle_groups, anime_links, filter_rules, downloads, cron_logs
- 後續新增：raw_items, title_parsers, catch_all_parser migration

### Phase 2: 數據庫訪問層

- Diesel Schema 和模型生成（Queryable + Insertable）
- r2d2 連接池（max_size=16）
- Docker alpine 基礎鏡像優化

### Phase 3: 核心服務架構

- Axum Web 框架
- 服務註冊（HashMap 內存註冊表）
- CRUD 操作層（動畫、季度、字幕組、連結等）
- REST API 端點

### Phase 4: 過濾規則引擎

- FilterEngine 正則匹配
- 正向/反向過濾規則

### Phase 5: 定時調度系統

- FetchScheduler - 定期觸發 Fetcher 爬取
- DownloadScheduler - 偵測新連結並派送下載、追蹤進度、觸發 Viewer 同步

### Phase 6: 擷取服務實現

- Mikanani RSS Fetcher
- 非同步爬取（202 + 回呼）
- URL 歸屬檢查
- Magnet link 優先提取

### Phase 7: 下載器實現

- qBittorrent WebAPI client
- 批次下載/取消/暫停/恢復/刪除
- 支援 magnet link 和 .torrent URL
- trait-based 抽象 (`DownloaderClient` + `MockDownloaderClient`)

### Phase 8: Jellyfin Viewer 實現

- 檔案搬移至 `{anime_title}/Season XX/` 結構
- bangumi.tv API 整合（搜尋、metadata、集數資訊）
- NFO 產生（tvshow.nfo + episode.nfo + poster.jpg）
- 獨立 `viewer_jellyfin` 資料庫（4 張表）
- 非同步同步管線（202 + 回呼）

### Phase 9: CLI 工具實現

- 8 個 CLI 命令：subscribe, list, links, filter, download, status, services, logs
- HTTP client（GET/POST/DELETE）
- 24 個測試（100% pass）
- Docker 多階段構建

---

## 近期重要功能（Phase 9 後）

### 標題解析器系統（Title Parsers）
- 可配置的正則解析器，取代硬編碼解析邏輯
- CRUD API（GET/POST/DELETE /parsers）
- 原始 RSS 項目管理（GET /raw-items, reparse, skip）
- Catch-all parser 作為 fallback

### 自動下載派送（Auto-Download Dispatch）
- DownloadScheduler 自動偵測 pending 連結
- 批次派送至已註冊的 Downloader
- 定期輪詢下載進度
- 下載完成後自動觸發 Viewer 同步

### Magnet Link 優先
- Fetcher 優先提取 magnet link
- Downloader 同時支援 magnet 和 .torrent URL
- `extract_hash_from_url` 統一處理兩種格式

### 服務生命週期
- Deferred service registration（port binding 成功後才註冊）
- Viewer/Downloader 註冊時觸發待處理任務
- Graceful port binding（端口衝突處理）

### CORS 配置
- 環境變數驅動的 CORS 設定
- 支援多來源、萬用字元

---

## 待完成的工作

| 階段 | 描述 | 狀態 |
|------|------|------|
| Phase 10 | 高級功能與優化 | 計劃中 |
| Phase 11 | 生產環境部署 | 計劃中 |

### Phase 10 建議方向

- 前端 Web UI
- WebSocket 實時日誌流
- API 認證與授權
- Prometheus 監控指標
- 連接池與查詢優化

---

## 當前代碼狀態

### 服務與端點

| 服務 | Port | 端點數 | 狀態 |
|------|------|--------|------|
| Core Service | 8000 | 45 | 運行中 |
| Fetcher (Mikanani) | 8001 | 3 | 運行中 |
| Downloader (qBittorrent) | 8002 | 7 | 運行中 |
| Viewer (Jellyfin) | 8003 | 2 | 運行中 |
| CLI | - | - | 可用 |

### 主要依賴

- Diesel 2.1（ORM）+ r2d2 連接池
- Tokio（異步運行時）
- Axum（Web 框架）
- Tracing（日誌）
- PostgreSQL 15+（資料庫）
- reqwest（HTTP client）

### 開發環境（docker-compose.dev.yaml）

| 服務 | Port | 用途 |
|------|------|------|
| PostgreSQL | 5432 | 資料庫（bangumi + viewer_jellyfin） |
| Adminer | 8081 | 資料庫管理介面 |
| qBittorrent | 8080 | BT 下載客戶端 |
| Jellyfin | 8096 | 媒體伺服器 |

---

## 相關文檔

- **架構設計**：`docs/plans/2025-01-21-rust-bangumi-architecture-design.md`
- **API 規格**：`docs/API-SPECIFICATIONS.md`
- **開發指南**：`DEVELOPMENT.md`
- **Viewer 設計**：`docs/plans/2026-02-06-viewer-jellyfin-design.md`

---

**分支**：master
**完成階段**：Phase 1-9 (9/11)
