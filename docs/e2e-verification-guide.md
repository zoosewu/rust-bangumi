# 新架構端對端驗證指南

本文檔描述如何驗證新的 Fetcher 架構（原始數據 → 中央解析）是否運作正常。

## 前置條件

1. 確保 PostgreSQL 資料庫已運行
2. 確保所有編譯錯誤已解決
3. Core Service 應監聽 `http://localhost:3000`

## 驗證步驟

### Step 1: 啟動 Core Service

```bash
cd /workspace/core-service
cargo run --release
```

預期輸出：
```
核心服務監聽於 0.0.0.0:8000
```

### Step 2: 驗證解析器 API

#### 列出預設解析器
```bash
curl -s http://localhost:3000/parsers | jq
```

預期結果：
```json
[
  {
    "parser_id": 1,
    "name": "LoliHouse 標準格式",
    "description": "匹配 [字幕組] 動畫名稱 - 集數 [解析度] 格式",
    "priority": 100,
    "is_enabled": true,
    ...
  },
  {
    "parser_id": 2,
    "name": "六四位元 星號格式",
    "priority": 90,
    ...
  },
  {
    "parser_id": 3,
    "name": "預設解析器",
    "priority": 1,
    ...
  }
]
```

#### 獲取單一解析器
```bash
curl -s http://localhost:3000/parsers/1 | jq
```

#### 新增自訂解析器
```bash
curl -X POST http://localhost:3000/parsers \
  -H "Content-Type: application/json" \
  -d '{
    "name": "測試解析器",
    "description": "測試用",
    "priority": 50,
    "condition_regex": "^\\[.+\\]",
    "parse_regex": "^\\[([^\\]]+)\\]\\s*(.+?)\\s+-\\s*(\\d+)",
    "anime_title_source": "regex",
    "anime_title_value": "2",
    "episode_no_source": "regex",
    "episode_no_value": "3"
  }' | jq
```

### Step 3: 模擬 Fetcher 回傳原始數據

```bash
curl -X POST http://localhost:3000/raw-fetcher-results \
  -H "Content-Type: application/json" \
  -d '{
    "subscription_id": 1,
    "items": [
      {
        "title": "[LoliHouse] 黄金神威 最终章 / Golden Kamuy - 53 [WebRip 1080p HEVC-10bit AAC][无字幕]",
        "description": "第53話",
        "download_url": "https://example.com/anime/file.torrent",
        "pub_date": "2026-01-31T12:00:00Z"
      },
      {
        "title": "[六四位元] 某動畫 - 12 [1920x1080]",
        "description": null,
        "download_url": "https://example.com/anime/file2.torrent",
        "pub_date": "2026-01-31T13:00:00Z"
      },
      {
        "title": "無法匹配的標題格式",
        "description": null,
        "download_url": "https://example.com/anime/file3.torrent",
        "pub_date": null
      }
    ],
    "fetcher_source": "mikanani",
    "success": true,
    "error_message": null
  }'
```

預期結果：
```json
{
  "success": true,
  "items_received": 3,
  "items_parsed": 2,
  "items_failed": 1,
  "message": "Processed 3 items: 2 parsed, 1 failed"
}
```

### Step 4: 驗證原始數據已儲存

#### 列出所有原始項目
```bash
curl -s "http://localhost:3000/raw-items" | jq
```

#### 按狀態過濾
```bash
curl -s "http://localhost:3000/raw-items?status=parsed" | jq
curl -s "http://localhost:3000/raw-items?status=no_match" | jq
```

#### 按訂閱 ID 過濾
```bash
curl -s "http://localhost:3000/raw-items?subscription_id=1" | jq
```

### Step 5: 驗證解析結果

#### 獲取單一項目詳情
```bash
curl -s "http://localhost:3000/raw-items/1" | jq
```

預期看到：
- `status`: "parsed" 或 "no_match"
- `parser_id`: 使用的解析器 ID
- `error_message`: 錯誤信息（如果失敗）
- `parsed_at`: 解析時間戳

### Step 6: 重新解析項目

#### 重新解析失敗的項目
```bash
curl -X POST http://localhost:3000/raw-items/3/reparse | jq
```

預期：如果現在有匹配的解析器，狀態應更新為 "parsed"

#### 標記項目為跳過
```bash
curl -X POST http://localhost:3000/raw-items/3/skip
```

### Step 7: 驗證動畫記錄已建立

檢查 anime_links 表是否包含從 raw_anime_items 建立的記錄：

```bash
# 查詢包含 raw_item_id 的連結
psql -U bangumi -d bangumi -c "SELECT * FROM anime_links WHERE raw_item_id IS NOT NULL LIMIT 5;"
```

## 預期的數據流

```
Fetcher (RSS)
    ↓
[原始標題 1, 原始標題 2, 原始標題 3]
    ↓
POST /raw-fetcher-results
    ↓
Core Service:
  1. 存儲到 raw_anime_items (status=pending)
  2. 使用 TitleParserService 解析標題
  3. 對於成功的解析：
     - 建立或獲取 anime, series, season, group
     - 建立 anime_links 記錄
     - 更新 raw_anime_items status=parsed
  4. 對於失敗的解析：
     - 更新 raw_anime_items status=no_match/failed
    ↓
/raw-items API 允許查看和重新解析
```

## 解析失敗的故障排查

### 情況 1: 新標題格式無法解析

1. 檢查日誌中的错誤信息
2. 建立新的解析器：
   ```bash
   # 確定標題格式
   # 建立匹配該格式的 condition_regex 和 parse_regex
   # 使用 POST /parsers 新增
   ```
3. 使用 POST /raw-items/:id/reparse 重新解析

### 情況 2: 特定解析器優先級不對

1. 查看 /parsers 的優先級
2. 使用 DELETE /parsers/:id 刪除低優先級的解析器
3. 重新解析項目

## 性能測試

### 批量導入測試

測試 1000 個原始項目的処理：

```bash
# 生成包含 1000 個項目的 payload
# 發送 POST /raw-fetcher-results

# 驗證：
curl -s "http://localhost:3000/raw-items?limit=1000" | jq '.[] | length'
```

### 解析性能

檢查平均解析時間：

```bash
psql -U bangumi -d bangumi -c "
SELECT
  status,
  COUNT(*) as count,
  ROUND(AVG(EXTRACT(EPOCH FROM (parsed_at - created_at)))::numeric, 2) as avg_parse_secs
FROM raw_anime_items
GROUP BY status;
"
```

## 回滾計劃

如果新架構有問題，可以：

1. 臨時禁用所有解析器：
   ```bash
   # 更新 title_parsers 設置 is_enabled = false
   psql -U bangumi -d bangumi -c "UPDATE title_parsers SET is_enabled = false;"
   ```

2. 使用舊的 `/fetcher-results` endpoint 直接創建 anime_links

3. 手動修復 raw_anime_items 表中的記錄

## 驗證清單

- [ ] 3 個預設解析器已建立
- [ ] 可以列出、獲取、建立、刪除解析器
- [ ] 原始項目成功儲存到 raw_anime_items
- [ ] 解析成功的項目轉換為 anime_links
- [ ] 解析失敗的項目可以重新解析
- [ ] 原始項目可以標記為跳過
- [ ] 性能在可接受範圍內（< 100ms 每項）

## 常見問題

**Q: 為什麼有些項目狀態是 "no_match"？**
A: 沒有任何啟用的解析器匹配該標題格式。需要新增或修改解析器。

**Q: 如何修改現有解析器？**
A: 目前需要刪除後重新建立。未來可能添加 PUT /parsers/:id 端點。

**Q: raw_anime_items 和 anime_links 的區別？**
A: raw_anime_items 是原始的、未解析的 RSS 數據。anime_links 是解析成功後建立的可用記錄。

**Q: 為什麼需要在 Core Service 中解析？**
A: 這樣 Fetcher 可以無狀態、高效地抓取任何 RSS 源。解析邏輯集中在 Core 中，易於修改和測試。
