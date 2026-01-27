# 訂閱系統廣播機制 - 測試指南

**完成日期：** 2026-01-27
**實現版本：** v1.0

---

## 概述

本文檔提供訂閱系統廣播機制的完整測試指南。廣播機制在創建訂閱時，向所有啟用的 Fetcher 詢問其能力，選擇優先級最高的能處理者，若無任何 Fetcher 能處理則拒絕創建。

## 實現完成狀態

### 編譯驗證
- ✅ Core Service 編譯成功（132MB debug, 優化版本 release）
- ✅ Mikanani Fetcher 編譯成功（93MB debug）
- ✅ 0 個編譯錯誤
- ✅ 所有庫測試通過（42/42）

### 核心功能
- ✅ 統一廣播函數（`broadcast_can_handle`）
- ✅ 自動選擇模式（無指定 Fetcher ID）
- ✅ 指定 Fetcher 模式（帶 Fetcher ID）
- ✅ 60 秒同步超時
- ✅ 優先級排序選擇
- ✅ 嚴格驗證（無能力拒絕）

### 相關文件
- 設計文檔：`2026-01-27-broadcast-mechanism-design.md`
- 實現計畫：`2026-01-27-broadcast-mechanism-implementation.md`

---

## 測試前置條件

### 環境準備

```bash
# 1. 確保 PostgreSQL 運行
psql -U postgres -c "SELECT version();"

# 2. 創建測試數據庫
createdb -U bangumi bangumi

# 3. 設置環境變數
export DATABASE_URL=postgresql://bangumi:bangumi_password@localhost:5432/bangumi
export CORE_SERVICE_URL=http://localhost:8000
export RUST_LOG=debug

# 4. 進入項目目錄
cd /workspace
```

### 數據庫遷移

```bash
# 執行所有待定遷移
diesel migration run

# 驗證 base_url 欄位已添加
psql -c "SELECT column_name FROM information_schema.columns WHERE table_name='fetcher_modules';" | grep base_url
```

### 編譯

```bash
# 構建 release 版本
cargo build --release

# 驗證二進制文件
ls -lh target/release/core-service target/release/fetcher-mikanani
```

---

## 測試場景

### 場景 1：自動選擇 - 單個 Fetcher 能處理

**前置條件：**
- Mikanani Fetcher 已啟動
- CORE Service 已啟動
- Fetcher 已向 CORE 註冊

**執行步驟：**

```bash
# 發送請求
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215",
    "name": "冬季新番"
  }'
```

**預期結果：**

```json
HTTP 201 Created

{
  "subscription_id": 1,
  "fetcher_id": 1,
  "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215",
  "name": "冬季新番",
  "source_type": "rss",
  "assignment_status": "auto_assigned",
  "auto_selected": true,
  "fetcher_id": 1,
  "is_active": true,
  "created_at": "2026-01-27T...",
  ...
}
```

**日誌驗證：**

CORE 日誌中應出現：
```
INFO: Fetcher 1 can handle: https://mikanani.me/RSS/Bangumi?bangumiId=3215 (priority: 100)
INFO: Created subscription 1 for URL https://mikanani.me/RSS/Bangumi?bangumiId=3215 with fetcher 1 (auto_assigned)
```

Mikanani 日誌中應出現：
```
INFO: Checking if can handle subscription: url=https://mikanani.me/RSS/Bangumi?bangumiId=3215, type=rss
INFO: can_handle_subscription result: can_handle=true
```

---

### 場景 2：指定 Fetcher - 該 Fetcher 能處理

**前置條件：**
- 場景 1 已完成（Fetcher ID=1）
- Mikanani Fetcher 運行中

**執行步驟：**

```bash
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "fetcher_id": 1,
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3216",
    "name": "另一部番劇"
  }'
```

**預期結果：**

```json
HTTP 201 Created

{
  "subscription_id": 2,
  "fetcher_id": 1,
  "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3216",
  "assignment_status": "assigned",
  "auto_selected": false,
  "assigned_at": "2026-01-27T...",
  ...
}
```

**日誌驗證：**

CORE 日誌：
```
INFO: Broadcasting can_handle to fetcher 1
INFO: Fetcher 1 can handle: https://mikanani.me/RSS/Bangumi?bangumiId=3216
INFO: Created subscription 2 for URL https://mikanani.me/RSS/Bangumi?bangumiId=3216 with fetcher 1 (assigned)
```

---

### 場景 3：自動選擇 - 無 Fetcher 能處理

**前置條件：**
- Mikanani Fetcher 運行中

**執行步驟：**

```bash
# URL 不包含 mikanani.me，Fetcher 無法處理
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "source_url": "https://example.com/feed.xml",
    "name": "非 Mikanani 源"
  }'
```

**預期結果：**

```json
HTTP 400 Bad Request

{
  "error": "no_capable_fetcher",
  "message": "No fetcher can handle this subscription request"
}
```

**日誌驗證：**

CORE 日誌：
```
WARN: Fetcher 1 cannot handle: https://example.com/feed.xml
WARN: No fetcher can handle subscription for URL: https://example.com/feed.xml (type: rss)
```

Mikanani 日誌：
```
DEBUG: Checking if can handle subscription: url=https://example.com/feed.xml, type=rss
DEBUG: can_handle_subscription result: can_handle=false
```

---

### 場景 4：指定 Fetcher - 無法處理

**執行步驟：**

```bash
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "fetcher_id": 1,
    "source_url": "https://example.com/feed.xml",
    "name": "非 Mikanani 源"
  }'
```

**預期結果：**

```json
HTTP 400 Bad Request

{
  "error": "no_capable_fetcher",
  "message": "No fetcher can handle this subscription request"
}
```

---

### 場景 5：重複訂閱 URL

**前置條件：**
- 場景 1 中已創建 `https://mikanani.me/RSS/Bangumi?bangumiId=3215`

**執行步驟：**

```bash
# 重複使用相同的 URL
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3215",
    "name": "重複訂閱"
  }'
```

**預期結果：**

```json
HTTP 409 Conflict

{
  "error": "already_exists",
  "message": "Subscription already exists for this URL: https://mikanani.me/RSS/Bangumi?bangumiId=3215"
}
```

---

## 數據庫驗證

### 訂閱記錄驗證

```bash
psql -U bangumi bangumi -c \
  "SELECT subscription_id, fetcher_id, source_url, assignment_status, auto_selected
   FROM subscriptions
   ORDER BY created_at DESC
   LIMIT 5;"
```

**預期輸出：**

```
 subscription_id | fetcher_id | source_url | assignment_status | auto_selected
-----------------+------------+--------------------------------------------+-------------------+---------------
               2 |          1 | https://mikanani.me/RSS/Bangumi?bangumiId=3216 | assigned    | f
               1 |          1 | https://mikanani.me/RSS/Bangumi?bangumiId=3215 | auto_assigned   | t
```

### Fetcher 模塊驗證

```bash
psql -U bangumi bangumi -c \
  "SELECT fetcher_id, name, priority, base_url
   FROM fetcher_modules;"
```

**預期輸出：**

```
 fetcher_id |      name       | priority |         base_url
------------+-----------------+----------+---------------------------
          1 | mikanani        |      100 | http://fetcher-mikanani:8001
```

---

## 性能驗證

### 廣播延遲測試

```bash
# 使用 time 命令測量 API 響應時間
time curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{"source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=3217", "name": "性能測試"}'
```

**預期結果：**
- 總耗時：< 2 秒（包括 HTTP 開銷）
- 廣播和決策：< 1 秒（同步操作）

### 並發測試

```bash
# 發送 5 個並發請求
for i in {1..5}; do
  curl -X POST http://localhost:8000/subscriptions \
    -H "Content-Type: application/json" \
    -d "{\"source_url\": \"https://mikanani.me/RSS/Test$i\", \"name\": \"並發測試 $i\"}" &
done
wait
```

**預期結果：**
- 所有 5 個請求均返回 201 Created
- 無競態條件或連接池耗盡

---

## 故障排查

### 問題 1：Fetcher 無法接收廣播請求

**症狀：** CORE 日誌出現 「request failed」或「timeout」

**檢查清單：**
```bash
# 1. Fetcher 進程是否運行
ps aux | grep fetcher-mikanani

# 2. Fetcher base_url 是否正確
psql -U bangumi bangumi -c "SELECT name, base_url FROM fetcher_modules;"

# 3. 網絡連接測試
curl http://fetcher-mikanani:8001/health

# 4. Fetcher 日誌
tail -50 fetcher-mikanani.log
```

### 問題 2：Subscription 創建返回 500 Internal Server Error

**症狀：** 響應包含 `"error": "broadcast_failed"`

**檢查清單：**
```bash
# 1. 數據庫連接
psql -U bangumi bangumi -c "SELECT version();"

# 2. Fetcher 表是否為空
psql -U bangumi bangumi -c "SELECT COUNT(*) FROM fetcher_modules WHERE is_enabled=true;"

# 3. CORE 日誌級別調整為 DEBUG
export RUST_LOG=debug
```

### 問題 3：廣播超時

**症狀：** 訂閱創建返回 500，日誌顯示「timeout」

**檢查清單：**
```bash
# 1. Fetcher 響應時間
time curl -X POST http://fetcher-mikanani:8001/can-handle-subscription \
  -H "Content-Type: application/json" \
  -d '{"source_url": "https://mikanani.me/test", "source_type": "rss"}'

# 2. 60 秒超時是否太短
# 若 Fetcher 響應 > 60 秒，需調整超時或檢查 Fetcher 性能
```

---

## 完整測試流程

```bash
#!/bin/bash
set -e

echo "=== 準備環境 ==="
export DATABASE_URL=postgresql://bangumi:bangumi_password@localhost:5432/bangumi
export RUST_LOG=debug

echo "=== 執行遷移 ==="
diesel migration run

echo "=== 編譯 ==="
cargo build --release

echo "=== 啟動服務 (背景) ==="
./target/release/core-service > core-service.log 2>&1 &
CORE_PID=$!

./target/release/fetcher-mikanani > fetcher-mikanani.log 2>&1 &
FETCHER_PID=$!

echo "等待服務啟動..."
sleep 2

echo "=== 場景 1：自動選擇 - Fetcher 能處理 ==="
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{"source_url":"https://mikanani.me/RSS/Test1","name":"測試1"}'

echo -e "\n=== 場景 3：無 Fetcher 能處理 ==="
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{"source_url":"https://example.com/test","name":"測試2"}'

echo -e "\n=== 數據庫驗證 ==="
psql -U bangumi bangumi -c "SELECT subscription_id, assignment_status, auto_selected FROM subscriptions;"

echo "=== 清理 ==="
kill $CORE_PID $FETCHER_PID
```

---

## 成功標準

廣播機制測試通過需滿足以下所有條件：

| 項目 | 標準 | 驗證 |
|------|------|------|
| 自動選擇成功 | 201 Created, assignment_status='auto_assigned' | ✓ 場景 1 |
| 指定 Fetcher 成功 | 201 Created, assignment_status='assigned' | ✓ 場景 2 |
| 無能力拒絕 | 400 Bad Request, error='no_capable_fetcher' | ✓ 場景 3、4 |
| 重複拒絕 | 409 Conflict, error='already_exists' | ✓ 場景 5 |
| 並發正確 | 無競態條件、數據一致 | ✓ 性能驗證 |
| 延遲可接受 | < 2 秒 | ✓ 性能驗證 |
| 數據庫正確 | 所有訂閱記錄符合預期 | ✓ 數據庫驗證 |
| 日誌完整 | 所有流程均有日誌 | ✓ 各場景日誌驗證 |

---

## 參考文檔

- [訂閱系統廣播機制設計](2026-01-27-broadcast-mechanism-design.md)
- [訂閱系統廣播機制實現計畫](2026-01-27-broadcast-mechanism-implementation.md)
