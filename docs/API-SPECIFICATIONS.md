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
- ✅ **Fetcher 結果接收** (`POST /fetcher-results`)
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
- ✅ 健康檢查
- ✅ RSS 爬取功能
- ✅ 訂閱管理

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
- ✅ 健康檢查
- ✅ Mikanani 專用的 RSS 爬取
- ✅ 訂閱廣播處理
- ✅ 服務信息端點

**特點：**
- 詳細的文檔和範例
- Mikanani 特化的參數和驗證
- 支援的 URL 格式說明
- 詳細的錯誤處理說明

**服務器：**
- Docker 生產環境：`http://fetcher-mikanani:8001`
- 本地開發環境：`http://localhost:8001`

**Mikanani 特性：**
- 支援多種標題格式檢測（[01]、第01話、EP01）
- 自動 SHA256 去重
- 指數退避重試機制（最多 3 次）
- 字幕組信息提取

## API 規格之間的關係

```
┌─────────────────────────────────────────────────┐
│         核心服務 (Core Service)                 │
│         Port: 8000                              │
│  ┌─────────────────────────────────────────┐   │
│  │ POST /fetcher-results                   │   │
│  │ 接收來自 Fetcher 的爬取結果             │   │
│  │ Request: FetcherResultsPayload          │   │
│  │ Response: FetcherResultsResponse        │   │
│  └─────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
                       ↑
                       │ 發送爬取結果
                       │
┌─────────────────────────────────────────────────┐
│      Mikanani Fetcher Service                   │
│      Port: 8001                                 │
│  ┌─────────────────────────────────────────┐   │
│  │ GET /health                             │   │
│  │ POST /fetch                             │   │
│  │ POST /subscribe                         │   │
│  │ GET /info                               │   │
│  └─────────────────────────────────────────┘   │
│                                                 │
│  遵守通用 Fetcher API 規格                      │
│  + Mikanani 特化功能                            │
└─────────────────────────────────────────────────┘
```

## API 數據流

### 1. Fetcher 爬取流程

```
1. 核心服務：POST /subscriptions
   → 建立新的 RSS 訂閱

2. 核心服務：POST /services/register (Fetcher 自動執行)
   → Fetcher 向核心服務註冊

3. 核心服務：POST /subscribe (廣播訂閱信息)
   → 通知 Fetcher 新的訂閱

4. Mikanani Fetcher：POST /subscribe (接收)
   → Fetcher 接收訂閱信息

5. Mikanani Fetcher：POST /fetch (內部或定期執行)
   → Fetcher 爬取 RSS 源

6. Mikanani Fetcher：計算結果
   → 解析 RSS、提取元數據、生成結果

7. Mikanani Fetcher：POST /fetcher-results (提交到核心服務)
   → 提交爬取結果

8. 核心服務：處理和存儲結果
   → 建立動畫、季度、字幕組、連結等
```

### 2. 結果結構對應

**Fetcher 端：**
- Mikanani Fetcher 內部處理
- 生成 `FetchedAnime` 和 `FetchedLink` 結構

**核心服務端：**
- 接收 `FetcherResultsPayload`（包含數組的 `FetchedAnime`）
- 轉換為數據庫模型（`Anime`、`AnimeSeries`、`AnimeLink` 等）
- 存儲到 PostgreSQL

## 開發指南

### 開發 Fetcher 服務時

1. 實現通用 Fetcher API (`fetcher-openapi.yaml`) 中的所有端點
2. 如需特化功能，在 Mikanani 規格中補充說明
3. 確保響應格式符合規格定義

### 使用 Fetcher 時

1. 查看 `/health` 檢查服務健康狀態
2. 調用 `POST /fetch` 爬取 RSS 源
3. Fetcher 自動向核心服務提交結果

### 整合新的 Fetcher 服務

1. 建立新的 Fetcher 服務目錄
2. 實現通用 Fetcher API 規格
3. 建立特化的 OpenAPI 規格文件
4. 更新核心服務支持的 fetcher_source 列表

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
| 核心服務 | 30+ | `/services`, `/anime`, `/fetcher-results` |
| Fetcher (通用) | 3 | `/health`, `/fetch`, `/subscribe` |
| Mikanani Fetcher | 4 | `/health`, `/fetch`, `/subscribe`, `/info` |

## 版本管理

- **核心服務版本：** 0.1.0
- **Fetcher API 版本：** 0.1.0
- **Mikanani Fetcher 版本：** 0.1.0

---

**最後更新：** 2026-01-26
**維護者：** Bangumi Project
