# Rust Bangumi - 微服務架構設計文檔

**日期：** 2025-01-21
**版本：** 1.0
**狀態：** 已驗證

---

## 1. 總體架構與數據流

### 1.1 系統分層

本系統由 5 個主要微服務組成，每個服務運行在獨立的 Docker 容器中：

1. **主服務（Core Service）** - 端口 8000
   - 管理 PostgreSQL 數據庫（核心數據存儲）
   - 運行內置 Cron 調度器，定時觸發擷取任務
   - 提供 REST API 供 CLI 工具和其他區塊調用
   - 服務註冊中心，接收並維護其他微服務的能力信息
   - 應用過濾規則、執行邏輯刪除、協調下載和同步流程

2. **擷取區塊（Fetcher Services）** - 可多個實例，如 `fetcher-mikanani`
   - 各自監聽獨立端口（如 8001, 8002, ...）
   - 啟動時向主服務 POST `/services/register` 註冊自己的能力
   - 提供 `/fetch` 端點接收主服務的 Cron 觸發
   - 執行 RSS 解析或爬蟲邏輯，取得最新動畫數據（不存儲）
   - 回傳原始動畫數據給主服務
   - 自行維護 RSS 訂閱配置（可本地存儲或數據庫）

3. **下載區塊（Downloader Services）** - 可多個實例
   - 各自監聽獨立端口
   - 啟動時向主服務註冊
   - 提供 `/download` 端點，判斷 URL 格式是否支持（magnet/torrent/http 等）
   - 調用實際下載工具（如 qBittorrent），追蹤進度
   - 定期回報下載進度到主服務

4. **顯示區塊（Viewer Service）** - 通常只有一個活躍實例
   - 監聽主服務的同步通知
   - 下載完成後，根據自己的邏輯組織文件（文件夾結構、命名等）
   - 提供 `/sync` 端點處理文件同步

5. **CLI 工具** - Rust 命令行應用
   - 調用主服務 REST API
   - 用戶與系統的主要交互方式

### 1.2 部署方式

- 使用 **Docker Compose** 定義所有服務及 PostgreSQL 容器
- `docker-compose up` 一鍵拉起整個系統
- 所有服務通過 Docker 內置 DNS 相互通信（如 `core-service:8000`）

---

## 2. 數據庫架構

### 2.1 核心表結構

#### 季度表 (Seasons)
```sql
CREATE TABLE seasons (
  season_id SERIAL PRIMARY KEY,
  year INT NOT NULL,
  season VARCHAR(10) NOT NULL,  -- 冬/春/夏/秋
  display_name TEXT GENERATED ALWAYS AS (CONCAT(year, season)) STORED,
  created_at TIMESTAMP DEFAULT NOW()
);
```

#### 動畫表 (Animes)
```sql
CREATE TABLE animes (
  anime_id SERIAL PRIMARY KEY,
  title VARCHAR(255) NOT NULL UNIQUE,
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW()
);
```

#### 動畫季數表 (AnimeSeries)
```sql
CREATE TABLE anime_series (
  series_id SERIAL PRIMARY KEY,
  anime_id INT NOT NULL REFERENCES animes(anime_id),
  series_no INT NOT NULL,  -- 第幾季
  season_id INT NOT NULL REFERENCES seasons(season_id),
  description TEXT,
  aired_date DATE,
  end_date DATE,
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  UNIQUE(anime_id, series_no)
);
```

#### 字幕組表 (SubtitleGroups)
```sql
CREATE TABLE subtitle_groups (
  group_id SERIAL PRIMARY KEY,
  group_name VARCHAR(255) NOT NULL UNIQUE,
  created_at TIMESTAMP DEFAULT NOW()
);
```

#### 動畫連結表 (AnimeLinks)
```sql
CREATE TABLE anime_links (
  link_id SERIAL PRIMARY KEY,
  series_id INT NOT NULL REFERENCES anime_series(series_id),
  group_id INT NOT NULL REFERENCES subtitle_groups(group_id),
  episode_no INT NOT NULL,  -- 第幾集
  title VARCHAR(255),
  url VARCHAR(2048) NOT NULL,  -- magnet/torrent/http 等格式
  source_hash VARCHAR(255) NOT NULL,  -- 擷取區塊提供
  filtered_flag BOOLEAN DEFAULT FALSE,  -- 邏輯刪除標記
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  UNIQUE(series_id, group_id, episode_no, source_hash)
);
```

#### 過濾規則表 (FilterRules)
```sql
CREATE TABLE filter_rules (
  rule_id SERIAL PRIMARY KEY,
  series_id INT NOT NULL REFERENCES anime_series(series_id),
  group_id INT NOT NULL REFERENCES subtitle_groups(group_id),
  rule_order INT NOT NULL,  -- 規則優先級
  rule_type VARCHAR(20) NOT NULL,  -- Positive 或 Negative
  regex_pattern TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT NOW(),
  UNIQUE(series_id, group_id, rule_order)
);
```

#### 下載記錄表 (Downloads)
```sql
CREATE TABLE downloads (
  download_id SERIAL PRIMARY KEY,
  link_id INT NOT NULL REFERENCES anime_links(link_id),
  downloader_type VARCHAR(50) NOT NULL,  -- qbittorrent 等
  status VARCHAR(20) NOT NULL,  -- pending/downloading/completed/failed
  progress DECIMAL(5, 2) DEFAULT 0.0,  -- 0-100
  downloaded_bytes BIGINT DEFAULT 0,
  total_bytes BIGINT DEFAULT 0,
  error_message TEXT,
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW()
);
```

#### Cron 任務日誌表 (CronLogs)
```sql
CREATE TABLE cron_logs (
  log_id SERIAL PRIMARY KEY,
  fetcher_type VARCHAR(50) NOT NULL,
  status VARCHAR(20) NOT NULL,  -- success/failed
  error_message TEXT,
  attempt_count INT DEFAULT 1,
  executed_at TIMESTAMP DEFAULT NOW()
);
```

### 2.2 數據流說明

- **RSS 訂閱表**：由擷取區塊各自維護（不在核心表內）
- **source_hash**：擷取區塊回傳的動畫列表必須包含此欄位，用於追蹤數據源
- **filtered_flag**：邏輯刪除標記，即使被過濾掉的連結也會存儲，但標記為 `true`
- **過濾規則**：以 `series_id + group_id` 為單位，規則有序執行

---

## 3. 主服務 REST API 與服務發現

### 3.1 服務註冊端點

**POST /services/register**

請求：
```json
{
  "service_type": "fetcher" | "downloader" | "viewer",
  "service_name": "mikanani",
  "host": "fetcher-mikanani",
  "port": 8001,
  "capabilities": {
    "fetch_endpoint": "/fetch",
    "download_endpoint": "/download",
    "sync_endpoint": "/sync"
  }
}
```

回應：
```json
{
  "service_id": "uuid",
  "registered_at": "2025-01-21T10:30:00Z"
}
```

### 3.2 主要 REST 端點

```
GET /services                          - 列出所有已註冊服務
GET /services/{type}                   - 列出特定類型服務
GET /services/{service_id}/health      - 檢查服務健康狀態

POST /subscribe                        - 添加 RSS 訂閱
GET /anime                             - 列出動畫（支持過濾）
GET /anime/{anime_id}                  - 獲取動畫詳情
GET /anime/{anime_id}/series/{num}     - 獲取指定季數

GET /cron/status                       - 查看 Cron 任務狀態
POST /fetch/{subscription_id}          - 手動觸發擷取
POST /download/{link_id}               - 手動觸發下載

POST /download-callback/progress       - 下載器回報進度
POST /sync-callback                    - 同步完成回調

GET /logs --type cron|download         - 查詢日誌
```

### 3.3 擷取區塊 `/fetch` 端點返回格式

```json
{
  "animes": [
    {
      "title": "動畫標題",
      "description": "動畫描述",
      "season": "冬",
      "year": 2025,
      "series_no": 1,
      "links": [
        {
          "episode_no": 1,
          "subtitle_group": "字幕組名稱",
          "title": "第1集",
          "url": "magnet:?xt=... 或其他格式",
          "source_hash": "hash_value_from_source"
        }
      ]
    }
  ]
}
```

---

## 4. CLI 工具設計

### 4.1 主要命令

```bash
# 訂閱管理
bangumi subscribe <rss-url> --fetcher <fetcher-name>
bangumi list [--anime-id <id>] [--season <year>/<season>]
bangumi links <anime-id> [--series <num>] [--group <group-name>]

# 過濾規則
bangumi filter <series-id> <group-name> add <positive|negative> <regex>
bangumi filter <series-id> <group-name> list
bangumi filter <series-id> <group-name> remove <rule-id>

# 下載
bangumi download <link-id> [--downloader <name>]

# 狀態查詢
bangumi status
bangumi services
bangumi logs --type cron|download [--filter ...]

# Cron 管理
bangumi cron list
bangumi cron add <subscription-id> --expression "0 */6 * * *"
bangumi cron disable <subscription-id>
```

---

## 5. Cron 調度與任務管理

### 5.1 調度配置

通過 docker-compose 環境變數或配置文件定義 Cron 任務：

```json
{
  "subscriptions": [
    {
      "subscription_id": "uuid1",
      "fetcher_type": "mikanani",
      "cron_expression": "0 */6 * * *",
      "enabled": true
    }
  ]
}
```

### 5.2 執行流程

1. Cron 觸發時，主服務根據 subscription_id 找到對應的擷取區塊服務
2. 調用 `POST {fetcher-host}:{port}/fetch` 獲取原始數據
3. 接收結果後，應用該 RSS 的 FilterRules，決定 `filtered_flag`
4. 所有數據寫入 AnimeLinks 表，記錄本次執行到 CronLogs 表
5. 如失敗，按照重試策略重試

### 5.3 重試策略

- **最大重試次數**：20 次
- **初始延遲**：60 秒（一分鐘）
- **退避策略**：指數退避（每次 * 2）

等待時間序列：
```
1st retry:  60s
2nd retry:  120s (2分鐘)
3rd retry:  240s (4分鐘)
...
20th retry: 60s * 2^19 ≈ 31天
```

每次重試嘗試都記錄到 CronLogs 表。

---

## 6. 下載區塊架構與集成

### 6.1 下載區塊職責

- 啟動時向主服務註冊
- 提供 `/download` 端點，判斷 URL 格式是否支持
- 調用實際下載工具（如 qBittorrent API）
- 定期回報進度到主服務

### 6.2 下載請求端點

**POST /download**

請求：
```json
{
  "link_id": "uuid",
  "url": "magnet:?xt=... 或 http://...",
  "callback_url": "http://core-service:8000/download-callback"
}
```

回應（立即）：
```json
{
  "status": "accepted" | "unsupported" | "error",
  "message": "開始下載 | URL 格式不支持此下載器 | 錯誤信息"
}
```

### 6.3 進度回調

下載器定期（如每 30 秒）調用主服務的進度更新端點：

**POST {callback_url}/progress**

```json
{
  "link_id": "uuid",
  "downloader_type": "qbittorrent",
  "status": "downloading" | "completed" | "failed",
  "progress": 0.75,
  "downloaded_bytes": 1000000,
  "total_bytes": 1500000,
  "error_message": null
}
```

### 6.4 流程說明

1. 主服務接收到新的 AnimeLink，調用各 downloader 的 `/download` 端點
2. 各 downloader 判斷 URL 格式，支持則返回 `accepted`，不支持則返回 `unsupported`
3. Downloader 在後台執行下載，定期回報進度
4. 下載完成或失敗時，回報最終狀態

---

## 7. 顯示區塊架構

### 7.1 顯示區塊職責

- 啟動時向主服務註冊
- 監聽主服務的同步通知
- 下載完成後，根據自己的邏輯決定文件組織方式
- 調整文件命名和存放路徑

### 7.2 同步端點

**POST /sync**

請求：
```json
{
  "link_id": "uuid",
  "anime_title": "動畫名稱",
  "series_no": 1,
  "episode_no": 1,
  "subtitle_group": "字幕組",
  "file_path": "/path/to/downloaded/file.mkv",
  "file_size": 1500000000
}
```

回應：
```json
{
  "status": "synced" | "failed",
  "target_path": "/media/path/organized/file.mkv",
  "message": "..."
}
```

### 7.3 流程說明

- 下載完成後，主服務調用已註冊的 viewer 服務的 `/sync` 端點
- Viewer 自行決定如何處理文件（移動、符號鏈接、重命名等）

---

## 8. 錯誤處理與監控

### 8.1 錯誤處理策略

**Cron 執行失敗**：
- 記錄到 CronLogs（status, error_message, attempt_count）
- 按指數退避策略重試（最多 20 次）
- 主服務在 `/status` 端點暴露最新失敗信息

**下載失敗**：
- 如某 downloader 返回 `unsupported`，嘗試其他 downloader
- 如所有 downloader 都失敗，標記 `Downloads.status = 'failed'`
- 記錄 error_message，等待用戶手動干預

**顯示/同步失敗**：
- Viewer 返回 `failed` 時，記錄到日誌
- 保持文件在暫存位置，等待手動處理或重試

**服務不可用**：
- 主服務定期心跳檢測已註冊服務（如每 30 秒）
- 心跳失敗後標記服務為 `offline`
- 避免向離線服務發送新任務

### 8.2 監控與日誌

**主要日誌表**：
- `CronLogs`：每次 Cron 執行記錄
- `Downloads`：下載狀態追蹤（status、progress）
- 服務心跳記錄（內存或簡單表）

**CLI 查詢日誌**：
```bash
bangumi logs --type cron [--subscription-id <id>]
bangumi logs --type download [--link-id <id>]
bangumi services --check-health
```

---

## 9. 數據流示例

### 9.1 完整工作流

1. **初始化**：各 fetcher/downloader/viewer 啟動，向主服務註冊

2. **定時擷取**：
   - Cron 觸發 → 主服務呼叫 `fetcher-mikanani/fetch`
   - 擷取區塊解析 RSS，回傳原始動畫數據
   - 主服務應用過濾規則，寫入 AnimeLinks（含 `filtered_flag`）

3. **用戶操作**：
   - CLI: `bangumi list` 列出未過濾的動畫
   - CLI: `bangumi filter <series-id> <group> add positive "1080p"` 添加規則
   - 已有的過濾標記可通過重新應用規則更新（需要額外邏輯）

4. **下載**：
   - CLI: `bangumi download <link-id>` 或自動觸發
   - 主服務嘗試各 downloader 直到成功或全失敗
   - Downloader 回報進度到主服務

5. **同步**：
   - 下載完成後，主服務呼叫 viewer 的 `/sync` 端點
   - Viewer 整理文件到媒體庫位置

---

## 10. 技術棧選型

| 組件 | 選技術 | 理由 |
|------|--------|------|
| 主服務框架 | Axum + Tokio | 高性能異步 Rust Web 框架 |
| CLI 框架 | Clap | 強大的命令行參數解析 |
| 數據庫 | PostgreSQL | 多表複雜查詢、事務支持 |
| ORM | SQLx 或 Diesel | 類型安全的 SQL 執行 |
| Cron 調度 | tokio-cron-scheduler | Tokio 異步支持 |
| 容器化 | Docker + Docker Compose | 標準化部署 |
| 通信 | REST API (JSON) | 簡單、易除錯 |

---

## 11. 部署與配置

### 11.1 docker-compose.yml 結構

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: bangumi
      POSTGRES_PASSWORD: password
    volumes:
      - postgres_data:/var/lib/postgresql/data

  core-service:
    build: ./core-service
    ports:
      - "8000:8000"
    depends_on:
      - postgres
    environment:
      DATABASE_URL: postgresql://...
      CRON_JOBS: |
        {...}

  fetcher-mikanani:
    build: ./fetchers/mikanani
    ports:
      - "8001:8001"
    depends_on:
      - core-service
    environment:
      CORE_SERVICE_URL: http://core-service:8000

  downloader-qbittorrent:
    build: ./downloaders/qbittorrent
    ports:
      - "8002:8002"
    depends_on:
      - core-service
    environment:
      CORE_SERVICE_URL: http://core-service:8000
      QBITTORRENT_URL: http://qbittorrent:8080

  viewer-jellyfin:
    build: ./viewers/jellyfin
    ports:
      - "8003:8003"
    depends_on:
      - core-service
    volumes:
      - /media/jellyfin:/media/output
    environment:
      CORE_SERVICE_URL: http://core-service:8000

volumes:
  postgres_data:
```

---

## 12. 後續擴展點

- **Web UI**：後續可添加 Web 前端，調用主服務 REST API
- **更多擷取區塊**：支持更多 RSS 源或爬蟲
- **更多下載區塊**：支持 Aria2、直鏈下載等
- **通知功能**：郵件/Telegram 通知下載完成
- **API 認證**：添加 API Key 或 OAuth2 認證
- **服務發現增強**：考慮 Consul、Etcd 等更完善的服務發現

---

## 13. 設計決策總結

| 決策項 | 選擇 | 理由 |
|--------|------|------|
| 架構風格 | 微服務 | 各區塊獨立部署、易於擴展 |
| 通信方式 | REST API | 簡單、標準、易於除錯 |
| 服務發現 | 動態註冊 | 支持動態增減模組 |
| 調度方式 | Cron + REST | 簡單、集中管理 |
| 重試策略 | 指數退避 20 次 | 平衡重試力度和資源消耗 |
| 數據庫 | 共享單一 | 數據一致性好、事務簡單 |
| 邏輯刪除 | 帶 flag | 保留歷史數據，便於審計 |
| 過濾規則 | 有序陣列執行 | 靈活、可擴展 |
| 初始交互 | CLI | 快速迭代、部署簡單 |

