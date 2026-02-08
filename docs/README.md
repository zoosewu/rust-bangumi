# Bangumi 文檔導航

本目錄包含 Bangumi 項目的所有文檔資源。

## 📖 核心文檔

### [開發指南](./DEVELOPMENT.md)
- 開發環境設置
- 本地開發流程
- 常見命令和工作流

### [進度日誌](./PROGRESS.md)
- 項目實現進度
- Phase 1-9 完成情況
- 當前狀態和統計

## 🏗️ 架構與設計

### [API 規格文檔](./API-SPECIFICATIONS.md)
- Fetcher API 規格
- Mikanani Fetcher 特化規格
- 核心服務 API 數據流

### [RSS 訂閱管理架構](./ARCHITECTURE_RSS_SUBSCRIPTIONS.md)
- RSS 訂閱系統設計
- 數據流圖和交互說明

### [Viewer Jellyfin 設計文件](./plans/2026-02-06-viewer-jellyfin-design.md)
- Viewer 同步管線設計
- Core ↔ Viewer 資料流
- bangumi.tv Metadata 整合

### [Viewer Jellyfin README](../viewers/jellyfin/README.md)
- 開發環境設置與啟動流程
- API 文件（POST /sync、GET /health）
- Docker 部署與 Volume 掛載
- bangumi.tv API 整合與 NFO 格式

## ⚙️ 配置指南

### [CORS 配置指南](./CORS-CONFIGURATION.md)
- CORS 環境變數說明
- 使用場景和範例
- 測試和故障排除

### [CORS 快速參考](./CORS-QUICK-REFERENCE.md)
- 常用 CORS 配置
- 快速開始模板
- 快速查詢表

## 📋 規劃與報告

詳見 [plans/](./plans/) 目錄

### 核心規劃文檔

| 文件 | 描述 |
|------|------|
| [2025-01-21-rust-bangumi-architecture-design.md](./plans/2025-01-21-rust-bangumi-architecture-design.md) | 完整的系統架構設計 |
| [2025-01-21-implementation-plan.md](./plans/2025-01-21-implementation-plan.md) | 實現計劃和路線圖 |

### 階段完成報告

| Phase | 文件 | 狀態 |
|-------|------|------|
| 9 | [PHASE9_IMPLEMENTATION.md](./plans/PHASE9_IMPLEMENTATION.md) | ✅ 完成 |

### 功能實現報告

| 功能 | 文件 | 完成日期 |
|------|------|--------|
| Viewer Jellyfin 同步管線 | [2026-02-06-viewer-jellyfin-design.md](./plans/2026-02-06-viewer-jellyfin-design.md) | 2026-02-08 |
| 自動下載派送 | [2026-02-06-auto-download-dispatch-design.md](./plans/2026-02-06-auto-download-dispatch-design.md) | 2026-02-06 |
| Fetcher API 規格 | [2026-01-26-fetcher-api-spec-completion.md](./plans/2026-01-26-fetcher-api-spec-completion.md) | 2026-01-26 |
| CORS 實現 | [2026-01-26-cors-implementation-completion.md](./plans/2026-01-26-cors-implementation-completion.md) | 2026-01-26 |
| RSS 訂閱管理重構 | [2026-01-22-rss-subscription-management-refactor.md](./plans/2026-01-22-rss-subscription-management-refactor.md) | 2026-01-22 |
| 代碼清理最佳實踐 | [2026-01-22-codebase-cleanup-and-best-practices.md](./plans/2026-01-22-codebase-cleanup-and-best-practices.md) | 2026-01-22 |

## 📁 文件結構

```
docs/
├── README.md                                    # 本文件
├── DEVELOPMENT.md                              # 開發指南
├── PROGRESS.md                                 # 進度日誌
├── API-SPECIFICATIONS.md                       # API 規格文檔
├── ARCHITECTURE_RSS_SUBSCRIPTIONS.md            # RSS 訂閱架構
├── CORS-CONFIGURATION.md                       # CORS 配置指南
├── CORS-QUICK-REFERENCE.md                     # CORS 快速參考
├── api/
│   ├── openapi.yaml                            # 核心服務 API 規格
│   ├── fetcher-openapi.yaml                    # 通用 Fetcher API 規格
│   └── mikanani-fetcher-openapi.yaml           # Mikanani Fetcher API 規格
└── plans/
    ├── 2025-01-21-*.md                         # 早期規劃和架構
    ├── 2026-01-22-*.md                         # 最近的改進和重構
    └── 2026-01-26-*.md                         # 最新功能實現
```

## 🔍 快速查詢

### 我想...

**開始開發**
→ [開發指南](./DEVELOPMENT.md)

**了解項目進度**
→ [進度日誌](./PROGRESS.md)

**查看 API 文檔**
→ [API 規格文檔](./API-SPECIFICATIONS.md) 或 [api/](./api/) 目錄

**配置 CORS**
→ [CORS 快速參考](./CORS-QUICK-REFERENCE.md)

**理解系統架構**
→ [2025-01-21-rust-bangumi-architecture-design.md](./plans/2025-01-21-rust-bangumi-architecture-design.md)

**了解 RSS 訂閱系統**
→ [RSS 訂閱管理架構](./ARCHITECTURE_RSS_SUBSCRIPTIONS.md)

**開發 / 部署 Viewer Jellyfin**
→ [Viewer Jellyfin README](../viewers/jellyfin/README.md)

## 📊 統計信息

- **總文檔數**：20+ 個 markdown 文檔
- **API 規格**：3 個 OpenAPI 規格文件
- **服務數量**：4 個（Core、Fetcher、Downloader、Viewer）
- **最後更新**：2026-02-08

## 📝 文檔維護

文檔遵循以下命名約定：

- `*.md` - Markdown 文檔
- `YYYY-MM-DD-*.md` - 帶日期的計劃和報告
- `api/*.yaml` - OpenAPI 規格文件

如有文檔更新或新增，請遵循以下規則：

1. 功能實現報告放在 `plans/` 目錄
2. 項目導航或配置指南放在 `docs/` 根目錄
3. API 規格放在 `api/` 目錄
4. 使用 ISO 日期格式 (YYYY-MM-DD) 命名報告文件

---

**最後更新：** 2026-01-26
**維護者：** Bangumi Project
