# Manual Download Retry — Design Spec

**Date**: 2026-04-29
**Status**: Approved (pending implementation plan)

## 1. 範圍與目標

新增**使用者手動重試下載**功能。允許從 UI 觸發單筆或批次重試，重試動作沿用既有 `DownloadDispatchService::dispatch_new_links`，不修改其內部行為，因此每次重試都會 INSERT 新的 download row、保留歷史。

**可重試 status 集合**：`failed`、`downloader_error`、`no_downloader`、`cancelled`。

不在此次範圍：
- `batch_failed` / `sync_failed` 屬於後處理失敗（檔案匹配 / 同步），語義不同，未來如要支援應走獨立的「重新匹配」/「重新同步」端點
- 重試次數上限 / 退避策略（依現況不需要）
- Webhook / 通知整合
- 前端 UI（後續另案）

## 2. API 規格

### 2.1 單筆重試

```
POST /downloads/:download_id/retry
```

- 路徑參數：`download_id: i32`
- 無 request body

**Response 200**：
```json
{ "download_id": 105, "link_id": 508, "status": "dispatched" }
```

**錯誤回應**：
- `404 download_not_found` — download 不存在
- `409 not_retryable` — status 不在可重試集合（response 帶當前 status）
- `409 link_not_dispatchable` — link 已被 conflict / filter / link_status='resolved' / 批次衝突傳播擋住
- `500 dispatch_failed` — 內部錯誤

### 2.2 批次重試

```
POST /downloads/retry
```

**Optional body**（全部欄位可省）：
```json
{
  "download_ids": [101, 102],
  "status": ["failed", "cancelled"],
  "downloader_type": "magnet"
}
```

三種篩選為 AND 結合。系統一律額外卡上「status ∈ retryable 集合」，故 `status` 欄位若給了 `completed` 也不會誤動。

**Response 200**：
```json
{
  "downloads_matched": 12,        // 篩選命中的 download 筆數
  "not_retryable": 0,             // 命中後被擋掉的（status 不在 retryable）
  "unique_links": 10,             // dedup 後送進 dispatch 的 link 數
  "dispatched": 9,                // dispatch 成功派出的 link 數
  "no_downloader": 1,             // 無可用 downloader
  "conflict_or_filtered": 0,      // 被 conflict / filter / resolved / 批次傳播擋掉
  "failed": 0
}
```

> 計數語義：`downloads_matched` / `not_retryable` 是 download 級；`unique_links` 之後皆為 link 級。前端可同時顯示「12 筆失敗下載 → 9 個 link 已重派」。

## 3. 實作邏輯

### 3.1 Service 層

在 `core-service/src/services/download_dispatch.rs` 的 `DownloadDispatchService` 新增：

```rust
pub struct RetryResult {
    pub downloads_matched: usize,
    pub not_retryable: usize,
    pub unique_links: usize,
    pub dispatched: usize,
    pub no_downloader: usize,
    pub conflict_or_filtered: usize,
    pub failed: usize,
}

pub async fn manual_retry(
    &self,
    download_ids: Vec<i32>,
) -> Result<RetryResult, String>
```

流程：
1. 載入指定 `download_ids` 對應的 downloads → `downloads_matched`
2. 用純函式 `partition_retryable(&[Download]) -> (retryable_downloads, not_retryable_count)` 分流
3. 從 retryable_downloads 收集 link_id 並 dedup → `unique_links`
4. 呼叫既有 `dispatch_new_links(link_ids)` → 取得 `dispatched / no_downloader / failed`
5. `conflict_or_filtered = unique_links - dispatched - no_downloader - failed`

**可重試 status 常數**：將 `["cancelled", "failed", "no_downloader"]` 擴充為 `["cancelled", "failed", "no_downloader", "downloader_error"]`，並抽成 `services` 模組 (或 `dispatch` 內部) `pub const RETRYABLE_STATUSES`，供 service / handler 共用。

### 3.2 Handler 層

新檔或加在 `core-service/src/handlers/downloads.rs`：

#### `retry_one(download_id)` — 對應 `POST /downloads/:download_id/retry`
1. `state.dispatch_service.manual_retry(vec![download_id]).await`
2. 翻譯結果：
   - `downloads_matched == 0` → 讀一次 download 確認存在 → 404 vs 409 not_retryable
   - `dispatched == 1` → 200 status="dispatched"
   - `dispatched == 0 && conflict_or_filtered == 1` → 409 link_not_dispatchable
   - `dispatched == 0 && no_downloader == 1` → 200 status="no_downloader"
   - `failed > 0` → 500 dispatch_failed

#### `retry_bulk(query)` — 對應 `POST /downloads/retry`
1. 用 query 把 filter（download_ids / status / downloader_type）轉成 download_id 列表，預先卡 retryable 集合：
   ```rust
   let mut q = downloads::table.into_boxed();
   q = q.filter(downloads::status.eq_any(RETRYABLE_STATUSES));
   if let Some(ids) = &req.download_ids { q = q.filter(downloads::download_id.eq_any(ids)); }
   if let Some(s) = &req.status         { q = q.filter(downloads::status.eq_any(s)); }
   if let Some(dt) = &req.downloader_type { q = q.filter(downloads::downloader_type.eq(dt)); }
   ```
2. `manual_retry(those_ids)`
3. 直接回 `RetryResult` JSON

### 3.3 Route 註冊

在 `main.rs` 註冊：
```rust
.route("/downloads/retry", post(handlers::downloads::retry_bulk))
.route("/downloads/:download_id/retry", post(handlers::downloads::retry_one))
```

### 3.4 OpenAPI

- 在 `core-service/src/openapi.rs` 補新 DTO
- 在 `docs/api/openapi.yaml` 加兩個 path + 對應 schemas

## 4. 邊界情境

| 情境 | 行為 |
|------|------|
| download_id 不存在 | 單筆 → 404；批次 → 不算 candidate |
| Download 在 active status (pending/downloading/syncing/...) | 計 not_retryable；單筆 → 409 |
| Link `conflict_flag=true` | dispatch 跳過，計 conflict_or_filtered |
| Link `filtered_flag=true` | 同上 |
| Link `link_status='resolved'` | 同上 |
| Link 來源 raw_item 有兄弟衝突中（既有批次傳播） | 同上 |
| 無可用 downloader | dispatch 內部 INSERT 一筆 status='no_downloader' 的新 row，計 no_downloader |
| 同 link 在批次出現多次（不同 download_id） | service 層去重 link_id，dispatch 只跑一次 |
| 批次篩選結果為空 | 200 回 candidates=0；不報 error |
| Connection pool 取不到連線 | 500 + error message |

### Logging
- 單筆：`tracing::info!` 紀錄 download_id / link_id / 結果
- 批次：紀錄 candidates / dispatched / 各類 skipped；超過 100 筆時降為 debug 細節

## 5. 測試

### Unit tests (`download_dispatch.rs::tests`)
- `partition_retryable` 純函式測試：覆蓋每個 retryable status / 各種 active status / 空輸入
- `manual_retry` 不做完整測試（涉及 HTTP 呼叫 downloader），靠純函式 + 手動驗證

### Integration / Handler tests
- 不做（binary crate 限制）

### 手動驗證
```bash
# 1. 製造 failed 下載
docker exec bangumi-postgres-dev psql -U bangumi -d bangumi -c \
  "UPDATE downloads SET status='failed', error_message='test' WHERE download_id=105;"

# 2. 單筆重試
xh POST localhost:8000/downloads/105/retry

# 3. 批次重試（只重試 magnet 的失敗）
xh POST localhost:8000/downloads/retry status:='["failed"]' downloader_type=magnet
```

## 6. 不變式 / 非目標

- 不修改 `dispatch_new_links` 既有行為
- 不引入新的 download status
- 不加重試次數上限（dispatch 本身允許多次重派；使用者按多少次都行）
- 不做認證 / 授權（內網單人服務）
- 不做前端 UI（spec 只涵蓋後端）

## 7. 檔案影響清單

- `core-service/src/services/download_dispatch.rs` — 新增 `manual_retry`、`partition_retryable`、`RetryResult`、`RETRYABLE_STATUSES`
- `core-service/src/handlers/downloads.rs` — 新增 `retry_one`、`retry_bulk`、相關 request DTO
- `core-service/src/dto.rs` — 新增 `RetryBulkRequest`、`RetryResultResponse`、`RetryOneResponse`
- `core-service/src/openapi.rs` — 註冊新 schemas
- `core-service/src/main.rs` — 註冊兩個 route
- `docs/api/openapi.yaml` — 路徑 + schema 文件
