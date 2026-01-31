# Fetcher 架構重構規格書

## 一、概述

### 變更目標
將解析職責從 Fetcher 移至 Core Service，Fetcher 只負責抓取原始資料。

### 架構變更

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           變更後架構                                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Fetcher                              Core Service                      │
│  ┌──────────────┐                    ┌──────────────────────────────┐   │
│  │ 抓取 RSS     │                    │                              │   │
│  │ (不解析)     │ ──POST────────────→│  1. 儲存到 raw_anime_items   │   │
│  └──────────────┘   原始資料          │                              │   │
│                     - title           │  2. 查詢 title_parsers       │   │
│                     - description     │     (按 priority 排序)       │   │
│                     - download_url    │                              │   │
│                     - pub_date        │  3. 嘗試解析標題              │   │
│                                       │                              │   │
│                                       │  4. 成功 → 建立 anime_links  │   │
│                                       │     失敗 → 更新 status       │   │
│                                       └──────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 二、資料庫變更

### 2.1 新增表：`raw_anime_items`

儲存 Fetcher 抓取的原始資料。

```sql
CREATE TABLE raw_anime_items (
    item_id             SERIAL PRIMARY KEY,

    -- 原始資料（來自 RSS）
    title               TEXT NOT NULL,              -- RSS <title>
    description         TEXT,                       -- RSS <description>
    download_url        VARCHAR(2048) NOT NULL,     -- RSS <enclosure> 或 <link>
    pub_date            TIMESTAMP,                  -- RSS <pubDate>

    -- 來源追蹤
    subscription_id     INT NOT NULL REFERENCES subscriptions(subscription_id),

    -- 處理狀態
    status              VARCHAR(20) NOT NULL DEFAULT 'pending',
    parser_id           INT REFERENCES title_parsers(parser_id),
    error_message       TEXT,
    parsed_at           TIMESTAMP,

    -- 中繼資料
    created_at          TIMESTAMP NOT NULL DEFAULT NOW(),

    -- 去重
    UNIQUE(download_url)
);

-- 索引
CREATE INDEX idx_raw_items_status ON raw_anime_items(status);
CREATE INDEX idx_raw_items_subscription ON raw_anime_items(subscription_id);
CREATE INDEX idx_raw_items_created ON raw_anime_items(created_at DESC);
```

#### 狀態定義

| status | 說明 |
|--------|------|
| `pending` | 待處理，尚未嘗試解析 |
| `parsed` | 完全成功，所有必要欄位都已提取 |
| `partial` | 部分成功，缺少部分必要欄位 |
| `failed` | 解析失敗，無法提取必要欄位 |
| `no_match` | 無匹配解析器，所有解析器的 condition_regex 都不匹配 |
| `skipped` | 手動跳過 |

---

### 2.2 新增 ENUM 類型：`parser_source_type`

```sql
CREATE TYPE parser_source_type AS ENUM ('regex', 'static');
```

### 2.3 新增表：`title_parsers`

儲存標題解析器配置。

```sql
CREATE TABLE title_parsers (
    parser_id               SERIAL PRIMARY KEY,

    -- 基本資訊
    name                    VARCHAR(100) NOT NULL,
    description             TEXT,
    priority                INT NOT NULL DEFAULT 0,
    is_enabled              BOOLEAN NOT NULL DEFAULT TRUE,

    -- 解析規則
    condition_regex         TEXT NOT NULL,
    parse_regex             TEXT NOT NULL,

    -- ========== anime_title (必要) ==========
    anime_title_source      parser_source_type NOT NULL,
    anime_title_value       VARCHAR(255) NOT NULL,

    -- ========== episode_no (必要) ==========
    episode_no_source       parser_source_type NOT NULL,
    episode_no_value        VARCHAR(50) NOT NULL,

    -- ========== series_no (必要，空值時程式碼帶入 1) ==========
    series_no_source        parser_source_type,
    series_no_value         VARCHAR(50),

    -- ========== subtitle_group (非必要) ==========
    subtitle_group_source   parser_source_type,
    subtitle_group_value    VARCHAR(255),

    -- ========== resolution (非必要) ==========
    resolution_source       parser_source_type,
    resolution_value        VARCHAR(50),

    -- ========== season (非必要) ==========
    season_source           parser_source_type,
    season_value            VARCHAR(20),

    -- ========== year (非必要) ==========
    year_source             parser_source_type,
    year_value              VARCHAR(10),

    -- 時間戳
    created_at              TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMP NOT NULL DEFAULT NOW()
);

-- 索引
CREATE INDEX idx_title_parsers_priority
ON title_parsers(priority DESC)
WHERE is_enabled = TRUE;
```

#### ENUM 類型說明

`parser_source_type` 定義了欄位值的來源：

| 值 | 說明 | value 欄位含義 |
|----|------|----------------|
| `regex` | 從 parse_regex 的捕獲組提取 | 捕獲組索引（如 `1`, `2`） |
| `static` | 使用固定值 | 固定值（如 `LoliHouse`, `1`） |
| `NULL` | 不提取此欄位 | value 也為 `NULL` |

#### 必要欄位定義

| 欄位 | 必要性 | 空值處理 |
|------|--------|----------|
| `anime_title` | **必要** | 無法為空，解析失敗則整體失敗 |
| `episode_no` | **必要** | 無法為空，解析失敗則整體失敗 |
| `series_no` | **必要** | 空值時程式碼帶入預設值 `1` |
| `subtitle_group` | 非必要 | 可為空 |
| `resolution` | 非必要 | 可為空 |
| `season` | 非必要 | 可為空 |
| `year` | 非必要 | 可為空 |

---

### 2.3 修改表：`anime_links`

新增外鍵連結到原始資料。

```sql
ALTER TABLE anime_links
ADD COLUMN raw_item_id INT REFERENCES raw_anime_items(item_id);

CREATE INDEX idx_anime_links_raw_item ON anime_links(raw_item_id);
```

---

## 三、API 變更

### 3.1 Fetcher 回傳格式

#### 舊格式（移除）

```rust
// 移除
pub struct FetchedAnime {
    pub title: String,
    pub description: String,
    pub season: String,
    pub year: i32,
    pub series_no: i32,
    pub links: Vec<FetchedLink>,
}

pub struct FetchedLink {
    pub episode_no: i32,
    pub subtitle_group: String,
    pub title: String,
    pub url: String,
    pub source_hash: String,
    pub source_rss_url: String,
}
```

#### 新格式

```rust
// shared/src/models.rs

/// 原始動畫項目（單集）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAnimeItem {
    pub title: String,                      // RSS <title>
    pub description: Option<String>,        // RSS <description>
    pub download_url: String,               // RSS <enclosure> url
    pub pub_date: Option<DateTime<Utc>>,    // RSS <pubDate>
}

/// Fetcher 回傳的結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherResultsPayload {
    pub subscription_id: i32,
    pub items: Vec<RawAnimeItem>,
    pub fetcher_source: String,
    pub success: bool,
    pub error_message: Option<String>,
}
```

### 3.2 Core Service 新增 API

#### 解析器管理

```
GET    /parsers                     # 列出所有解析器
POST   /parsers                     # 新增解析器
GET    /parsers/:parser_id          # 取得單一解析器
PUT    /parsers/:parser_id          # 更新解析器
DELETE /parsers/:parser_id          # 刪除解析器
```

#### 原始資料管理

```
GET    /raw-items                   # 列出原始資料（支援 status 篩選）
GET    /raw-items/:item_id          # 取得單一項目
POST   /raw-items/:item_id/reparse  # 重新解析單一項目
POST   /raw-items/reparse           # 批次重新解析（依 status 或 parser_id）
POST   /raw-items/:item_id/skip     # 標記為跳過
```

---

## 四、解析流程

### 4.1 收到 Fetcher 結果時

```
1. 收到 FetcherResultsPayload
   │
2. 對每個 RawAnimeItem:
   │
   ├─ 檢查 download_url 是否已存在
   │   └─ 已存在 → 跳過（去重）
   │
   ├─ 儲存到 raw_anime_items (status = 'pending')
   │
   └─ 立即執行解析流程
```

### 4.2 解析流程

```
1. 取得所有啟用的解析器（按 priority DESC 排序）
   │
2. 對每個解析器:
   │
   ├─ 檢查 condition_regex 是否匹配標題
   │   └─ 不匹配 → 嘗試下一個解析器
   │
   ├─ 執行 parse_regex 提取捕獲組
   │
   ├─ 根據欄位設定提取各欄位:
   │   ├─ source = 'regex' → 從捕獲組取值
   │   └─ source = 'static' → 使用固定值
   │
   ├─ 驗證必要欄位:
   │   ├─ anime_title 必須有值
   │   ├─ episode_no 必須有值且為有效數字
   │   └─ series_no 空值時帶入 1
   │
   └─ 判斷結果:
       ├─ 全部必要欄位成功 → status = 'parsed'
       ├─ 部分必要欄位成功 → status = 'partial'
       └─ 必要欄位失敗 → 嘗試下一個解析器

3. 所有解析器都不匹配 → status = 'no_match'

4. 解析成功時:
   │
   ├─ 建立/查詢 anime 記錄
   ├─ 建立/查詢 anime_series 記錄
   ├─ 建立/查詢 subtitle_group 記錄
   └─ 建立 anime_link 記錄（包含 raw_item_id）
```

---

## 五、範例資料

### 5.1 解析器範例

```sql
-- 解析器 1: LoliHouse 標準格式
-- 標題範例: [LoliHouse] 黄金神威 最终章 / Golden Kamuy - 53 [WebRip 1080p HEVC-10bit AAC]
INSERT INTO title_parsers (
    name, description, priority,
    condition_regex, parse_regex,
    anime_title_source, anime_title_value,
    episode_no_source, episode_no_value,
    series_no_source, series_no_value,
    subtitle_group_source, subtitle_group_value,
    resolution_source, resolution_value
) VALUES (
    'LoliHouse 標準格式',
    '匹配 [字幕組] 動畫名稱 - 集數 [解析度] 格式',
    100,
    '^\[.+\].+\s-\s\d+',
    '^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)\s*\[.*?(\d{3,4}p)',
    'regex', '2',      -- anime_title 從捕獲組 2
    'regex', '3',      -- episode_no 從捕獲組 3
    NULL, NULL,        -- series_no 空值，程式碼帶入 1
    'regex', '1',      -- subtitle_group 從捕獲組 1
    'regex', '4',      -- resolution 從捕獲組 4
    NULL, NULL,        -- season 不提取
    NULL, NULL         -- year 不提取
);

-- 解析器 2: 六四位元 星號格式
-- 標題範例: 六四位元字幕组★可以帮忙洗干净吗？★04★1920x1080★AVC AAC MP4★繁体中文
INSERT INTO title_parsers (
    name, description, priority,
    condition_regex, parse_regex,
    anime_title_source, anime_title_value,
    episode_no_source, episode_no_value,
    series_no_source, series_no_value,
    subtitle_group_source, subtitle_group_value,
    resolution_source, resolution_value
) VALUES (
    '六四位元 星號格式',
    '匹配以 ★ 分隔的格式',
    90,
    '^[^★]+★.+★\d+★',
    '^([^★]+)★(.+?)★(\d+)★(\d+x\d+)',
    'regex', '2',
    'regex', '3',
    'static', '1',     -- 固定為第 1 季
    'regex', '1',
    'regex', '4',
    NULL, NULL,
    NULL, NULL
);

-- 解析器 3: 預設解析器（最低優先權）
INSERT INTO title_parsers (
    name, description, priority,
    condition_regex, parse_regex,
    anime_title_source, anime_title_value,
    episode_no_source, episode_no_value,
    series_no_source, series_no_value,
    subtitle_group_source, subtitle_group_value
) VALUES (
    '預設解析器',
    '嘗試匹配任何包含 - 數字 的標題',
    1,
    '.+\s-\s\d+',
    '^(.+?)\s+-\s*(\d+)',
    'regex', '1',
    'regex', '2',
    'static', '1',
    'static', '未知字幕組',
    NULL, NULL,
    NULL, NULL
);
```

### 5.2 原始資料範例

```sql
-- 從 mikanani RSS 抓取的原始資料
INSERT INTO raw_anime_items (title, description, download_url, pub_date, subscription_id, status)
VALUES (
    '[LoliHouse] 黄金神威 最终章 / Golden Kamuy - 53 [WebRip 1080p HEVC-10bit AAC][无字幕]',
    '[LoliHouse] 黄金神威 最终章 / Golden Kamuy - 53 [WebRip 1080p HEVC-10bit AAC][无字幕][394.5MB]',
    'https://mikanani.me/Download/20260127/63a0419c3b8b8865111837f59f3b9bfb7cc79817.torrent',
    '2026-01-27 00:05:00',
    1,
    'pending'
);
```

---

## 六、實作任務清單

### 6.1 資料庫變更
- [ ] 建立 `raw_anime_items` 表
- [ ] 建立 `title_parsers` 表
- [ ] 修改 `anime_links` 表新增 `raw_item_id` 欄位
- [ ] 新增初始解析器資料

### 6.2 Shared 模組
- [ ] 新增 `RawAnimeItem` 結構
- [ ] 修改 `FetcherResultsPayload` 結構
- [ ] 移除舊的 `FetchedAnime`, `FetchedLink` 結構

### 6.3 Fetcher 修改
- [ ] 移除 `rss_parser.rs` 的 `parse_title()` 邏輯
- [ ] 修改 `parse_feed()` 只回傳原始資料
- [ ] 更新 `FetchTask` 使用新的資料格式

### 6.4 Core Service 修改
- [ ] 新增 `raw_anime_items` 的 model 和 schema
- [ ] 新增 `title_parsers` 的 model 和 schema
- [ ] 實作標題解析服務 (`services/title_parser.rs`)
- [ ] 修改 `handlers/fetcher_results.rs` 使用新流程
- [ ] 新增解析器管理 API
- [ ] 新增原始資料管理 API

### 6.5 測試
- [ ] 解析器邏輯單元測試
- [ ] API 整合測試
- [ ] 使用真實 RSS 資料驗證

---

## 七、版本歷程

| 版本 | 日期 | 變更內容 |
|------|------|---------|
| 1.0 | 2026-01-30 | 初版規格 |
