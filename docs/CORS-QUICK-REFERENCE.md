# CORS 快速參考

## 快速開始

### 方法 1：允許所有來源（開發環境）
```bash
# .env
ENABLE_CORS=true
CORS_ALLOWED_ORIGINS=*
```

### 方法 2：禁用 CORS
```bash
# .env
ENABLE_CORS=false
```

### 方法 3：允許特定來源（生產環境）
```bash
# .env
ENABLE_CORS=true
CORS_ALLOWED_ORIGINS=https://app.example.com,https://admin.example.com
```

## 環境變數

| 變數 | 預設值 | 說明 |
|------|-------|------|
| `ENABLE_CORS` | `true` | 啟用/禁用 CORS |
| `CORS_ALLOWED_ORIGINS` | `*` | 允許的來源 |

## 常用配置

### 本地開發
```bash
ENABLE_CORS=true
CORS_ALLOWED_ORIGINS=*
```

### 多端口開發
```bash
ENABLE_CORS=true
CORS_ALLOWED_ORIGINS=http://localhost:8004,http://localhost:3001,http://localhost:8080
```

### 單一生產域名
```bash
ENABLE_CORS=true
CORS_ALLOWED_ORIGINS=https://app.example.com
```

### 多個生產域名
```bash
ENABLE_CORS=true
CORS_ALLOWED_ORIGINS=https://app.example.com,https://api.example.com
```

### 禁用所有 CORS
```bash
ENABLE_CORS=false
```

## 支持的服務

- ✅ core-service (核心服務)
- ✅ fetcher-mikanani (Mikanani Fetcher)
- ✅ downloader-qbittorrent (qBittorrent Downloader)
- ✅ viewer-jellyfin (Jellyfin Viewer)

## 支持的 HTTP 方法

`GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `OPTIONS`

## 測試 CORS

```bash
# curl 測試
curl -X OPTIONS http://localhost:8000/health \
  -H "Origin: http://localhost:8004" \
  -v
```

## 日誌消息

| 消息 | 含義 |
|------|------|
| `CORS 已啟用 - 允許所有來源` | 允許所有來源的請求 |
| `CORS 已啟用 - 僅允許...` | 只允許特定來源（待完全實現） |
| `CORS 已禁用` | CORS 已完全禁用 |

## 更多信息

- 詳細文檔: [docs/CORS-CONFIGURATION.md](docs/CORS-CONFIGURATION.md)
- 實現報告: [docs/plans/2026-01-26-cors-implementation-completion.md](docs/plans/2026-01-26-cors-implementation-completion.md)
