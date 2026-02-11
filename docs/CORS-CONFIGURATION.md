# CORS 配置指南

本文檔說明如何在 Bangumi 項目中配置 CORS（跨來源資源共享）。

## 概述

所有 Bangumi 服務（核心服務、Fetcher、Downloader、Viewer）都支持通過環境變數進行 CORS 配置。

## 環境變數

### 1. `ENABLE_CORS`

**描述：** 全局開啟或關閉 CORS 支持

**值：**
- `true`（預設）- 啟用 CORS
- `false` - 禁用 CORS

**範例：**
```bash
ENABLE_CORS=true
```

### 2. `CORS_ALLOWED_ORIGINS`

**描述：** 允許的跨來源請求的來源

**值：**
- `*`（預設）- 允許所有來源
- 特定域名 - 逗號分隔的域名列表

**範例：**
```bash
# 允許所有來源
CORS_ALLOWED_ORIGINS=*

# 允許特定本地域名
CORS_ALLOWED_ORIGINS=http://localhost:8004,http://localhost:3001

# 允許生產域名
CORS_ALLOWED_ORIGINS=https://app.example.com,https://admin.example.com

# 混合配置
CORS_ALLOWED_ORIGINS=http://localhost:8004,https://app.example.com
```

## 使用場景

### 場景 1：開發環境 - 允許所有來源

```bash
# .env
ENABLE_CORS=true
CORS_ALLOWED_ORIGINS=*
```

所有本地開發的前端應用都可以訪問 API。

### 場景 2：開發環境 - 允許特定端口

```bash
# .env
ENABLE_CORS=true
CORS_ALLOWED_ORIGINS=http://localhost:8004,http://localhost:3001
```

只允許運行在本機指定端口的應用訪問 API。

### 場景 3：生產環境 - 只允許自己的域名

```bash
# .env
ENABLE_CORS=true
CORS_ALLOWED_ORIGINS=https://app.example.com,https://admin.example.com
```

只允許你擁有的域名訪問 API，確保安全性。

### 場景 4：完全禁用 CORS

```bash
# .env
ENABLE_CORS=false
```

完全禁用 CORS，只允許同源請求。此時 `CORS_ALLOWED_ORIGINS` 被忽略。

## Docker Compose 配置

### 全局配置

所有服務都會自動讀取環境變數。在 `.env` 文件中設定：

```bash
# 啟用 CORS 並允許所有來源
ENABLE_CORS=true
CORS_ALLOWED_ORIGINS=*
```

然後啟動容器：

```bash
docker-compose up
```

### 服務級別配置

如果想為不同服務配置不同的 CORS 策略，可以在 `docker-compose.override.yaml` 中覆蓋：

```yaml
services:
  core-service:
    environment:
      ENABLE_CORS: "true"
      CORS_ALLOWED_ORIGINS: "https://app.example.com"

  fetcher-mikanani:
    environment:
      ENABLE_CORS: "false"
```

## 支持的 HTTP 方法

CORS 層支持以下 HTTP 方法：

- `GET` - 獲取資源
- `POST` - 創建資源
- `PUT` - 更新資源
- `DELETE` - 刪除資源
- `PATCH` - 部分更新
- `OPTIONS` - CORS 預檢請求

## 支持的 Header

CORS 層會自動鏡像/反映請求中的 Header，並支持憑證（Cookies）。

**允許的請求 Header：**
- 所有標準 HTTP Header
- 自定義 Header

**響應 Header：**
- `Access-Control-Allow-Origin` - 允許的來源
- `Access-Control-Allow-Methods` - 允許的 HTTP 方法
- `Access-Control-Allow-Headers` - 允許的 Header
- `Access-Control-Allow-Credentials` - 是否允許憑證

## 實現細節

### 核心服務 (Core Service)

**文件：** `core-service/src/cors.rs`

核心服務使用 `tower-http` 的 `CorsLayer` 中間件。

```rust
// 啟用 CORS
let cors = cors::create_cors_layer();
if let Some(cors_layer) = cors {
    app = app.layer(cors_layer);
}
```

### Fetcher 服務 (Mikanani Fetcher)

**文件：** `fetchers/mikanani/src/cors.rs`

Fetcher 服務使用相同的 CORS 實現，確保一致性。

### 其他服務

- **Downloader (qBittorrent)** - 待實現
- **Viewer (Jellyfin)** - 待實現

## 測試 CORS 配置

### 使用 curl 測試

```bash
# 測試 CORS 預檢請求
curl -X OPTIONS http://localhost:8000/health \
  -H "Origin: http://localhost:8004" \
  -H "Access-Control-Request-Method: GET" \
  -v

# 應該看到類似的 Response Header：
# < Access-Control-Allow-Origin: http://localhost:8004
# < Access-Control-Allow-Methods: GET, POST, DELETE, ...
```

### 使用瀏覽器測試

```javascript
// 在瀏覽器控制台中測試
fetch('http://localhost:8000/health', {
  method: 'GET',
  headers: {
    'Content-Type': 'application/json',
  }
})
.then(response => response.json())
.then(data => console.log(data))
.catch(error => console.error('CORS Error:', error));
```

## 日誌輸出

當服務啟動時，會輸出 CORS 配置信息：

```
CORS 已啟用 - 允許的來源: *
```

或

```
CORS 已禁用
```

## 常見問題

### Q: 為什麼出現 CORS 錯誤？

**A:**
1. 檢查 `ENABLE_CORS` 是否設為 `true`
2. 檢查前端應用的來源是否在 `CORS_ALLOWED_ORIGINS` 中
3. 確認使用的是正確的協議（http/https）和端口

### Q: 如何在生產環境中安全地配置 CORS？

**A:**
1. 只允許你自己擁有的域名
2. 使用 HTTPS（https://）
3. 避免使用通配符 `*`

### Q: 是否可以為不同的 API 端點配置不同的 CORS 策略？

**A:** 目前不支持。所有端點使用相同的 CORS 配置。如有特殊需求，可以在 `cors.rs` 中擴展實現。

## 相關文檔

- [API 規格文檔](/workspace/docs/API-SPECIFICATIONS.md)
- [開發指南](/workspace/DEVELOPMENT.md)
- [Docker 部署指南](/workspace/docs/DOCKER-DEPLOYMENT.md)

## 更新日誌

### v0.1.0 (2026-01-26)
- 初始 CORS 支持實現
- 核心服務 CORS 實現完成
- Mikanani Fetcher CORS 實現完成
- 環境變數配置系統完成

---

**最後更新：** 2026-01-26
**維護者：** Bangumi Project
