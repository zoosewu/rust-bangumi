# 生產環境問題修復 (2026-10-16)

分析來源：2026-10-16 對 yeh3f@192.168.68.101 的唯讀檢查。
環境稽核（根因、證據、觀察用 SQL）：`docs/runbooks/2026-10-16-production-environment-audit.md`（下稱「稽核」，E1–E9）。
每次實作只解決一個問題。

## 階段 0：環境基礎（最優先，其餘修正都建立在健康的 DB 上）

- [x] repo 補上生產 2026-09-07 的本地改動：jellyfin config/cache 改用 bind 型 named volume（`docker-compose.override.yaml`）
- [x] repo：postgres 改用 Docker named volume `postgres_data` + `POSTGRES_INITDB_ARGS=--data-checksums`；`.env.prod` 移除 `POSTGRES_DATA_DIR`
- [x] 修復腳本 `scripts/2026-10-16-postgres-migrate-to-volume.sh`（preflight → dump → switch → restore → verify → start，含 rollback）
  - 已在生產以唯讀方式驗證：去重 SQL（暫存表 + rollback）、筆數統計、驗證 SQL（能抓出全部 4 類已知問題）
- [x] commit + push（b16b348、6ef2cf9、d14edc9：修正 bash 3.2 把全形字元當變數名的問題）
- [x] 生產執行（2026-10-16 09:21–10:00 +08，使用者操作）
  1. 生產 `git checkout -- docker-compose.override.yaml && git pull` → d14edc9（原 override 備份於主機 `/tmp/override.prod-backup.yaml`）
  2. `preflight` `dump` `switch` `restore` `verify` 全 PASS：筆數與匯出一致；索引 / 重複 / 序列 OK；
     data_checksums=on；PK 21、FK 21、UNIQUE 11、CHECK 6 全部重建成功；505 可經 PK 與 hash 索引查到；
     `cron_logs` 1165 筆、`sync_tasks` 2 筆（508→220、510→221）重新編號
  3. `start`：全部服務 healthy，postgres 無錯誤
- [ ] 2026-10-23 前後：確認無異常後刪除舊目錄 `data/postgres`、根目錄 `postgres-data/`、主機 `/tmp/override.prod-backup.yaml`；
  備份 `data/backups/2026-10-16-pg/` 保留
- [ ] 後續：定期 `pg_dump` 到外接硬碟作為備份（取代以 bind mount 資料目錄當備份）

## 階段 1：程式修正（依序，一次一項）

- [x] 1. 同步完成後從 qB 移除 torrent（稽核 E8）
  - sync callback `synced` 後，背景檢查同 hash 的所有 downloads；沒有 pending/downloading/downloader_error 時呼叫 downloader `DELETE /downloads/:hash`（delete_files=false）
  - 決策邏輯抽成純函式並寫單元測試
- [x] 6. core 與 downloader-qbittorrent 掛載 `${DOWNLOADS_DIR}:/downloads:ro`（稽核 E2：video_file 全為 NULL、合集全部 batch_failed、資料夾型 torrent 同步失敗）
  - 挑影片改為「最大的影片檔」而非第一個（BD 資料夾含特典 / 選單）
  - 實作：`docker-compose.yaml` 兩個服務加唯讀掛載；`shared::pick_main_video`（純函式，大小由呼叫端注入）+ 4 個單元測試；
    core `extract_media_files` 改用它（`fs::metadata` 取大小）
  - 驗證：`cargo test -p shared -p core-service` 全過、clippy 對改動無警告；compose 展開確認兩服務都掛 /downloads:ro；
    以生產 7 個資料夾的實際檔名跑 `match_batch_files`：6 個合集 42 集全部配對成功，劇場版 17 個影片中選中正片
  - 部署後資料修復：`scripts/2026-10-16-requeue-file-matching.sql`（唯讀計數確認命中 43 列）
  - 未處理：dev 環境的服務以 cargo 在 host 執行，qB 回報的 /downloads 路徑在 host 上不存在，同樣拿不到檔案清單（另案）
  - [x] 部署後確認（2026-10-16 12:38–12:54 UTC）：downloader 回報實際檔案；43 筆中 41 筆 synced（含 421 劇場版選中正片）；
    合集 torrent 在全部同步後自動移除（log 有 `Removed torrent … after sync`，第 1 項也一併確認）。
    另外 2 筆（589、598）重現 E6 而落入 sync_failed，檔案實際已在媒體庫 → `scripts/2026-10-16-mark-synced-589-598.sql`
    （已執行，UPDATE 2）。最終：43 筆全部 synced，7 個 torrent 在 qB 皆為 not_found
  - 部署時 core 比 downloader 早 5 秒起來，觸發 E3（4 筆 downloader_error），註冊後自動恢復；教訓見 lessons.md
- [x] 7. downloader_error 恢復（稽核 E3）
  - `apply_completed_files` 同時處理 `downloading` 與 `downloader_error`
  - 每次輪詢都做恢復檢查，不再只在「沒有 downloading」時才做
  - 連續失敗 N 次才標 `downloader_error`，單次網路錯誤不全標
  - 實作：`poll_downloader` 與 `check_recovery` 合併為一條路徑，每輪都查 `POLLED_STATUSES`（downloading + downloader_error）；
    狀態對應抽成純函式 `poll_outcome`（completed / error / downloading·paused·checking / 其他不動）；
    依 hash 更新時只動 POLLED_STATUSES 的列（原本會連同 hash 相同的舊 synced 列一起改）；
    完成或恢復時清掉 `Downloader offline` 的 error_message；每個 downloader 連續 3 次輪詢失敗（約 3 分鐘）才標 downloader_error
  - 驗證：core 161 個單元測試全過（新增 4 個），clippy 無新警告。DB 更新路徑沒有自動化測試，部署後觀察
  - `not_found` 仍維持不動，交給第 8 項
  - [ ] 部署後確認：重啟 downloader 不再出現 `marking … downloader_error`，只會看到 `Polling downloader … failed (1/3)`
- [x] 8. qB 回報 `not_found` 的處理（稽核 E4）+ 停滯偵測（稽核 E10 的程式缺口）
  - downloader 加入 URL 型 torrent 後輪詢 `torrents/info` 確認真的出現才回報成功
  - core：`downloading` 持續 `not_found` 超過 N 分鐘 → `failed`，交給既有重試
  - 實作（downloader）：`add_torrents` 全部送出後，每 0.5 秒查 `torrents/info`，最多 15 秒；沒出現的改回報 `failed`
    （reason：`qBittorrent did not register the torrent within 15s`），core 的派送會照既有邏輯 cascade 到下一個 downloader 或記成 failed。
    純函式 `reject_unregistered` + 2 個單元測試；mock qB 整合測試新增「qB 回 Ok. 但 torrent 沒出現」案例
  - 實作（core）：`not_found` 改成 `PollOutcome::Missing`，同一個 (downloader, hash) 連續回報 10 分鐘才改 `failed`
    （記憶體計時，core 重啟會重新計時）；下載有進度或從 downloader_error 恢復時才更新 `updated_at`，
    所以 `downloading` 的 updated_at 就是「最後有進度的時間」；超過 24 小時沒進度 → error_message 標
    `No download progress for 24 hours (no reachable peers?)`，狀態維持 downloading，進度一動就清掉
  - 驗證：downloader 28 + 17 個測試、core 162 個測試全過；clippy 只有既有的 `map_torrent_state` 警告
  - 限制：`failed` 目前只在 downloader 重新註冊或手動重試時才會重派（沒有定時重試）
  - 2026-10-16 17:37 UTC 部署（downloader 先、core 後）：core 沒有出現 downloader_error 或輪詢失敗；
    第一輪輪詢只更新有進度的 622/623，621（停在 99.7%）的 updated_at 沒變 → `is_distinct_from` 條件在 PG 上如預期
  - [ ] 待下一次派送確認：log 出現 `Torrent submitted…`，且沒有 `never appeared`
- [x] 9. 派送前一律先偵測衝突（稽核 E5）
  - `pending_ai_results.rs:471`（rerun）與 `raw_items.rs:347`（單筆 reparse）漏了；收斂到共用函式 / `dispatch_new_links` 內
  - 實作：新增 `DownloadDispatchService::detect_conflicts_and_dispatch`（偵測 → 合併自動解除衝突的 link → 派送；
    偵測失敗就不派送，因為衝突標記不可信）。建立／重新啟用 link 的 8 個路徑全部改走它
    （fetcher_results ×2、links、filters ×2、pending_ai_results filter confirm、reparse service、單筆 reparse）
  - `rerun_unmatched_raw_items` 改成回傳新 link id、不自己派送；兩個呼叫端（parser confirm、
    subscription step 3）原本就在之後跑衝突偵測，現在改成偵測後才派送
  - 不經過它的只剩：重派既有 link（retry_*、manual_retry，衝突已偵測過）與使用者解衝突後派送選中的 link
  - 一併修正 `retry_completed_downloads`：`notify_viewer` 改回傳 `SyncNotice { Sent, Skipped, NoViewer }`，
    因衝突被略過（Skipped）不再被當成「沒有 viewer」而中斷整個重試迴圈
  - 驗證：cargo test 全過、clippy 警告數不變（131）
  - [ ] 部署後觀察：下一次 parser 確認 / 單筆 reparse 產生同集多版本時，log 先出現 `Conflict detected` 且不派送
- [x] 10. 同步不重複、結果不倒退（稽核 E6，2026-10-16 以 589/598 重現確認根因）
  - core：`failed` callback 不得覆蓋 `synced`（只在 `syncing` 時接受）；送出前先標 `syncing`，逾時不回退，交給 `recover_stale_syncing`
  - core：輪詢迴圈送出前重新確認狀態（避免拿 callback 前讀出的舊清單重送）
  - viewer：同一 download 已有 processing task 時不重做；來源不存在但目標已存在 → 回報 `synced`
  - viewer：handler / `process_sync` 的阻塞呼叫（r2d2、`Path::exists`）移出 async worker（容器 1 CPU = 1 個 worker）
  - 實作（core）：`notify_viewer` 送出前用條件更新 `completed → syncing` 認領，認領失敗（狀態已變）就略過，
    順帶解決輪詢用舊清單重送；逾時保留 `syncing` 交給 `recover_stale_syncing`，只有連線被拒或 viewer 回非 2xx 才退回 `completed`；
    `failed` callback 只在 `syncing`/`resyncing` 時接受，更新也帶狀態條件
  - 實作（viewer）：記憶體內 `InFlightSyncs`，同一 download 處理中時重複請求直接回 202；`/sync` handler 不再碰 DB，
    sync_tasks 的寫入改用 `spawn_blocking`；`organize_episode` 來源不在但目標已在 → 回報成功；
    file_organizer 的 `exists()` 全改 `tokio::fs::try_exists`
  - 驗證：cargo test 全過（含 2 個新的 tempdir 測試：重複同步回報同一檔案、來源與目標都不在仍失敗）；clippy 警告數不變
  - 未處理：`/resync`、`/delete` handler 仍在 async worker 上同步呼叫 `db.get()` / `exists()`（使用頻率低，之後再改）
  - [ ] 部署後觀察：大量同步時 core log 不再出現 `Failed to notify viewer` 逾時後重送；viewer log 若出現
    `already in progress, ignoring duplicate` 表示去重有作用；不再有新的 `sync_failed`
- [ ] 12. webhook：一次觸發大量通知時 Discord 回 429，訊息遺失不重試 → 依 `retry_after` 重試或排隊送出
- [ ] 13. webhook URL（含 token）以明文寫進 core log（`webhook_service` 的 `Webhook N fired to …`）→ 只記 webhook id；
  已外洩到 log 的 token 建議在 Discord 重新產生
- [ ] 11. 取消時清除殘檔（稽核 E7）
  - 未完成的下載取消時 `deleteFiles=true`
  - 訂閱 purge 依各 download 的 `module_id` 送到正確的 downloader
- [ ] 2. AI 生成改為全域排隊（一次一個）+ 限流自動重試
- [ ] 3. 待確認（pending_result_id 非空）的 filter rule 一律不套用
- [ ] 4. 解析器綁定建立它的訂閱（parse 時只用 global + 本訂閱的 parser）
- [ ] 5. 依 `docs/superpowers/plans/2026-07-16-metadata-identity.md` 實作作品身分 / metadata

## 階段 2：資料修復（對應的程式修正部署後才做；全部在 DB 修復完成後）

| 對象 | 前提 | 動作 |
|------|------|------|
| ~~qB 已同步 torrent~~ | — | 2026-10-16 完成：項目 1 部署（core 以 93d073e 重建）後移除 132 個（94 missingFiles + 38 completed 但檔案已搬走），失敗 0，qB 查詢全部 not_found |
| ~~42 筆 `batch_failed`（6 個合集）+ 421 魔法科劇場版~~ | 2026-10-16 完成 | 執行 `scripts/2026-10-16-requeue-file-matching.sql`：改回 `downloading`，讓輪詢以實際檔案清單重跑完成流程。注意：てんびん、暴走千金的集數媒體庫已有（來自單集下載），同步時會以同檔名覆蓋 |
| 563–566（downloader_error，qB 已完成） | — | 重啟時被重試機制刪除，link 因衝突未重派，目前無 download 紀錄；qB 裡的 torrent 與檔案還在。解衝突後選中的 link 會重派，qB 已有該 torrent → 直接完成並同步 |
| ~~555（downloading，qB not_found）~~ | — | 已自動處理：重啟時重派為 609，qB 這次成功加入 |
| 559/560、576/579/581/582、563–566 | 使用者 | UI 上解決 6 筆衝突（1581/1583/1589/1591/1609/1628）；未選中且已下載的版本從 qB 移除並刪檔 |
| 506、508（sync_failed，實際已同步） | 無 | 改 `synced`、`file_path` 填媒體庫路徑，從 qB 移除 torrent |
| ~~589、598（sync_failed，實際已同步，2026-10-16 重現 E6）~~ | — | 2026-10-16 完成：`scripts/2026-10-16-mark-synced-589-598.sql`，UPDATE 2 |
| 621–624（qB 0% 超過一天，稽核 E10） | 2026-10-16 已設定轉發（624 已 synced，其餘下載中） | 路由器設定 TCP/UDP 16881 → 192.168.68.101 的轉發（加上 DHCP 保留），之後看下載者分頁有沒有 `I` 旗標、進度有沒有開始動 |
| 515（無 hash 的 completed） | 無 | 改 `cancelled`（S01E10 已在媒體庫） |
| /downloads 孤兒檔 11 個（Mujikaku CR 02/03、推しの子 9 個） | 無 | 刪除 |
| Jellyfin trickplay 唯讀失敗 | 使用者決定 | trickplay 改存 cache，或媒體庫改 rw |

### 2026-10-16 DB 修復後重啟的觀察

- downloader 重新註冊觸發「重試 failed / downloader_error」：22 筆被認領，14 筆重新派送（607–620），
  3 筆因已有進行中的下載略過，其餘因衝突略過。
  - 需要的（媒體庫缺這集）：607 LIAR GAME 24、609 尼古喵喵 10、613/620 Mujikaku 11/12、615 罗梅莉亚 02、617/618 女僕 8/4
  - 重複的（媒體庫已有這集，DB 卻沒有任何同集紀錄，推測是 E1 的寫入遺失）：608、610、611、612、614、616、619。
    同步時會以同檔名覆蓋，沒有傷害，只浪費頻寬
- 停服務的順序是 downloader 先停、core 後停，core 輪詢失敗時把 555、561 標成 downloader_error（就是 E3 的行為），
  重啟後被重試機制重派。之後的 runbook 要先停 core
- 506 / 508 / 421 被 viewer 註冊時重設重試，會再失敗 3 次後回到 `sync_failed`（等項目 6、10）
- 新發現的小 bug：`retry_completed_downloads` 把「因衝突略過」（`notify_viewer` 回 `Ok(false)`）當成「沒有 viewer」而中斷整個迴圈
  （log：`No viewer available during retry, stopping`），後面的 completed 都不會被重試 → 併入項目 9

## 觀察依據

稽核文件末的「日常觀察清單」；DB 一致性用稽核 E1「如何檢查」的 SQL（唯讀，主機或 OrbStack 重啟後必跑）。

## Review

### 1. 同步完成後移除 torrent
- `core-service/src/services/sync_service.rs`：新增 `release_torrent_if_settled()`；判斷邏輯抽成純函式
  `torrent_releasable()`，只要同 hash（同 downloader）還有 `pending` / `downloading` / `downloader_error`
  的 download 就保留 torrent（合集 torrent 由多個 download 共用）。其他狀態（completed、syncing、
  batch_unmatched 等）都只讀磁碟上的檔案，而檔案保留（`delete_files=false`），因此不受影響。
- `core-service/src/handlers/sync.rs`：viewer 回報 `synced` 後在背景呼叫，失敗只記 warn，不影響 callback。
- qBittorrent / PikPak 的 `DELETE /downloads/:hash` 介面一致，不需要改 downloader。
- 驗證：3 個新單元測試 + `cargo test -p core-service` 全過；clippy 對新程式碼無警告。
  DB 查詢 + HTTP 呼叫的路徑沒有自動化測試，部署後需看 log 有沒有 `Removed torrent … after sync`。
- 已知限制：最後一集停在 `batch_failed` / `sync_failed`（沒有 `synced` 回報）的 torrent 不會自動移除。
- 已部署（2026-10-16，core 以 93d073e 重建）；既有 torrent 已手動清除（見階段 2）。
- [ ] 待確認：下一筆新同步後，core log 出現 `Removed torrent … after sync`，且該 hash 在 qB 為 not_found。

---

# 死 tracker + 重試競態修復 (2026-07-14 第二階段)

Spec: `docs/superpowers/specs/2026-07-14-dead-tracker-retry-race-fix-design.md`
Runbook: `docs/runbooks/2026-07-14-tracker-fix-deploy.md`
(第一階段登入競態修復已完成並部署,見前一份 spec)

## 計畫

- [x] Fix A: fetcher 保留原始 .torrent URL(移除 TRACKERS / torrent_url_to_magnet)
- [x] Fix A: entry_to_raw_item 純函式重構 + RSS 樣本單元測試
- [x] Fix B: retry_failed_downloads / retry_no_downloader_links 原子化 (DELETE..RETURNING)
- [x] 資料遷移 SQL(190 筆映射自實際 RSS 收割,100% hash 對應)
- [x] SQL 端到端驗證(拋棄式 Postgres + 全部 repo 遷移 + 冪等重跑)
- [x] 部署 runbook
- [x] `cargo fmt` / `cargo clippy` / `cargo test` 全過
- [x] Review 章節總結

## 部署後驗證(runbook 第 5 節)— 全數通過 (2026-07-15 部署)

- [x] `Retried failed downloads: 23 dispatched, 0 failed again`,只出現一次
- [x] downloads 無重複 link_id(0 rows)
- [x] 種子帶 7 個 tracker(修復前僅 3 個且全死),nyaa.tracker.wf 正常、40 peers
- [x] 無重複攝入:重複 download_url = 0;192 → 204 的成長來自新訂閱與新集數
- [x] 遷移結果:magnet 192 → 0;23 筆重派下載已全數完成並同步


## Review

- `fetchers/mikanani/src/rss_parser.rs`:移除 `TRACKERS` 常數與 `torrent_url_to_magnet()`,
  download_url 直接使用 RSS enclosure 的原始 `.torrent` URL。entry→RawAnimeItem 映射
  抽為純函式 `entry_to_raw_item()`,`fetch_raw_items` 改為 `filter_map` 迭代器鏈
  (符合專案函數式原則)。順手移除該函式內既有的多餘 `DateTime::<Utc>::from` 轉換。
  舊的 6 個 magnet 轉換測試已無對應程式碼,replaced by 2 個以樣本 RSS XML 驅動的測試。
- `core-service/src/services/download_dispatch.rs`:`retry_failed_downloads()` 與
  `retry_no_downloader_links()` 的「SELECT 後 DELETE」改為單一
  `DELETE ... RETURNING link_id`,併發呼叫各得不相交集合,消除重複派送競態。
  順帶簡化:不再需要載入完整 `Download` 記錄。
- `scripts/2026-07-14-fix-magnet-urls.sql`:190 筆 hash→URL 映射(自 7 個訂閱的 RSS
  實際收割,與 DB 中 190 個 magnet hash 100% 對應)。已在拋棄式 Postgres 上套用全部
  repo 遷移後端到端驗證:magnet 歸零、source_hash 重算正確(與 Python sha256 比對一致)、
  重複記錄清除、冪等重跑全 0。
- 驗證:全 workspace `cargo test` 372 passed / 0 failed;`cargo fmt` 已套用;
  touched files 的 clippy 無新增警告(僅存 repo 既有的 `DownloaderCapability` unused import)。

## 後續事項(本次範圍外)

- `816a09f` 的 batch status migration 尚未套用到生產(潛在 bug,下次部署會自動帶上)
- fetcher pub_date 解析(mikanani torrent:pubDate 命名空間欄位,全為 NULL)
- download_scheduler 對 not_found 狀態的處理(`_ => continue` 靜默忽略)
- ~~主機對外 UDP / DHT 環境檢查~~ — 部署後已自行恢復(connected, DHT 282 節點)
