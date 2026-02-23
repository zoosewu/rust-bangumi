# CLI 設計文件

**日期：** 2026-02-23
**狀態：** 已批准

## 概述

重寫 `bangumi` CLI 工具，完整對應後端所有 API 端點。CLI 設計供日常運維操作與自動化腳本兩用途，提供 human-readable 表格輸出（預設）與 JSON 輸出（`--json` flag）。

---

## 1. 全域選項

```
bangumi [OPTIONS] <COMMAND>

Options:
  --api-url <URL>    Core Service base URL
                     預設: http://localhost:8000
                     環境變數: BANGUMI_API_URL
  --json             輸出原始 JSON（適合管道/腳本）
  -h, --help         顯示說明
  -V, --version      顯示版本
```

---

## 2. 指令總覽

| 完整指令 | 別名 | 說明 |
|---------|------|------|
| `status` | `st` | Dashboard 統計 + 服務健康狀態 |
| `subscription` | `sub` | RSS 訂閱管理 |
| `anime` | — | 動畫條目管理 |
| `series` | — | 動畫系列查詢與管理 |
| `raw-item` | `raw` | Raw RSS 項目瀏覽與操作 |
| `conflict` | — | 衝突列表與解決 |
| `download` | `dl` | 下載記錄查詢 |
| `filter` | — | 過濾規則管理 |
| `parser` | — | 標題解析器管理 |
| `subtitle-group` | `sg` | 字幕組管理 |
| `qb-config` | — | qBittorrent 連線設定 |

---

## 3. 各資源子指令詳細規格

### 3.1 `status` (別名 `st`)

```
bangumi status
```

- 呼叫 `GET /dashboard/stats`
- 顯示：動畫總數、系列總數、活躍訂閱數、下載中/已完成/失敗、待解析 raw items、待解決衝突
- 顯示各服務健康狀態（Fetcher/Downloader/Viewer）

---

### 3.2 `subscription` (別名 `sub`)

```
bangumi subscription list [--status active|inactive]
bangumi subscription add <url> [--name <n>] [--interval <minutes>]
bangumi subscription show <id>
bangumi subscription update <id> [--name <n>] [--interval <min>] [--active|--inactive]
bangumi subscription delete <id> [--purge]
```

API 對應：
- `list` → `GET /subscriptions`
- `add` → `POST /subscriptions`，body: `{source_url, name?, fetch_interval_minutes?}`
- `show` → `GET /subscriptions`（以 id 篩選）
- `update` → `PATCH /subscriptions/:id`，body: `{name?, fetch_interval_minutes?, is_active?}`
- `delete` → `DELETE /subscriptions/:id?purge=<bool>`

---

### 3.3 `anime`

```
bangumi anime list
bangumi anime add <title>
bangumi anime delete <id>
bangumi anime series <anime_id>   # 列出某動畫的所有系列
```

API 對應：
- `list` → `GET /anime`
- `add` → `POST /anime`，body: `{title}`
- `delete` → `DELETE /anime/:id`
- `series` → `GET /anime/:id/series`

---

### 3.4 `series`

```
bangumi series list [--anime <anime_id>]
bangumi series show <id>
bangumi series update <id> [--description <s>] [--aired-date <date>] [--end-date <date>] [--season-id <id>]
bangumi series links <id>         # 列出集數連結（含下載狀態）
```

API 對應：
- `list` → `GET /series`（或 `GET /anime/:id/series`）
- `show` → `GET /anime/series/:id`
- `update` → `PUT /anime/series/:id`
- `links` → `GET /links/:series_id`

---

### 3.5 `raw-item` (別名 `raw`)

```
bangumi raw-item list [--status pending|parsed|no_match|failed|skipped] [--sub <id>] [--limit <n>] [--offset <n>]
bangumi raw-item show <id>
bangumi raw-item reparse <id>
bangumi raw-item skip <id>
```

API 對應：
- `list` → `GET /raw-items?status=&subscription_id=&limit=&offset=`
- `show` → `GET /raw-items/:id`
- `reparse` → `POST /raw-items/:id/reparse`
- `skip` → `POST /raw-items/:id/skip`

---

### 3.6 `conflict`

```
bangumi conflict list              # 列出所有衝突（訂閱衝突 + link 衝突）
bangumi conflict resolve <id> --fetcher <fetcher_id>
bangumi conflict resolve-link <id> --link <link_id>
```

API 對應：
- `list` → `GET /conflicts` + `GET /link-conflicts`（合併顯示）
- `resolve` → `POST /conflicts/:id/resolve`，body: `{fetcher_id}`
- `resolve-link` → `POST /link-conflicts/:id/resolve`，body: `{chosen_link_id}`

---

### 3.7 `download` (別名 `dl`)

```
bangumi download list [--status <s>] [--limit <n>] [--offset <n>]
```

`status` 可選值：`downloading`、`completed`、`failed`、`paused`

API 對應：
- `list` → `GET /downloads?status=&limit=&offset=`

---

### 3.8 `filter`

```
bangumi filter list [--type global|anime|series|group|fetcher] [--target <id>]
bangumi filter add --type <t> [--target <id>] --regex <pattern> [--negative] [--order <n>]
bangumi filter delete <id>
bangumi filter preview --type <t> [--target <id>] --regex <pattern> [--positive|--negative]
```

API 對應：
- `list` → `GET /filters?target_type=&target_id=`
- `add` → `POST /filters`，body: `{target_type, target_id?, rule_order, is_positive, regex_pattern}`
- `delete` → `DELETE /filters/:id`
- `preview` → `POST /filters/preview`

---

### 3.9 `parser`

```
bangumi parser list [--type <created_from_type>] [--target <id>]
bangumi parser show <id>
bangumi parser add --name <n> [--condition <regex>] [--parse-regex <regex>] [--priority <n>] [...]
bangumi parser update <id> [same options as add]
bangumi parser delete <id>
bangumi parser preview [--id <id>] [options...]
```

API 對應：
- `list` → `GET /parsers?created_from_type=&created_from_id=`
- `show` → `GET /parsers/:id`
- `add` → `POST /parsers`
- `update` → `PUT /parsers/:id`
- `delete` → `DELETE /parsers/:id`
- `preview` → `POST /parsers/preview`

---

### 3.10 `subtitle-group` (別名 `sg`)

```
bangumi subtitle-group list
bangumi subtitle-group add <name>
bangumi subtitle-group delete <id>
```

API 對應：
- `list` → `GET /subtitle-groups`
- `add` → `POST /subtitle-groups`，body: `{group_name}`
- `delete` → `DELETE /subtitle-groups/:id`

---

### 3.11 `qb-config`

```
bangumi qb-config set-credentials --user <u> --password <p> [--downloader-url <url>]
```

API 對應（直接呼叫 Downloader，非 Core）：
- `set-credentials` → `POST /config/credentials`，body: `{username, password}`
- `--downloader-url` 預設 `http://localhost:8002`，環境變數 `BANGUMI_DOWNLOADER_URL`

---

## 4. 輸出格式

### 預設（Human-readable）
- 列表使用表格格式（`tabled` crate）
- 關鍵狀態標記用顏色（`colored`）：綠色=成功/啟用，黃色=進行中，紅色=失敗/衝突
- 單筆詳情以 key-value 格式顯示

### JSON 模式（`--json`）
- 直接序列化 API response 結果
- 不加任何裝飾，適合 `jq` 管道

### 退出碼
- `0`：成功
- `1`：使用者輸入錯誤（缺少參數等）
- `2`：API 錯誤（4xx/5xx）
- `3`：連線失敗

---

## 5. 技術選型

| 元件 | 套件 | 說明 |
|------|------|------|
| CLI 框架 | `clap` v4 (derive) | 自動生成 help，支援別名 |
| HTTP 客戶端 | `reqwest` | async HTTP，支援 JSON body |
| Async runtime | `tokio` | 搭配 reqwest |
| 表格輸出 | `tabled` | 彈性的 Rust 表格庫 |
| 彩色輸出 | `colored` | 跨平台終端顏色 |
| 錯誤處理 | `anyhow` | 簡潔的 error chain |
| 序列化 | `serde_json` | JSON 序列化/反序列化 |

---

## 6. 目錄結構

```
cli/
├── Cargo.toml
└── src/
    ├── main.rs              # 入口點，全域 args 解析與 dispatch
    ├── client.rs            # ApiClient struct（base_url, json flag, HTTP methods）
    ├── output.rs            # OutputFormatter（print_table / print_json / print_kv）
    ├── error.rs             # CliError enum，退出碼對應
    └── commands/
        ├── mod.rs           # Commands enum，所有子指令 dispatch
        ├── status.rs        # status 指令
        ├── subscription.rs  # subscription 指令
        ├── anime.rs         # anime 指令
        ├── series.rs        # series 指令
        ├── raw_item.rs      # raw-item 指令
        ├── conflict.rs      # conflict 指令
        ├── download.rs      # download 指令
        ├── filter.rs        # filter 指令
        ├── parser.rs        # parser 指令
        ├── subtitle_group.rs # subtitle-group 指令
        └── qb_config.rs     # qb-config 指令
```

---

## 7. 錯誤處理策略

- HTTP 4xx：解析 response body 取得錯誤訊息，顯示給用戶
- HTTP 5xx：顯示 `Service error (HTTP <code>)`
- 連線失敗：顯示 `Cannot connect to <url>. Is the service running?`
- 解析失敗：顯示 raw response 供 debug

---

## 8. Help 實作規範

- 每個 subcommand 設定 `about`（一行摘要）與 `long_about`（詳細說明）
- 每個 argument 設定 `help` 說明文字
- `bangumi --help`：顯示所有頂層指令與別名
- `bangumi subscription --help`：顯示 subscription 子指令列表
- `bangumi subscription add --help`：顯示所有參數說明

---

## 9. 環境變數

| 環境變數 | 對應選項 | 說明 |
|---------|---------|------|
| `BANGUMI_API_URL` | `--api-url` | Core Service URL |
| `BANGUMI_DOWNLOADER_URL` | `--downloader-url` | Downloader Service URL（qb-config 用） |
