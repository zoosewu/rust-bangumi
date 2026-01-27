# 廣播機制功能測試指南

## 編譯狀態
✅ **編譯成功**
- Core Service: 132MB (debug)
- Mikanani Fetcher: 93MB (debug)
- 所有庫測試通過（26/26）
- 0 個編譯錯誤

## 前置條件

1. **PostgreSQL 數據庫**
   ```bash
   # 創建測試數據庫
   createdb -U bangumi bangumi_test

   # 設置環境變數
   export DATABASE_URL=postgresql://bangumi:bangumi_password@localhost:5432/bangumi
   ```

2. **執行數據庫遷移**
   ```bash
   cd /workspace
   diesel migration run
   ```

3. **啟動服務**
   ```bash
   # 終端 1：CORE Service
   ./target/debug/core-service

   # 終端 2：Mikanani Fetcher
   ./target/debug/fetcher-mikanani
   ```

## 測試場景 1：自動選擇 - 單個 Fetcher 能處理

**前置：** Mikanani Fetcher 已向 CORE 註冊

**測試請求：**
```bash
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215",
    "name": "Test Anime"
  }'
```

**預期結果：**
- HTTP 狀態碼：201 Created
- 響應中包含：
  - `"assignment_status": "auto_assigned"`
  - `"auto_selected": true`
  - `"fetcher_id": 1` (或已註冊的 Mikanani Fetcher ID)

**日誌驗證：**
- CORE 日誌：「Broadcasting can_handle to...」
- CORE 日誌：「Fetcher X can handle:...」
- CORE 日誌：「Created subscription X for URL...」
- Mikanani 日誌：「Checking if can handle subscription:...」
- Mikanani 日誌：「can_handle_subscription result: can_handle=true」

---

## 測試場景 2：指定 Fetcher - 能處理

**前置：** Mikanani Fetcher 已註冊，ID 為 1

**測試請求：**
```bash
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "fetcher_id": 1,
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3216",
    "name": "Test Anime 2"
  }'
```

**預期結果：**
- HTTP 狀態碼：201 Created
- 響應中包含：
  - `"assignment_status": "assigned"`
  - `"auto_selected": false`
  - `"assigned_at"` 時間戳非 null

**日誌驗證：**
- CORE 日誌：「Broadcasting can_handle to fetcher 1」
- CORE 日誌：「Fetcher 1 can handle:...」
- Mikanani 日誌：「can_handle_subscription result: can_handle=true」

---

## 測試場景 3：自動選擇 - 無 Fetcher 能處理

**前置：** Mikanani Fetcher 已啟用

**測試請求：**（URL 不包含 mikanani.me，所以 Mikanani 無法處理）
```bash
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "source_url": "https://example.com/feed.xml",
    "name": "Non-Mikanani Feed"
  }'
```

**預期結果：**
- HTTP 狀態碼：400 Bad Request
- 響應：
  ```json
  {
    "error": "no_capable_fetcher",
    "message": "No fetcher can handle this subscription request"
  }
```

**日誌驗證：**
- CORE 日誌：「Broadcasting can_handle to...」
- CORE 日誌：「Fetcher X cannot handle:...」
- CORE 日誌：「No fetcher can handle subscription for URL:...」
- Mikanani 日誌：「can_handle_subscription result: can_handle=false」

---

## 測試場景 4：指定 Fetcher - 無法處理

**前置：** Mikanani Fetcher 已註冊，ID 為 1

**測試請求：**
```bash
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "fetcher_id": 1,
    "source_url": "https://example.com/feed.xml",
    "name": "Non-Mikanani Feed"
  }'
```

**預期結果：**
- HTTP 狀態碼：400 Bad Request
- 響應：
  ```json
  {
    "error": "no_capable_fetcher",
    "message": "No fetcher can handle this subscription request"
  }
  ```

**日誌驗證：**
- CORE 日誌：「Broadcasting can_handle to fetcher 1」
- CORE 日誌：「Fetcher 1 cannot handle:...」
- Mikanani 日誌：「can_handle_subscription result: can_handle=false」

---

## 測試場景 5：重複訂閱

**前置：** 訂閱已存在於場景 1 中

**測試請求：**（重複使用相同的 URL）
```bash
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215",
    "name": "Test Anime Duplicate"
  }'
```

**預期結果：**
- HTTP 狀態碼：409 Conflict
- 響應：
  ```json
  {
    "error": "already_exists",
    "message": "Subscription already exists for this URL: ..."
  }
  ```

---

## 數據庫驗證

**查詢訂閱記錄：**
```bash
psql -U bangumi bangumi -c "SELECT subscription_id, fetcher_id, source_url, assignment_status, auto_selected FROM subscriptions ORDER BY created_at DESC LIMIT 5;"
```

**預期結果：**
- 應該看到多個記錄
- 自動選擇的訂閱：`assignment_status = 'auto_assigned'`, `auto_selected = true`
- 指定 Fetcher 的訂閱：`assignment_status = 'assigned'`, `auto_selected = false`

**查詢 Fetcher 模塊：**
```bash
psql -U bangumi bangumi -c "SELECT fetcher_id, name, priority, base_url FROM fetcher_modules;"
```

**預期結果：**
- 應該看到 Mikanani Fetcher 記錄
- `base_url` 應該是 `http://fetcher-mikanani:8001` 格式

---

## 性能監測

**CORE Service 啟動日誌：**
```
[2026-01-27T...] INFO: Core service listening on 0.0.0.0:8000
[2026-01-27T...] INFO: Loaded subscription broadcaster
```

**Mikanani Fetcher 啟動日誌：**
```
[2026-01-27T...] INFO: Mikanani fetcher service listening on ...
[2026-01-27T...] INFO: 已向核心服務註冊
```

**廣播完成時間：**
- 單個訂閱創建：通常 < 1 秒（同步廣播 60 秒超時）
- 多個 Fetcher 場景：應該並發查詢，所有響應時間的最大值 < 超時

---

## 故障排查

### 場景 1：Fetcher 無法接收廣播請求

**症狀：** CORE 日誌顯示「request failed」或「timeout」

**檢查：**
1. Mikanani Fetcher 是否正在運行
2. Firewall 是否允許 CORE → Mikanani 通信
3. Mikanani 的 base_url 是否正確：
   ```bash
   psql -U bangumi bangumi -c "SELECT name, base_url FROM fetcher_modules WHERE name='mikanani';"
   ```

### 場景 2：Subscription 創建返回 500 Internal Server Error

**症狀：** 響應中含有 `"error": "broadcast_failed"`

**檢查：**
1. 數據庫連接是否正常
2. Fetcher 列表是否為空：
   ```bash
   psql -U bangumi bangumi -c "SELECT COUNT(*) FROM fetcher_modules WHERE is_enabled=true;"
   ```
3. Fetcher 的 base_url 是否為空
4. 查看 CORE 日誌中的詳細錯誤信息

### 場景 3：Fetcher can_handle 端點返回 400+

**症狀：** CORE 日誌顯示 Fetcher 無法解析請求

**檢查：**
1. Mikanani 是否正確實現了 `/can-handle-subscription` 端點
2. 請求體格式是否正確：
   ```json
   {
     "source_url": "...",
     "source_type": "rss"
   }
   ```
3. Mikanani 日誌中是否有解析錯誤

---

## 完整測試流程

```bash
# 1. 啟動數據庫
pg_ctl start

# 2. 準備環境
export DATABASE_URL=postgresql://bangumi:bangumi_password@localhost:5432/bangumi
cd /workspace

# 3. 運行遷移
diesel migration run

# 4. 編譯
cargo build --release

# 5. 在 3 個終端運行
# 終端 1
./target/release/core-service

# 終端 2
./target/release/fetcher-mikanani

# 終端 3 - 執行測試
# 場景 1
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{"source_url":"https://mikanani.me/RSS/Bangumi?bangumiId=3215","name":"Test"}'

# 場景 3
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{"source_url":"https://example.com/feed.xml","name":"Test"}'

# 6. 查看數據庫
psql -U bangumi bangumi -c "SELECT * FROM subscriptions ORDER BY created_at DESC LIMIT 2;"
```

---

## 預期完整流程

```
客戶端請求創建訂閱
↓
CORE create_subscription() 檢查重複
↓
CORE 調用 broadcast_can_handle(target_fetcher_id=None)
↓
廣播到所有啟用的 Fetcher（並發）
    ├─ Fetcher 1: POST /can-handle-subscription (5ms)
    ├─ Fetcher 2: POST /can-handle-subscription (10ms)
    └─ Fetcher 3: POST /can-handle-subscription (8ms)
↓
等待所有響應或 60 秒超時（此例 ~15ms）
↓
收集能處理的 Fetcher，按優先級排序
↓
選擇優先級最高的 Fetcher
↓
插入訂閱記錄
↓
返回 201 Created
```

---

## 完成標準

- ✅ 自動選擇場景：訂閱創建成功，assignment_status='auto_assigned'
- ✅ 指定 Fetcher 場景：訂閱創建成功，assignment_status='assigned'
- ✅ 無能力拒絕場景：返回 400，error='no_capable_fetcher'
- ✅ 廣播並發工作：多個 Fetcher 同時被查詢
- ✅ 優先級選擇：最高優先級 Fetcher 被選中
- ✅ 日誌完整：所有流程都有適當的日誌記錄
- ✅ 數據庫正確：訂閱記錄具有正確的 assignment_status 和 auto_selected 值
