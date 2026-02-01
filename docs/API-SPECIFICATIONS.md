# Bangumi API 規格文檔

本文檔說明 Bangumi 項目中所有 API 規格文件的組織結構和用途。

## API 規格文件概覽

### 1. 核心服務 API (`/docs/api/openapi.yaml`)

**目的：** 定義核心服務的完整 REST API 規格

**涵蓋範圍：**
- ✅ 服務註冊和管理
- ✅ 動畫和季度管理
- ✅ 字幕組管理
- ✅ 過濾規則管理
- ✅ 動畫連結管理
- ✅ RSS 訂閱管理
- ✅ **Fetcher 結果接收** (`POST /fetcher-results`, `POST /raw-fetcher-results`)
- ✅ 衝突解決
- ✅ 健康檢查

**服務器：**
- 開發環境：`http://localhost:8000`
- 生產環境：`http://core-service:8000`（Docker）

**使用場景：**
- 其他微服務（Downloader、Viewer）與核心服務通信
- Fetcher 向核心服務提交爬取結果
- 前端應用調用核心服務 API

### 2. 通用 Fetcher API (`/docs/api/fetcher-openapi.yaml`)

**目的：** 定義所有 Fetcher 服務的標準 API 介面

**涵蓋範圍：**
- ✅ 健康檢查 (`GET /health`)
- ✅ RSS 爬取功能 (`POST /fetch`)
- ✅ URL 歸屬檢查 (`POST /can-handle-subscription`)

**特點：**
- 通用的請求/響應格式
- 所有 Fetcher 服務應實現的標準端點
- 與具體 Fetcher 實現無關

**使用場景：**
- 定義 Fetcher 服務的標準契約
- 文檔化通用的 Fetcher API

### 3. Mikanani Fetcher API (`/docs/api/mikanani-fetcher-openapi.yaml`)

**目的：** 定義 Mikanani 特化 Fetcher 服務的詳細 API 規格

**涵蓋範圍：**
- ✅ 健康檢查 (`GET /health`)
- ✅ Mikanani 專用的 RSS 爬取 (`POST /fetch`)
- ✅ URL 歸屬檢查 (`POST /can-handle-subscription`)

**特點：**
- 詳細的文檔和範例
- Mikanani 特化的參數和驗證
- 支援的 URL 格式說明
- 詳細的錯誤處理說明

**服務器：**
- Docker 生產環境：`http://fetcher-mikanani:8001`
- 本地開發環境：`http://localhost:8001`

**Mikanani 特性：**
- 支援 Mikanani RSS 格式解析
- 指數退避重試機制（最多 3 次）
- 非同步爬取（回呼模式）

## API 規格之間的關係

```
┌─────────────────────────────────────────────────┐
│         核心服務 (Core Service)                 │
│         Port: 8000                              │
│  ┌─────────────────────────────────────────┐   │
│  │ POST /raw-fetcher-results               │   │
│  │ 接收來自 Fetcher 的原始爬取結果         │   │
│  │ Request: RawFetcherResultsPayload       │   │
│  └─────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────┐   │
│  │ FetchScheduler                          │   │
│  │ 排程並觸發 Fetcher 爬取                 │   │
│  │ 調用: POST /fetch on Fetcher            │   │
│  └─────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
         │                           ↑
         │ 1. 觸發爬取               │ 4. 回呼結果
         │ POST /fetch               │ POST /raw-fetcher-results
         ↓                           │
┌─────────────────────────────────────────────────┐
│      Mikanani Fetcher Service                   │
│      Port: 8001                                 │
│  ┌─────────────────────────────────────────┐   │
│  │ GET /health                             │   │
│  │ POST /fetch                             │   │
│  │ POST /can-handle-subscription           │   │
│  └─────────────────────────────────────────┘   │
│                                                 │
│  2. 回傳 202 Accepted                          │
│  3. 背景執行爬取並回呼                         │
└─────────────────────────────────────────────────┘
```

## API 數據流

### 1. Fetcher 爬取流程（Core-Driven）

```
1. 核心服務：POST /subscriptions
   → 建立新的 RSS 訂閱

2. 核心服務：FetchScheduler 定期檢查到期訂閱
   → 每 60 秒檢查一次

3. 核心服務：POST /fetch (發送到 Fetcher)
   → 觸發 Fetcher 爬取
   → Request: { subscription_id, rss_url, callback_url }

4. Mikanani Fetcher：回傳 202 Accepted
   → 立即回應，不阻塞

5. Mikanani Fetcher：背景執行爬取任務
   → 抓取 RSS、解析項目

6. Mikanani Fetcher：POST /raw-fetcher-results (回呼核心服務)
   → 提交原始爬取結果
   → Request: { subscription_id, items, fetcher_source, success, error_message }

7. 核心服務：處理和存儲結果
   → 解析標題、建立動畫、季度、字幕組、連結等
```

### 2. 結果結構對應

**Fetcher 端：**
- 產生 `RawFetcherResultsPayload`
- 包含 `RawAnimeItem` 陣列（原始 RSS 項目）

**核心服務端：**
- 接收 `RawFetcherResultsPayload`
- 使用 `TitleParser` 解析標題
- 轉換為數據庫模型（`Anime`、`AnimeSeries`、`AnimeLink` 等）
- 存儲到 PostgreSQL

## 開發指南

### 開發 Fetcher 服務時

1. 實現通用 Fetcher API (`fetcher-openapi.yaml`) 中的所有端點
2. 實現 `POST /can-handle-subscription` 來聲明 URL 歸屬
3. 確保 `POST /fetch` 為非同步操作（立即回傳 202，背景執行）
4. 完成後回呼到 `callback_url`

### 使用 Fetcher 時

1. 查看 `/health` 檢查服務健康狀態
2. 核心服務的 FetchScheduler 自動調用 `POST /fetch`
3. Fetcher 自動向核心服務回呼結果

### 整合新的 Fetcher 服務

1. 建立新的 Fetcher 服務目錄
2. 實現通用 Fetcher API 規格
3. 建立特化的 OpenAPI 規格文件
4. 向核心服務註冊 (`POST /services/register`)
5. 實現 `POST /can-handle-subscription` 來聲明支援的 URL 模式

## 規格驗證

所有 API 規格均遵守 OpenAPI 3.0.0 標準，可使用以下工具驗證：

### 使用 Swagger UI 檢視

```bash
# 核心服務 API
docker run -p 8080:8080 -e SWAGGER_JSON=/docs/api/openapi.yaml \
  -v $(pwd)/docs/api:/docs/api swaggerapi/swagger-ui

# Mikanani Fetcher API
docker run -p 8080:8080 -e SWAGGER_JSON=/docs/api/mikanani-fetcher-openapi.yaml \
  -v $(pwd)/docs/api:/docs/api swaggerapi/swagger-ui
```

### 本地驗證（使用 swagger-cli）

```bash
npm install -g swagger-cli
swagger-cli validate docs/api/openapi.yaml
swagger-cli validate docs/api/mikanani-fetcher-openapi.yaml
swagger-cli validate docs/api/fetcher-openapi.yaml
```

## 相關文檔

- **開發指南：** `/workspace/DEVELOPMENT.md`
- **Mikanani Fetcher README：** `/workspace/fetchers/mikanani/README.md`
- **架構設計：** `/workspace/docs/plans/`

## 端點統計

| 服務 | 端點數量 | 主要端點 |
|------|---------|---------|
| 核心服務 | 30+ | `/services`, `/anime`, `/raw-fetcher-results` |
| Fetcher (通用) | 3 | `/health`, `/fetch`, `/can-handle-subscription` |
| Mikanani Fetcher | 3 | `/health`, `/fetch`, `/can-handle-subscription` |

## 版本管理

- **核心服務版本：** 0.1.0
- **Fetcher API 版本：** 0.2.0
- **Mikanani Fetcher 版本：** 0.2.0

---

**最後更新：** 2026-02-01
**維護者：** Bangumi Project
