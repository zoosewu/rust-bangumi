# Resync 機制設計

## 問題

Parser CRUD 後觸發 reparse，會更新 `anime_links` 的 metadata（anime_title、series_no、episode_no、subtitle_group 等）。但已經 sync 到 Viewer 的 downloads 不會被重新通知，導致 Viewer 端的檔案名稱/路徑/NFO 與新 metadata 不一致。

## 設計方案：Viewer 端 resync endpoint

### 資料流

```
Parser CRUD
  → reparse_all_items()
  → upsert_anime_link() 更新 metadata，回報是否改變
  → 偵測已 synced downloads 的 metadata 變更
  → Core 發送 ViewerResyncRequest
  → Viewer 從自己 DB 查出檔案當前實際路徑
  → 搬移/重命名檔案 + 更新 NFO
  → callback Core 更新 target_path + status
```

### 改動範圍

#### 1. shared 模型

新增 `ViewerResyncRequest`：

```rust
pub struct ViewerResyncRequest {
    pub download_id: i32,
    pub series_id: i32,
    pub anime_title: String,
    pub series_no: i32,
    pub episode_no: i32,
    pub subtitle_group: String,
    pub old_target_path: String,   // Core 記錄的 target_path
    pub callback_url: String,
}
```

#### 2. Viewer 端

**Migration - sync_tasks 擴展：**

```sql
ALTER TABLE sync_tasks ADD COLUMN anime_title TEXT;
ALTER TABLE sync_tasks ADD COLUMN series_no INT;
ALTER TABLE sync_tasks ADD COLUMN subtitle_group TEXT;
ALTER TABLE sync_tasks ADD COLUMN task_type VARCHAR(10) NOT NULL DEFAULT 'sync';
```

**首次 sync 記錄 metadata：**
修改 `sync` handler，在建立 `NewSyncTask` 時存入 anime_title、series_no、subtitle_group。

**新增 `POST /resync` endpoint：**
1. 用 `download_id` 查 `sync_tasks` 表，取最新 completed 記錄的 `target_path`
2. 以 DB 中路徑為準（而非 Core 傳來的 `old_target_path`）
3. 根據新 metadata 計算新目標路徑
4. 路徑有變 → 搬移檔案；metadata 變 → 重新生成 NFO
5. bangumi_mapping 的 series_id 變了 → 重新查 bangumi.tv
6. 寫入新 sync_tasks 記錄（task_type = 'resync'）
7. callback Core

**清理空目錄：**
搬移檔案後，如果舊目錄變空，清理舊的 Season 目錄和 anime 目錄。

#### 3. Core 端

**upsert_anime_link 回傳擴展：**

```rust
struct UpsertResult {
    link_id: i32,
    is_new: bool,
    metadata_changed: bool,
}
```

比對更新前後的 series_id、group_id、episode_no 判斷 metadata_changed。

**reparse_affected_items 新增 resync 觸發：**
收集 `metadata_changed = true` 且 download status = `synced` 的項目，呼叫 `SyncService::notify_viewer_resync()`。

**SyncService 新增方法：**

```rust
pub async fn notify_viewer_resync(&self, download: &Download) -> Result<bool, String>
```

- 組建 ViewerResyncRequest（old_target_path = download.file_path）
- POST 到 Viewer `/resync`
- download 狀態改為 `resyncing`

**Download 狀態擴展：**

```
synced → resyncing → synced
                   → sync_failed
```

**sync_callback 無需改動：** 現有 handle_callback 已支援 synced/failed 回報。

### 流程範例

```
1. PUT /parsers/1 更新 Parser
2. reparse_all_items() 觸發
3. Item 原本: anime="進擊的巨人" → 新: anime="進撃の巨人"
4. upsert_anime_link: series_id 變了 → metadata_changed = true
5. 查 download: status="synced", file_path="/media/.../進擊的巨人 - S01E01.mkv"
6. Core → Viewer: ViewerResyncRequest
7. Viewer 查 sync_tasks: 最新 target_path 確認檔案位置
8. 搬移: .../進擊の巨人/Season 01/進撃の巨人 - S01E01.mkv
9. 重新生成 NFO
10. callback Core: status="synced", target_path="新路徑"
11. Core: download.file_path = 新路徑, status = "synced"
```
