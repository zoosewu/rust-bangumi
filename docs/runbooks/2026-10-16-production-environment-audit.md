# 2026-10-16 生產環境稽核：異常狀態的根因與觀察依據

> 範圍：yeh3f@192.168.68.101（macOS + OrbStack），部署版本 816a09f。
> 調查方式全程唯讀（DB 查詢、docker logs、qB 狀態 API、檔案列表）。
> 本文件是之後觀察環境的基準：每一項都附上「如何檢查」與「正常時應看到什麼」。

## 總覽

| # | 現象 | 根因層級 | 嚴重度 |
|---|------|----------|--------|
| E1 | PostgreSQL 索引遺失資料、主鍵重複、序列倒退 | 儲存層（環境） | **最高** |
| E2 | 合集全部 `batch_failed`、`video_file` 永遠是 NULL、資料夾型 torrent 同步失敗 | 部署設定（volume） | 高 |
| E3 | `downloader_error` 的下載在 qB 已完成卻永遠不會恢復 | 程式 | 高 |
| E4 | `downloading` 但 qB 找不到（`not_found`），永遠卡住 | 程式 | 中 |
| E5 | 同集的多個版本都被下載，之後因為衝突旗標永遠不同步 | 程式 + 待使用者處理的衝突 | 中 |
| E6 | 同步其實成功了，卻被記成 `sync_failed` | 程式（core 重複送同步 + 後到的失敗蓋掉成功 + viewer 不冪等） | 中 |
| E7 | /downloads 裡的孤兒檔案 | 程式（取消時不刪檔） | 低 |
| E8 | qB 留著 122 個已同步的 torrent（含 94 個 missingFiles） | 程式（第 1 項已修） | 低 |
| E10 | 冷門、舊集數的下載永遠 0%（連不到 NAT 後面的 peer） | 網路環境（路由器沒有轉發 16881） | 中 |

---

## E1：PostgreSQL 資料損壞（儲存層）

> **狀態：已修復（2026-10-16）**：資料庫已搬到 named volume `rust-bangumi_postgres_data`，並開啟 data_checksums，驗證全部通過。過程與重啟後的觀察記在 `tasks/todo.md` 階段 0。

### 證據

1. **主鍵重複**（主鍵索引存在，資料卻重複，代表索引當時沒有這些值）
   - `viewer_jellyfin.sync_tasks`：221 筆資料，只有 219 個不同的 task_id；142、144 各出現兩次。
   - `bangumi.cron_logs`：1165 個 log_id 重複（範圍 40342–53379）。
2. **索引缺少資料列**（seq scan 和 PK index scan 的筆數不同）
   - `bangumi.downloads`：seq 250 / idx 249，缺的是 **download 505（Mujikaku Seijo ep7 ABEMA，就是使用者看到「一直在下載」的那集）**；`idx_downloads_torrent_hash` 也找不到它。
   - `bangumi.cron_logs`：seq 54529 / idx 52199，缺少的資料橫跨 2026-09-05 到 2026-10-12，表示損壞**不是單一事件，而是持續在發生**。
   - `viewer_jellyfin.sync_tasks`：seq 221 / idx 219。
3. **postmaster 沒有重啟，序列卻倒退了**
   - `pg_postmaster_start_time()` = 2026-09-07 16:02:17，`RestartCount=0`。
   - `cron_logs_log_id_seq.last_value = 53462`，但 `max(log_id) = 56106`（寫入時間 10-12）。10-15 20:18 又從 53377 開始發號。
   - Postgres 不重啟的話，序列只有一種情況會倒退：buffer 被逐出後重新從磁碟讀進來，讀到的是舊資料（也就是**寫入遺失**）。
4. 已排除其他人共用資料目錄：沒有其他容器或行程掛載 `data/postgres`。claude-code-hub 的 postgres 用的是另一個目錄。
5. 設定：`fsync=on`、`full_page_writes=on`、**`data_checksums=off`**，所以 Postgres 本身偵測不到這類損壞。

### 根因（推論）

Postgres 的資料目錄是 bind mount：`/Volumes/Expansion/docker/rust-bangumi/data/postgres`。這是一顆 **USB 外接的 APFS 硬碟**，再經過 OrbStack 的檔案共享掛進 Linux VM。這條路徑不保證 fsync 或寫入順序的語意，出現了典型的「寫入遺失／讀到舊頁」現象。

無法從現有 log 指出是哪一次事件造成的，因為容器 log 只保留到 09-07。但第 3 點證明問題在 Postgres 沒重啟的情況下也會發生。

### 影響

- 透過索引查詢會漏掉資料，例如 `WHERE download_id=505` 查不到東西。UPDATE、去重、衝突判斷都可能做出錯誤決定。
- 主鍵和唯一性約束失效，可能寫入重複資料。
- 無法解釋的狀態變化（例如 E9 的 515）很可能都來自這裡。

### 修正方向（環境）

執行腳本：`scripts/2026-10-16-postgres-migrate-to-volume.sh`（步驟、驗證、回滾都在腳本內；計畫見 `tasks/todo.md` 階段 0）。

1. 停止整個 stack，用 `pg_dump` 匯出兩個資料庫（pg_dump 用 seq scan，資料完整）。
2. 處理重複的主鍵：`sync_tasks` 重新編號；`cron_logs` 可以直接清空，它只是 log。
3. **把 Postgres 資料搬到 OrbStack 的 named volume**（VM 內的 ext4），不要再放在外接硬碟的 bind mount 上。用 `initdb --data-checksums`（`POSTGRES_INITDB_ARGS`）建立新 cluster，再 restore。
4. restore 之後把所有序列 `setval` 到 `max(id)`，再做一次下方的檢查。
5. 備份改用定期 `pg_dump` 寫到外接硬碟，不要拿 bind mount 上的資料目錄當備份。

### 如何檢查（正常時應該沒有任何輸出列）

```sql
-- 每個主鍵：seq scan 筆數 vs index 筆數
create temp table r(tbl text, seq bigint, idx bigint);
do $$ declare t record; s bigint; i bigint; begin
for t in select c.relname tbl, a.attname col from pg_index x join pg_class c on c.oid=x.indrelid
  join pg_attribute a on a.attrelid=c.oid and a.attnum=x.indkey[0]
  join pg_namespace n on n.oid=c.relnamespace where x.indisprimary and n.nspname='public' loop
 set local enable_indexscan=off; set local enable_bitmapscan=off; set local enable_indexonlyscan=off;
 execute format('select count(*) from %I', t.tbl) into s;
 set local enable_seqscan=off; set local enable_indexscan=on; set local enable_indexonlyscan=on; set local enable_bitmapscan=on;
 execute format('select count(*) from %I where %I is not null', t.tbl, t.col) into i;
 set local enable_seqscan=on;
 insert into r values (t.tbl, s, i);
end loop; end $$;
select * from r where seq<>idx;
-- 序列落後於 max(id)
select 'cron_logs', last_value, (select max(log_id) from cron_logs) from cron_logs_log_id_seq
  where last_value < (select max(log_id) from cron_logs);
```

---

## E2：core 與 downloader 都沒有掛載 /downloads（部署設定）

### 證據

- 容器掛載：只有 `bangumi-qbittorrent` 和 `bangumi-viewer-jellyfin` 掛了 `/downloads`；`bangumi-core` 和 `bangumi-downloader-qbittorrent` **沒有任何 mount**。`docker-compose.yaml` 在 git 歷史裡也從來沒有為這兩個服務掛過。
- downloader 回報完成狀態時，用 `collect_files_recursive(content_path)` 掃自己的檔案系統（`qbittorrent_client.rs:325`），結果永遠是空的，所以 qB 狀態 API 回傳的 `files` 一律是 `[]`。
- DB 裡**所有** download 的 `video_file` 都是 NULL（每種狀態都 100%）。
- core 的 `retry_batch_unmatched` 也掃自己的檔案系統，log 裡出現 `retry_batch_unmatched: no files found in /downloads/[LoliHouse] Tenbin [01-12]…`。
- 147 筆 synced 剛好對應 147 個 hash，**從來沒有任何合集同步成功過**。

### 影響

- **42 筆 `batch_failed`（6 個合集）的根因是這一項**。之前推測的「合集解析器把作品拆成兩部」是另一個問題，不是 batch_failed 的原因。
- 單檔 torrent 能同步，是因為 `video_path` 會退回使用 `file_path`（`sync_service.rs:189`），而那剛好就是檔案本身。
- **421（魔法科劇場版）**：content_path 是資料夾，`video_path` 退回成資料夾路徑。viewer 的 `organize_episode` 對資料夾 rename 失敗，改用 copy 時得到 `the source path is neither a regular file…`。只要 E2 修好，`video_file` 就會指向資料夾裡的影片，viewer 不需要另外支援資料夾。要注意的是，目前「挑第一個影片檔」的邏輯遇到含特典或選單的 BD 資料夾可能會選錯，需要改成挑最大的影片檔。

### 修正方向

- 程式／部署：在 compose 中為 `downloader-qbittorrent` 和 `core-service` 掛上 `${DOWNLOADS_DIR}:/downloads:ro`。
- 環境：重新部署後，把 6 個合集的 42 筆 `batch_failed` 改回 `batch_unmatched` 並把 `sync_retry_count` 歸零，讓排程重新配對。421 改回 `completed` 並重新取得 `video_file`。

### 如何檢查

```sql
-- 新完成的 download 應該要有 video_file
select status, count(*) filter (where video_file is null) as null_video, count(*)
from downloads where updated_at > now() - interval '7 days' group by 1;
```
在 core log 裡不應該再出現 `retry_batch_unmatched: no files found`。

---

## E3：`downloader_error` 永遠不會恢復（程式）

**對象**：563–566（藥屋 S3 綠茶 49/50），qB 裡已經完成。

### 時間軸

- 2026-10-15 12:49:09：AI 規則確認後的 rerun 派送 4 筆，qB 加入成功。
- 12:49:42：輪詢 `GET /downloads?hashes=…` 時發生 `error sending request`（同一時間 downloader 正在連續加入 torrent），單次失敗就把**所有** `downloading`（6 筆）標成 `downloader_error`。
- 13:28:23：`check_recovery` 執行，log 顯示 `recovered, updating 6 error records`。555 和 561 回到 `downloading`，但 563–566 的 qB 狀態是 completed，交給 `apply_completed_files` 處理。

### 根因（兩個 bug 疊加）

1. `apply_completed_files` 只處理 `status='downloading'` 的資料列（`download_scheduler.rs:520`），對 `downloader_error` 不做任何事，所以已經完成的下載**永遠停在 downloader_error**。
2. `check_recovery` 只在該 downloader **完全沒有 `downloading`** 時才會執行（`download_scheduler.rs:101`）。只要有一筆卡在 downloading（例如 E4 的 555），恢復機制就不會跑。

另外，一次網路失敗就把全部標成錯誤，太敏感了。

### 修正方向

- 程式：`apply_completed_files` 同時處理 `downloading` 和 `downloader_error`；每次輪詢都要執行恢復檢查；連續失敗 N 次才標成 `downloader_error`。
- 環境：程式修好後會自動恢復。563–566 還有衝突（見 E5），要等衝突解決後才會同步。
- 2026-10-16 第 7 項：已實作（輪詢與恢復合併、連續 3 次失敗才標錯）。部署第 6 項時又重現一次：core 比 downloader 早 5 秒起來，4 筆被標成 downloader_error。

### 如何檢查

```sql
select download_id, torrent_hash, updated_at from downloads where status='downloader_error';
```
拿這些 hash 去問 downloader，不應該出現 qB 已經 completed 的情況。

---

## E4：`downloading` 但 qB 回報 `not_found`（程式）

**對象**：555（尼古喵喵 10，hash bae8933b）。

### 時間軸

- 2026-09-29 06:11：第一次派送時 qB session 過期，重新登入失敗，加入失敗（403），所以 553 是 failed。
- 06:48:01：重新派送（555），downloader 記錄 `Torrent added successfully: bae8933b`。
- 從那之後 qB 裡**從來沒有**這個 torrent。.torrent 檔案本身沒有問題（今天下載回來，infohash 確實是 bae8933b）。

### 根因

- downloader 判斷加入成功的方式只看 `torrents/add` 回傳 HTTP 2xx，hash 則是從 URL 檔名推出來的（`qbittorrent_client.rs:211`）。qB 下載 URL 型的 torrent 是**非同步**的，下載失敗時 API 一樣回 `Ok.`。當下 qB 為什麼沒有成功抓到檔案，要看 qB 自己的 log 才能確認（含憑證，我沒有讀）。
- core 輪詢時遇到 `not_found` 直接 `continue`（`download_scheduler.rs:129`），所以**永遠不會**轉成 failed 或重試。這筆資料又讓 E3 的恢復機制無法執行。

### 修正方向

- 程式：downloader 加入 torrent 後，輪詢 `torrents/info?hashes=` 幾秒，確認真的出現再回報成功。core 遇到 `not_found` 持續超過 N 分鐘時，改成 `failed`，讓既有的重試機制接手。
- 環境：把 555 改成 `failed`，觸發重新派送。

### 如何檢查

拿所有 `downloading` 的 hash 去問 downloader，不應該有 `not_found`。

---

## E5：同集多版本都被下載，之後永遠不同步（程式 + 未處理的衝突）

**對象**：559/560（青之箱 S2 01 簡／繁）、576/579/581/582（豌豆 S3 01/02 繁／簡）、563–566（綠茶 S3 49/50 簡／繁）。

### 時間軸（以青之箱為例）

- 2026-10-09 16:01:31：建立 link 1685/1687。
- 16:01:32.08：`pending_ai_results` 的 rerun **直接派送**，兩個版本都下載了。
- 16:01:32.88：之後才執行衝突偵測，建立 conflict 1581，並把 `conflict_flag=true`。
- 從此 `trigger_sync_for_completed` 會略過 `conflict_flag=true` 的下載（`download_scheduler.rs:289`），所以永遠停在 `completed`。

### 根因

有兩條派送路徑**沒有先做衝突偵測**：

- `handlers/pending_ai_results.rs:471`（AI 規則確認後的 rerun）
- `handlers/raw_items.rs:347`（單筆 reparse）

其他路徑（fetcher_results、reparse service、filters）都是先偵測衝突再派送。

另外，同一個字幕組同一集的簡體和繁體會被判定為衝突，**需要使用者在 UI 上選一個**。目前有 6 筆衝突還沒處理（1581、1583、1589、1591、1609、1628）。

### 修正方向

- 程式：把「先偵測衝突再派送」收斂到 `dispatch_new_links` 裡，或抽成共用函式，避免之後新增的路徑再漏掉。
- 環境：使用者解決這 6 筆衝突。選中的版本會被同步；沒選中的版本已經下載完成，要從 qB 移除並刪除檔案。
- 長期：用過濾器設定繁體或簡體偏好，可以從源頭避免這類衝突。

### 如何檢查

```sql
-- 已經完成下載卻卡在衝突的
select d.download_id, d.status from downloads d join anime_links l using(link_id)
where l.conflict_flag and d.status in ('completed','downloading');
select count(*) from anime_link_conflicts where resolution_status='unresolved';
```

---

## E6：同步成功卻被記成 `sync_failed`

**對象**：506（LoliHouse 轉校生 09）、508（Mujikaku 09 ABEMA）。

### 證據（508）

- `viewer_jellyfin.sync_tasks`：task 142 / download 508 在 2026-09-01 14:55:24 已經 **completed**，target 是 `…Mujikaku Seijo - S01E09.mkv`，媒體庫裡也確實有這個檔案。
- 30 秒後（14:55:54），core 又對 508 觸發同步。來源檔已經被搬走，結果是 `Source file does not exist`，三次後變成 `sync_failed`。
- 這筆 task 142 正是 E1 裡重複的主鍵，表示 viewer DB 在這段時間已經損壞。

### 根因

- core 沒有記下 508 的 `synced` callback：可能 callback 沒有送到，或者 UPDATE 因為索引損壞而沒有更新到資料列（E1）。這部分無法從 log 證實，因為 09-07 以前的 log 已經不在了。
- viewer 的同步**不具冪等性**：來源檔不存在、但目標檔已經存在時，應該視為成功，結果卻回報失敗。
- 506（Tenbin 09）也一樣：媒體庫已經有 `てんびん - S01E09.mkv`，mtime 是 09-01 17:48 +08，也就是 09:48 UTC，和 506 的建立時間一致。但 viewer 的 sync_tasks 141 把這次成功記在 **download 507** 名下；core 的 507 其實是暴走千金 09，並在 09:51 同步到另一個路徑。viewer 的資料列和實際同步內容對不上，也符合 E1 的資料錯亂。

### 2026-10-16 實際重現（589、598）：根因確認

第 6 項部署後重跑 43 筆合集同步，589（ブチ切れ令嬢 04）和 598（てんびん 08）以相同方式落入 `sync_failed`，log 完整，可以確認機制（E1 不是必要條件）：

1. core `notify_viewer` 的 HTTP 逾時是 10 秒。viewer 同時在搬多個大檔（外接硬碟，跨裝置 copy），`/sync` 的 202 回應超過 10 秒才回來，有時連請求都還沒進到 handler。
2. core 把逾時當成「沒送出」：狀態維持 `completed`，下一輪輪詢又送一次。589 在 12:39:51～12:53:50 間被送了 12 次（其中 9 次逾時），viewer 端同一筆 download 同時有多個處理中的請求。
3. 第一個請求搬檔成功並回報 `synced`（12:47:04）；同一毫秒 core 的輪詢迴圈又拿舊的清單送了一次（`completed` 清單在 callback 前就讀出來了）。
4. 後到的請求找不到來源檔（已被搬走）→ 回報 `failed`。`update_sync_status` 處理 `failed` 時**不檢查目前狀態**，把 `synced` 蓋回 `completed`，三次後變成 `sync_failed`。
5. viewer 202 變慢的原因（推論，待驗證）：viewer 容器限制 1 CPU，tokio 只有 1 個 worker；`process_sync` 與 handler 裡有同步阻塞呼叫（r2d2 `db.get()`、`Path::exists()` 等 stat），外接硬碟在大量 copy 時 stat 也會卡，唯一的 worker 被阻塞，整個 HTTP server 就停住。

508 在 09-01 的時間軸（成功 30 秒後又被觸發）和這次一致，應該是同一個機制，E1 只是讓 viewer 的紀錄更亂。

### 修正方向

- core：callback 的 `failed` 不得覆蓋 `synced`（只在 `syncing` 時接受結果）；送出前就把狀態改成 `syncing`，逾時不回退成 `completed`，交給既有的 `recover_stale_syncing`（5 分鐘）處理；輪詢迴圈送出前重新確認狀態。
- viewer：同一 download 已有處理中的 task 時直接回 202 不重做；阻塞呼叫移到 `spawn_blocking`（或改用 `tokio::fs::try_exists`）。
- 程式：viewer 在 `organize_episode` 中，若來源不存在但目標已存在，回報 `synced`。
- 環境：506 和 508 的 DB 狀態直接改成 `synced`，`file_path` 填媒體庫路徑，再從 qB 移除 torrent。

---

## E7：/downloads 裡的孤兒檔案

| 檔案 | 時間 | 來源 |
|------|------|------|
| Mujikaku Seijo 02/03（CR） | 2026-07-15 | download 424/426：link 被過濾後 **cancelled** |
| Sakurato Oshi no Ko S3 05–07（5 個） | 2026-03-04 | DB 已經沒有相關紀錄（訂閱 2/3/5 等已被刪除） |
| GreenTea Oshi no Ko 27–30（4 個） | 2026-03-13 | 同上 |

全部都是**沒下載完的稀疏檔**，外觀大小 0.3–1.2 GB，實際佔用很小。

### 根因

- 取消下載（`cancel_torrents`）和刪除訂閱時的 purge，都用 `deleteFiles=false` 呼叫 qB（`qbittorrent_client.rs:264`）。torrent 移除了，已經下載的一部分檔案卻永遠留著。
- purge 會把取消請求送給「優先度最高的 downloader」，不一定是當初下載這個 torrent 的 downloader（`subscriptions.rs` purge step 4）。

### 修正方向

- 程式：取消時，還沒完成的下載用 `deleteFiles=true`，因為不會有其他人需要這些殘檔。purge 要依照每筆 download 的 `module_id` 送到正確的 downloader。
- 環境：刪除上面 11 個檔案。

---

## E8：qB 保留已同步的 torrent（第 1 項已修）

- 現況：94 個 `synced` 在 qB 是 error/missingFiles，另外 28 個 `synced` 在 qB 顯示 completed，但檔案已經不在 /downloads（qB 還沒重新檢查）。
- 根因：同步後沒有移除 torrent；viewer 用 rename 把檔案搬走。
- 環境：部署第 1 項之後，用 `DELETE /downloads/{hash}?delete_files=false` 移除這 122 個 torrent。

---

## E9：無法解釋的狀態（列入觀察）

- **515**（暴走千金 10，link 1618）：2026-09-08 派送時遇到 403 而失敗，沒有 hash、沒有 file_path。2026-10-15 12:57:23 被 `recover_stale_syncing` 從 `syncing` 改回 `completed`。但程式裡唯一會設成 `syncing` 的是 `notify_viewer`，而它只會處理 `file_path` 非 NULL 的資料。log 裡也沒有對 515 發出同步的紀錄。目前最可能的解釋是 E1 造成的資料錯亂。媒體庫裡已經有 S01E10。
  - 環境：515 改成 `cancelled`，因為這一集已經存在。

---

## E10：qB 沒有對外開放的 port，NAT 後面的 peer 全都連不上（網路環境，2026-10-17）

**對象**：621–624（Mujikaku Seijo 10/11/12 ABEMA、Romelia Senki 02），在 qB 裡卡在 0% 超過一天。

### 證據

- qB 的 Trackers 分頁：5 個 tracker 全部「正在運作」，回報 Romelia 02 有 9～18 個種子。所以種子確實存在，tracker 也正常。
- 一般分頁：跑了 1 天 9 小時，只下載 40.8 KiB，而且全部被丟棄；連線數 8；種子顯示 1（總共 18）。做種的 torrent 同樣連不上下載者，上傳一直是 0。
- 下載者分頁：只有 1 個 peer，旗標 `H X P`（從 DHT/PeX 得知、µTP），**沒有 `I`（incoming）**，整個 qB 看不到任何從外部連進來的 peer。
- 自行向 tracker announce 取得 12 個 peer（大多是中國的住宅 IP），從生產主機和 qB 容器做 TCP 連線，結果完全一樣：**11 個失敗、1 個成功**。所以容器網路沒問題，是對方在 NAT 後面、不接受連入。
- 對外 IP 是 114.32.227.135（HiNet 固定制，不是 CGNAT）。從 LAN 連 `192.168.68.101:16881` 成功，表示 OrbStack 的 port 發布正常；連 `114.32.227.135:16881` 則是 **Connection refused**，路由器（192.168.68.1）沒有轉發這個 port。
- qB 設定 `Connection\UPnP=false`；就算打開，qB 在 docker bridge（172.25.0.20）裡也沒辦法透過 UPnP 向路由器要 port。
- Mac 有兩張網卡在同一個網段：en0 是 192.168.68.101，en1 是 192.168.68.2，兩者的預設路由都指向 192.168.68.1。

### 根因

兩端都在 NAT 後面時無法建立連線。我們只能連得到「能接受連入」的 peer（例如 seedbox）。熱門新番的 swarm 裡常有 seedbox，所以過去能正常下載，累計上傳 18.62 GiB 也都是靠主動連出去。但舊集數或冷門的 swarm 裡，剩下的種子多半是住宅網路的使用者，就永遠連不上。

### 修正方向（環境，需要使用者在路由器上操作）

- 在路由器設定 port 轉發：**TCP 和 UDP 的 16881 → 192.168.68.101**，並在 DHCP 幫 192.168.68.101 保留位址。
- 設好之後檢查：從 LAN 連 `114.32.227.135:16881` 不應該再是 refused（前提是路由器支援 NAT loopback），或用外部的 port 檢查工具確認；qB 的下載者分頁應該開始出現 `I` 旗標的 peer。
- 程式：core 看不到「卡住」的狀態，因為 downloader 把 qB 的 `stalledDL` 和 `queuedDL` 都回報成 `downloading`。之後可以在 E4 的 `not_found` 逾時處理一併考慮：`downloading` 而且進度長時間沒變時要提醒。

### 如何檢查

- qB WebUI 的下載者分頁：有沒有 `I` 旗標的 peer。
- `downloads` 裡 `downloading` 超過 24 小時、`progress=0` 的筆數，應該是 0。

---

## 其他已知但這次沒有深入的項目

- Jellyfin trickplay 一直失敗：jellyfin 容器以 `:ro` 掛載 `/media/jellyfin`，而 Jellyfin 設定把 trickplay 存在媒體旁邊。可以把 trickplay 改存到 cache，或改成 rw 掛載。
- 生產環境的 `docker-compose.override.yaml` 有本地修改（jellyfin 的 config 和 cache 改用 named bind volume），`git pull` 前要先處理。
- 生產環境的 core log 只保留 50MB×5，大約 40 天。

## 日常觀察清單

1. E1 的索引一致性檢查：每週一次，或主機、OrbStack 重啟後執行。
2. `downloads` 依狀態分組：`downloader_error`、`completed` 超過 1 小時、`batch_failed`、`sync_failed` 都應該是 0。
3. `downloading` 的 hash 在 qB 裡都要找得到；`downloading` 超過 24 小時、`progress=0` 的要查（E10）。
4. 未解決的衝突數。
5. /downloads 裡不應該有超過 7 天、又不屬於任何 `downloading` 的檔案。
