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
- [x] 2026-10-23 前後：確認無異常後刪除舊目錄 `data/postgres`、根目錄 `postgres-data/`、主機 `/tmp/override.prod-backup.yaml`；
  備份 `data/backups/2026-10-16-pg/` 保留
  - 2026-10-23 檢查：兩個 DB 的主鍵 seq/index 筆數一致（23 + 5 張表）、checksum 失敗 0、序列正常；postgres log 的 ERROR 都是手動查詢打錯；
    `/tmp/override.prod-backup.yaml` 已不存在（主機重開清掉）
  - **暫停刪除**：`data/postgres` 每分鐘被寫一次 → 發現 Docker Desktop 裡還有一整套舊的 rust-bangumi 在跑（見下方 E0）
- [ ] **E0（2026-10-23 發現，最高）主機同時跑 Docker Desktop 與 OrbStack 兩套 rust-bangumi**
  - Docker Desktop（context `desktop-linux`）：2026-07-15 建立的 bangumi-core / postgres / qbittorrent / jellyfin / viewer / fetcher /
    metadata / 兩個 downloader，`restart: always`，9/1 開機自動啟動、Up 7 週
  - 共用：postgres 的 `data/postgres`（E1 根因）、qBittorrent 的 `data/qbittorrent` 設定與 `data/downloads`、viewer 的媒體庫與下載目錄
  - 現況：Docker Desktop 的 postgres 從 10/16 起每分鐘重啟失敗（10044 次），所以那邊的 core 沒有 DB 可用；
    但它的 qBittorrent 仍在跑（7 週累計收 16.9 GB / 送 7.7 GB），可能把舊 torrent 寫回共用設定、在共用下載目錄產生檔案
  - 可能也解釋了：E1 資料損壞、無自覺聖女第 7 集反覆下載、/downloads 的孤兒檔、qB 的 missingFiles
  - Docker Desktop 也還有 dufs、etherpad、rustdesk、claude-code-hub dev、bgmi、karaoke-eternal、upsnap；OrbStack 的 etherpad 在重啟循環（疑似連接埠被佔）
  - [ ] 使用者：停止並取消自動重啟 Docker Desktop 的 bangumi-* 容器（指令見對話）
    - 2026-10-23 19:36 下的 `docker update/stop` 卡住沒有生效：Docker Desktop 的 VM 約 10/23 14:31 已整個停掉
      （找不到 VM 程序，`data/postgres/postmaster.opts` 最後寫入 14:31:37），所有容器（含 claude-code-hub 的兩個 postgres）都已不在跑；
      restart=always 沒改到，Docker Desktop 下次啟動會全部復活 → 要關掉 Docker Desktop 並取消開機啟動
    - 2026-10-24 使用者已結束 Docker Desktop（程序全數結束），`settings-store.json` 的 AutoStart 改為 false（備份 `.bak-20261024`）；
      注意：之後若再打開 Docker Desktop，裡面的 bangumi-* 與 claude-code-hub 兩個 postgres 會自動啟動，要先刪掉
  - 2026-10-24 qBittorrent 共用設定檢查（`data/qbittorrent/qBittorrent/BT_backup` 18 組）：
    - OrbStack qB 載入 4 個：630（CR 第 11 集，保留）；3 個 DB 沒有紀錄的舊種子（Super no Ura 09 error、藥屋 49/50 簡日版 completed）
    - 14 個 OrbStack qB 沒載入、續傳檔卻在 10/23 04:20–13:20 還在更新 → 是 Docker Desktop qB 在跑的已同步 / 已取消舊種子
      （無自覺聖女 07/09/10、尖帽子三個合集、暴走千金、Super no Ura、轉校生、欺詐遊戲 22、二十世紀電氣目錄、魔法科劇場版）
      → 解釋了第 7 集反覆下載與 /downloads 的孤兒檔、0 MB 合集資料夾；OrbStack qB 重啟時會載入它們重新下載
    - [x] 使用者：14 個續傳檔移到 `BT_backup.stale-20261024/`（不刪）；3 個 DB 沒紀錄的舊種子用 downloader API 連檔案移除
      - 2026-10-24 完成：三個移除都回 200、qB 查詢為 not_found；BT_backup 只剩 630；/downloads 只剩 CR 第 11 集；
        630 在舊 qB 停掉後開始有進度（0% → 0.34%）
  - [x] 停止後：確認 OrbStack qBittorrent 的 torrent 清單沒有被舊 torrent 污染，再決定 Docker Desktop 本身要不要關閉 / 取消開機啟動
    - 2026-10-24 使用者決定：Docker Desktop 上的其他服務（bgmi、karaoke-eternal 等）與 claude-code-hub 都不需要，已自行刪除資料夾；不搬移、不做檢查
  - [x] 2026-10-24 已刪 `data/postgres`、`postgres-data/`（使用者操作，備份保留）；`data/jellyfin-config`、`data/jellyfin-cache` 也只有 Docker Desktop 在用，可一併評估
- [x] 定期 `pg_dump` 到外接硬碟作為備份：compose 新增 `db-backup`（每天、bangumi + viewer_jellyfin、保留 14 天且至少 3 份、
  寫完以 pg_restore --list 驗證）；本機實測備份 / 失敗不留殘檔 / 保留規則皆正確；
  2026-10-20 14:39 部署：首次備份 bangumi 772K（23 張表資料）、viewer_jellyfin 20K（5 張表資料），皆可讀取

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
  - [x] 部署後觀察：下一次 parser 確認 / 單筆 reparse 產生同集多版本時，log 先出現 `Conflict detected` 且不派送
    - 2026-10-22 確認：10/21 抓到藥屋 51 繁日/簡日兩版，log `Conflict detected … episode_no=51`、`0 dispatched`
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
- [x] 12. webhook：一次觸發大量通知時 Discord 回 429，訊息遺失不重試 → 依 `retry_after` 重試或排隊送出
  - 實作：每個 webhook 一條佇列（`tokio::sync::Mutex`），同一 webhook 一次只送一則；429 時依 body 的 `retry_after`
    （其次 `Retry-After` header，預設 1 秒，上限 60 秒）等待後重試，最多 5 次，仍失敗記 error
  - 驗證：mock webhook 測試（429 兩次後成功 → 共 3 次；一直 429 → 停在 5 次）與 `retry_wait` 解析測試
  - [ ] 部署後觀察：下次一批同步完成時，Discord 通知數量與同步數一致；log 出現 `rate limited` 後接著 `fired`
- [x] 13. webhook URL（含 token）以明文寫進 core log（`webhook_service` 的 `Webhook N fired to …`）→ 只記 webhook id；
  已外洩到 log 的 token 建議在 Discord 重新產生
  - 實作：log 只記 webhook id；送出失敗時的 reqwest 錯誤用 `without_url()`（錯誤訊息預設會帶 URL）
  - [x] 使用者：在 Discord 重新產生 webhook URL，並在 UI 更新（2026-10-20 13:28 更新完成）
- [x] 11. 取消時清除殘檔（稽核 E7）
  - 未完成的下載取消時 `deleteFiles=true`
  - 訂閱 purge 依各 download 的 `module_id` 送到正確的 downloader
  - 實作：downloader 的 `cancel_torrents` 改成連檔案一起刪（qB `deleteFiles=true`、PikPak `delete_files=true`）；
    core 新增 `DownloadCancelService::remove_torrents`，取消與 purge 共用：依 `module_id` 分送，
    且**同一個 torrent 若還有範圍外、仍需要它的下載（狀態不是 cancelled/failed/synced，例如合集的其他集）就保留**。
    這同時修掉一個既有 bug：過濾掉合集中的一集會把整個合集 torrent 從 qB 移除
  - 取消範圍從 `downloading` 擴大到 `downloader_error`（同樣有 torrent 在 downloader 裡）
  - 沒有 `module_id` 的舊資料不再送給「優先度最高的 downloader」，直接略過
  - 驗證：新增 qB mock 測試確認 `deleteFiles=true`（先看到失敗再修）、4 個分組／保留規則單元測試；cargo test 全過、clippy 警告數不變
  - [ ] 部署後觀察：下次過濾器或 purge 觸發取消時，/downloads 不再留下殘檔；合集只過濾一集時 torrent 仍在 qB
- [x] 2. AI 生成改為全域排隊（併行數可在設定頁調整，預設 1）+ 限流自動重試
  - 根因：生產 55 筆 parser / filter 生成有 47 筆失敗，幾乎全是 Groq TPM 限流（每分鐘 8000 token，每次請求 3–4k）；
    2026-10-15 12:48–12:49 一分鐘內同時送出 16 筆。錯誤訊息裡有「Please try again in 30.9s」，但程式沒有等待重試
  - 實作：
    - `ai/queue.rs` 全域佇列（可調上限；調高立即放行，調低不打斷執行中的請求），所有 AI 呼叫在 `AiProviderChain::run` 排隊
    - 新增 `AiError::RateLimited`（rate_limit_exceeded JSON 或 HTTP 429，等待秒數取訊息或 Retry-After header）；
      所有 provider 都限流時佔著名額等 provider 指定的秒數 +1s（未指定 30s、上限 120s）後重試，最多 5 次
    - 最終仍限流時錯誤訊息以 `[rate_limit_exceeded:N]` 開頭，前端原本的倒數顯示終於對得上（以前前綴被包在 `provider unavailable: ...` 後面）
    - migration `ai_prompt_settings.max_concurrent_requests`（1–10，預設 1）；啟動時套用；設定頁新增「AI 請求併行數」下拉選單
    - `PUT /ai-prompt-settings` 改為只更新有送出的欄位：順手修掉既有 bug——儲存 parser prompt 會把 filter prompt 清成 NULL（反之亦然）
  - 驗證：佇列 5 個測試、限流重試 5 個測試（tokio 暫停時間）、設定範圍 1 個；本機以暫存 DB 跑 core：migration 套用（預設 1）、
    分開儲存併行數與 parser prompt 後 filter prompt 仍保留、改 3 立即生效、11 回 400、down/up 可重跑；前端 tsc、47 個測試通過
  - [x] 2026-10-17 23:23 (+08) 部署 core + frontend（551f369）：migration `20261017000001aimaxconcurrentrequests` 已套用，
    log `AI 請求併行數：1`，core healthy、restarts=0；經前端代理讀到 `max_concurrent_requests: 1`
  - 生產現況：`fixed_parser_prompt` 為 NULL、`fixed_filter_prompt` 為空字串（兩者都會用內建預設 prompt）、
    `custom_parser_prompt` 為 NULL、`custom_filter_prompt` 有 223 字，最後更新 2026-03-13。符合舊 bug 的痕跡，但無法確認是否有自訂內容被清掉
  - [ ] 部署後觀察：log 出現 `All AI providers rate limited, retrying after wait` 後該筆最終成功；新的 failed 不再是限流
- [x] 3. 待確認（pending_result_id 非空）的 filter rule 一律不套用
  - 根因：AI 產生的 filter 在確認前就以 `target_type='global'` 寫進 filter_rules，只靠 `pending_result_id` 標記；
    套用規則的 `collect_all_rules_for_link`（新 link 的 filtered_flag、所有 recalc）與 raw item 顯示都沒排除它 →
    確認前就對**所有**連結生效。另外該欄位 FK 是 `ON DELETE SET NULL`：pending 被刪而規則沒先刪時，會變成永久 global 規則
  - 實作：確認流程本來就從 `generated_data` 重建規則，這些資料列沒有用處 → 不再寫入；
    migration `2026-10-17-000000-drop-pending-filter-rules` 刪除既有資料列並移除 `filter_rules.pending_result_id` 欄位，
    讓待確認的規則在資料結構上無法存在。title_parsers 的 pending 機制不變（parser 的讀取原本就有排除）
  - 驗證：cargo test 全過、clippy 警告 131 → 130；本機 postgres:15 跑全部 migration，seed 一筆待確認與一筆已確認規則，
    up 後只剩已確認的，down → up 正常
  - [x] 資料（2026-10-17 生產唯讀查詢）：目前 0 筆待確認 filter rule（filter 類 pending_ai_results 只有 2 筆 failed）、
    0 筆 global 規則（只有 5 fetcher + 7 subscription）→ migration 不會刪到任何資料，也沒有變成永久的 AI 規則。
    以 SQL 依相同適用範圍與 `should_include` 語意重算 427 筆 active link：204 筆已過濾全都應過濾、223 筆未過濾全都應納入
    → **沒有被誤過濾的 link，不需修資料**
  - [x] 2026-10-17 14:51 (+08) 部署 core（94a6029）：migration `20261017000000droppendingfilterrules` 已套用，
    `filter_rules` 已無 `pending_result_id`、規則數維持 12；core healthy、restarts=0，log 無錯誤
- [x] 4. 解析器綁定建立它的訂閱（parse 時只用 global + 本訂閱的 parser）
  - 根因：parse 時載入所有已確認 parser、不看 `created_from_*`；而且待確認頁用訂閱層級確認時沒送 `target_id`，
    13 個 parser 存成 `subscription` + `created_from_id=NULL`（22 號綁到已不存在的訂閱 14）
  - [x] 程式：`TitleParser::applies_to_subscription`（綁訂閱的只解析該訂閱，其他都是全域）；`parse_title` 帶入
    raw item 的 subscription_id（fetcher 收件、單筆 / 批次 reparse、parser 確認後重跑）；parser 預覽逐筆依訂閱挑 parser；
    確認時訂閱層級沒給 target 就用 pending 的 subscription_id，都沒有則 400（filter 確認同樣適用）
  - [x] 資料：`scripts/2026-10-17-bind-parsers-to-subscriptions.sql`，依 pending 結果與實際解析紀錄對應 12 個 parser 的訂閱、
    停用沒有訂閱的 46；生產上以 ROLLBACK 試跑：更新 12 + 1 筆，兩個驗證查詢皆 0 列
  - [x] 2026-10-17 先套用資料修復 SQL（UPDATE 12、UPDATE 1，兩個驗證皆 0 列），20:46 (+08) 再部署 core（6ddc608）：
    healthy、restarts=0，log 無 ERROR / WARN；12 個 parser 已綁訂閱、46 已停用、60 仍待確認
  - [ ] 部署後觀察：新集數仍由原 parser 解析；確認新 parser 後 `title_parsers.created_from_id` 有值
- [x] 5. 作品身分 / metadata（計畫：`docs/superpowers/plans/2026-10-18-metadata-identity-v2.md`，spec 已於 2026-10-18 修訂）
  - 2026-10-18 重新實測：媒體庫 18 個資料夾 0 NFO / 0 海報；core 每次同步都送 `bangumi_id: None`；viewer 不會寫 `tvshow.nfo`；
    animes 簡介 / 日期全空；4 個訂閱被拆成兩個作品，其中 3 部的兩組 1–12 集逐位元組相同（重複約 19 GB）
  - 使用者決定（2026-10-18）：名稱中文即可、各季統一（沿 bgm 前傳鏈取第一季中文名，手動可覆寫，不用 AI）；季數由 parser 決定；
    沒有 bgm id 沿用現行做法 + log（不做待認領佇列）；重複集數保留合集版；合集集數都已下載則自動略過；殘骸作品看報表後 soft delete
  - [x] 階段 A：身分（A1–A10），commit 6901cf0…7b9bbbf 與封面修正
    - A1 `ExternalId` / payload 欄位；A2 mikanani 回報 `mikan/` `bgm/`（實測 3822→548818、4027→456080）；A3 migration
      （`anime_external_ids`、`metadata_namespaces`、作品系列身分 / 名稱來源 / soft delete、季名、`raw_anime_items.external_ids`）；
      A4 namespace 註冊落地；A5 metadata 依 subject id 查詢、前傳鏈找第一季、刪除 `list[0]`（實測藥屋 S2/S3 → 系列 420628）；
      A6 identity 服務；A7 攝入與所有 reparse 路徑接上身分；A8 合集集數都已下載自動略過；A9 手動改系列名稱（前端可編輯）；
      A10 回填（admin endpoint + `bangumi backfill-identity`）；作品列表背景補封面改用身分（不再標題搜尋）
    - 驗證：workspace 456 個測試通過（含 31 個 dev DB 測試，`DATABASE_TEST_URL` + `--include-ignored`）；
      本機端對端（core + metadata + 真實 bgm）新項目依身分建季、忽略 parser 作品名
    - 2026-10-18 以**生產資料副本**（pg_dump，不含 ai_providers / webhooks 資料）在本機跑回填：18/18 訂閱綁定、
      合併 36（35 個重複下載取消 + 超市後門 ep9 未下載的單集 link）、soft delete 56 作品（39 殘骸 + 17 舊作品）；
      套用後 17 個作品、名稱全來自 bgm；剩下的同集多 link 只有原本就待決的青之箱 / 藥屋，回填沒有替使用者決定
    - 觀察到（既有問題，未處理）：藥屋 S3 一個字幕組用跨季連續集數（49、50），另一個用該季集數（1、2、3）
    - [x] 2026-10-18 13:38 (+08) 部署 core / metadata / fetcher-mikanani / frontend（86e4fc6）：migration `20261018000000metadataidentity`
      已套用、metadata 註冊 `bgm`、四個服務 restarts=0；生產 dry-run 與副本結果一致且未寫入
    - [x] 2026-10-18 13:51 套用回填：18 訂閱綁定、17 個作品（全部 `title_source=bgm`、各一季帶 bgm id）、56 作品 soft delete、
      35 筆重複下載取消（`merged duplicate of download N`）＋ 1 個未下載 link 標 resolved；332 筆 raw item 全部帶 external_ids
    - [ ] 部署後觀察：新項目的 raw item 帶 external_ids、掛到 bgm 決定的季；log 沒有 `No usable identity` 警告（mikan 訂閱）
  - [x] 階段 B：Viewer 輸出（B1–B4），commit bcddb6b
    - 同步 / resync 請求帶季的 `external_id`、系列（名稱、簡介、首播日、海報）與季（名稱、原名、簡介、播出日、海報）
    - viewer 寫 `tvshow.nfo`、`Season NN/season.nfo`、系列與季的 `poster.jpg`（已存在不重抓）、單集 NFO（依 subject id 查，
      支援該季集數與跨季連續集數）
    - 驗證：core / viewer / shared 337 個測試通過；本機 viewer + metadata + 真實 bgm：金牌得主 S2 產出 NFO 與兩張海報，
      第 15 集正確對到第二季第 2 集「我的手牌」
    - 生產封面確認：沿用的舊作品（金牌得主、青之箱、藥屋）舊封面正好是第一季的圖，系列海報正確
    - [x] 2026-10-19 00:56 (+08) 部署 viewer-jellyfin → core-service（08c28ba）：兩者 healthy、restarts=0，core log 無 ERROR
    - [ ] 部署後觀察：下一集同步後該季資料夾出現 season.nfo、單集 NFO，系列資料夾出現 tvshow.nfo、poster.jpg
  - [x] 階段 C：媒體庫整理（C1–C3），commit cdafdfc、3314191、d29940f
    - C1 viewer：搬移時字幕跟著走；舊資料夾只剩產生的 NFO / 海報時一併清掉（有其他檔案就保留）
    - 修正既有 bug：`resyncing` 不在 downloads 狀態約束內，resync 狀態從未寫入成功（viewer 照做、core 不知道）；
      migration `20261019000000allowresyncingstatus`；改為送出前先搶占、resync 失敗回到 synced、5 分鐘沒回報回收為 synced
    - C2 `POST /admin/resync-library`（GET 查進度）：一次一集、等 viewer 回報才送下一集
    - C3 `POST /admin/merged-duplicates`：經 viewer 刪除回填合併掉的重複檔案（與保留檔案同路徑者不刪）
    - 驗證（本機以**最新生產資料副本** + 依生產檔名建的假媒體庫 + 本機 viewer / metadata / 真實 bgm）：
      C2 172/172 集成功（約 6 分鐘），15 個新資料夾都有 tvshow.nfo、season.nfo、單集 NFO、海報，劇場版字幕跟著搬；
      C3 刪除 34 個重複檔案，てんびん 資料夾自動清掉；第二次 dry-run 為空
    - 系統刪不到的 2 個殘檔（合集版已保留）：`Super no Ura…/S01E09`（無下載記錄，E7 孤兒）、`暴走千金…/S01E10`（下載 515 無路徑）→ 手動刪除
    - [x] 2026-10-19 14:10 (+08) 部署 viewer-jellyfin → core-service（3d4924a）：migration `20261019000000allowresyncingstatus` 已套用、
      兩者 healthy、restarts=0；生產 C2 dry-run：172 集、15 個系列，與副本一致
    - [x] 2026-10-19 C2 套用：172/172 集成功、0 失敗；15 個新資料夾都有 tvshow.nfo、season.nfo、單集 NFO、海報
      （欺詐遊戲第 21、26 集 bgm 沒有單集資料，沒有單集 NFO）
    - [x] C3 dry-run：34 個重複檔案（Super no Ura 11、てんびん 12、暴走千金 11），保留的那份都在新資料夾
    - [x] 2026-10-19 C3 套用：viewer 回報 34 筆刪除成功；てんびん 資料夾清掉；第二次 dry-run 為空；
      逐一檢查 172 個保留檔案全部存在
    - 發現：viewer 刪檔依自己的 sync_tasks 路徑，與 core 的 file_path 可能不同。下載 507（暴怒千金 ep9 單集版）在 viewer
      記錄為 `てんびん/S01E09`（當初同步到錯的資料夾），core 記錄為 `暴走千金…/S01E09` → 刪到的是 てんびん/S01E09
      （本身也是重複副本，沒有刪錯），`暴走千金…/S01E09` 留下
    - [x] 2026-10-19 使用者手動刪除 3 個殘檔與空資料夾：媒體庫剩 15 個系列資料夾
- [x] 衝突處理（使用者 2026-10-19 指定：優先「繁體＋日文」，其次「繁體」）
  - 7 個待決衝突的選擇：青之箱 ep1 → 1685、ep2 → 1691（繁日内嵌）；藥屋豌豆 ep1/2/3 → 1730/1737/1920（繁体，無日文版）；
    藥屋綠茶 ep49/50 → 1707/1703（繁日内嵌）
  - [x] 修正：已下載未同步（completed）的下載也可取消，沒選中的版本 torrent 與檔案一併移除（commit 見 git log）
  - [x] 2026-10-19 14:52 部署 core（aa91027）；使用者依上表解決 7 個衝突：未解決衝突 0；沒選中的 3 筆已下載版本
    （559、579、582）標 cancelled 並移除 torrent；選中且已下載的 560、576、581 已同步進媒體庫；其餘 4 個選中版本已派送下載
  - 發現：藥屋 S3 同時訂閱豌豆（該季集數 1、2、3）與綠茶（跨季集數 49、50），同一集在媒體庫出現兩次（S03E01 與 S03E49）
  - [x] 2026-10-19 15:18 使用者決定移除豌豆訂閱（27），purge：viewer 刪除媒體庫 3 集（3/3 成功）、qB 移除 5 個 torrent、
    刪除 5 筆下載 / 6 個 link / 6 筆原始項目；藥屋第三季資料夾只剩綠茶的 S03E49、S03E50。綁定 27 的 parser 47/51/52/59 保留（不再被使用）
- [ ] 暫緩（使用者 2026-10-20：等常發生再處理）：綠茶的藥屋 S3 用跨季集數，媒體庫為 S03E49、S03E50（應為第三季第 1、2 集）；可用 bgm 的 ep/sort 對照在解析時換成該季集數
- [x] AI 輸入縮減（使用者 2026-10-19，已部署）：過濾規則與解析器都只送前 3 集的標題（ada5bb5）；待部署 core
  - 依據：Groq 免費方案 gpt-oss-120b 每分鐘 8K token、每天 200K；生產的限流錯誤顯示單次請求就 3–4K token（Requested 3058–4121）
  - 自訂過濾 prompt 早已寫明偏好：繁日 > 繁 > 簡，解析度越高越好 → 先前失敗是限流，不是 AI 判斷問題
  - [ ] 部署後觀察：下一次 AI 生成的 pending 結果不再因限流失敗；生成的規則符合語言偏好
- [x] AI provider 設定強化（使用者 2026-10-19）：複製按鈕（伺服器端連 key 一起複製、複本停用、排最後、開啟編輯）、
  模型名稱下拉選單（打開時撈 `GET {base_url}/models`，編輯時沿用已存的 key，可打字篩選、仍可手打）；2026-10-19 16:05 已部署
- [x] 2026-10-19 AI 測試與取樣（使用者要求）：解析器改送「前三集的所有版本」（以寬鬆規則猜集數分組，生產 327 個標題猜中 315 個，
  其餘為劇場版）；新增「真實 prompt 測試」（設定頁每個 provider 的燒瓶按鈕，送正式流程相同的 prompt、只送該 provider、
  不建立待確認結果，顯示耗時、prompt、原始回應與每個標題的保留/排除或擷取結果）；2026-10-20 01:07 已部署
- [x] AI provider fallback 修正（使用者 2026-10-19）：任何一家失敗（含 400/401/403/404 設定錯誤）都換下一家；
  一輪全部失敗且其中有限流就等待後重跑（最多 5 次）；最終錯誤列出每一家的錯誤；2026-10-20 01:07 已部署（core healthy、restarts=0）
- [x] 2026-10-20 使用者修正 provider 設定：OpenRouter 兩組 max_tokens 改 8192、Mistral 改 ministral-14b-latest / 4096；四組皆啟用
- [x] viewer `/delete` 由 core 帶要刪的路徑並核對（eeccaf5）：一致才刪、不一致回報不刪、viewer 沒記錄時用 core 的、
  媒體庫外一律不刪；生產 176 筆已同步下載兩邊路徑全部一致（不會誤擋）；2026-10-20 14:39 部署（core / viewer healthy、restarts=0）
- [x] 2026-10-20 下載目錄殘檔（使用者要求：清掉現有的，也要讓之後不再累積）— 已實作，待部署
  - 現況：/downloads 35 個項目，qB 只剩 622 一個 torrent（卡 99.7%，使用者決定再等一兩天），其餘 34 個沒有任何 torrent 在用
  - 根因：同步後以 `delete_files=false` 移除 torrent，viewer 只搬走配對到的影片/字幕，合集裡沒訂閱的集數、特典、字型、音軌留在原地；
    而且只要沒有 `pending/downloading/downloader_error` 就提早移除，之後才同步的同 torrent 集數也一樣留下檔案
  - [x] 防止：同 torrent 的所有下載都不再需要下載目錄的檔案時（排除 pending、downloading、downloader_error、completed、
    syncing、sync_failed、batch_unmatched）才移除 torrent，並且 `delete_files=true`（已搬走的檔案不在原處，只會刪掉殘檔）
  - [x] 補救：下載排程每小時找「下載都已結束、但 downloader 仍保留」的 torrent，連檔案一起移除（涵蓋 callback 時 downloader 連不上、
    最後一集停在 batch_failed / cancelled 等沒有 synced 回報的情況）
  - [ ] 清理工具：core 掃描下載目錄，列出不屬於任何 downloader 仍保留的 torrent、也不被需要檔案的下載引用、且超過 24 小時沒變動的項目
    （dry-run）；確認後由 viewer（唯一可寫入下載目錄的服務）刪除，只接受下載目錄第一層、且刪除前重新掃描確認仍是孤兒；
    API + 設定頁按鈕
  - 實作：`sync_service`（`NEEDS_DOWNLOAD_FILES`、`release_settled_torrents`，downloader 回報 downloading/checking 的不動、刪前重查 DB）、
    `download_dir_cleanup`（`POST /admin/download-orphans`）、viewer `POST /downloads/cleanup`、設定頁「下載目錄清理」卡片
  - 驗證：假 downloader 的 DB 測試（只移除已結束的 torrent 且 delete_files=true、忙碌中的不動、保護清單正確）、掃描/刪除單元測試
  - 已知取捨：同 torrent 還有 completed / sync_failed 的集數時 torrent 會留著；qB 重啟時可能重抓已搬走的集數，最後移除時一併刪掉
  - 已知限制：使用者手動加進 qB、core 沒有記錄的 torrent 不在保護清單裡（靠 24 小時與刪除前確認）
  - [ ] 部署後：dry-run 清單應為 34 個（622 的檔案不在內）；使用者確認後刪除；之後新同步的 log 出現 `Removed torrent … and its leftover files after sync`
- [x] 2026-10-20 fetcher 的 pub_date 全部是 NULL — 已實作，待部署
  - 根因：mikanani 的發佈時間放在 `<torrent xmlns="https://mikanani.me/0.1/"><pubDate>`（無時區、UTC+8，
    已用 torrent 的 Last-Modified 比對確認），feed-rs 與 rss crate 都略過這個元素
  - [x] fetcher 用 quick-xml 讀出每個 item 的 enclosure URL 與 torrent/pubDate，以 UTC+8 轉換；標準 pubDate 仍優先
  - [x] core 收到已存在的項目時補上缺少的 pub_date（RSS 還列著的舊項目會自動補齊）
  - [x] 部署後：下一輪抓取後 `raw_anime_items.pub_date` 非 NULL 的筆數增加
    - 2026-10-22 確認：332/332 全部有值（舊項目也由 RSS 補齊），範圍 2018-02-28 ~ 2026-10-21
- [x] 2026-10-20 確認：下載排程對 qB `not_found` 已有處理（連續失蹤超過 10 分鐘標 failed），舊紀錄的 `_ => continue` 已不存在

- [ ] 2026-10-22 狀態檢查發現
  - 待決衝突實際 4 筆（不是 1 筆）：新的「简繁日内封」版本把 10/19 已解決的青之箱 ep2、藥屋 49、50 重新打開
    （status 改回 unresolved，resolved_at / chosen_link_id 保留舊值，已選版本早已 synced），加上藥屋 51（繁日 / 簡日）
  - 訂閱 28（青之箱）、30（藥屋）沒有任何過濾規則：AI 過濾規則只在訂閱精靈第 3 步或手動重新生成時產生，
    10/15 那次因限流失敗後就沒再產生；排程抓到的新衝突不會觸發 AI
  - 622 已卡 6 天（99.7%）；另一個版本 CR（link 1621）被過濾規則排除
  - 下載目錄 20 個孤兒尚未刪除（使用者在設定頁操作）
  - 備份 10/20、10/21 皆成功；部署後沒有新的下載 / 同步，同步相關的觀察項目仍無資料

- [x] 2026-10-22 使用者決定（1. 功能做好後自己把 622 換成 CR；2. 目前偏好內嵌，未來可能改；3. 自動產生過濾規則與防重複照方案做）
  - [x] 「改用這個版本」（手動指定該集版本）
    - migration：`anime_links.manual_choice BOOLEAN NOT NULL DEFAULT false`
    - `POST /links/:link_id/choose`：被選版本 manual_choice=true、filtered_flag=false、active、清 conflict_flag；
      同集（同季、同字幕組、同集數）其他版本 resolved、manual_choice=false；該集的衝突紀錄標為已解決（選這個版本）；
      取消其他版本的下載（下載中 / 已下載未同步，連 torrent 與檔案）；派送被選版本。已同步進媒體庫的舊版本保留（與解衝突一致），回應裡註明
    - 過濾重算時 manual_choice 的版本一律不過濾
    - 衝突偵測前：集數有 active 的手動指定版本時，其他 active 版本自動改 resolved（之後的新版本不再產生衝突）
    - 前端：作品季對話框每個版本加「改用此版本」（確認視窗說明影響），顯示「手動指定」「未採用」標籤；DTO 加 link_status、manual_choice
  - [x] 衝突重新打開時清掉 resolved_at、chosen_link_id，created_at 改為重新打開的時間（只在原本是 resolved 時）
  - [x] 自動產生過濾規則：抓取 / 解析器確認後若訂閱有未解決衝突就請 AI 產生（結果進待確認清單）
  - [x] 防重複（過濾規則與解析器共用判斷）：同訂閱最新一筆是 generating / pending → 不產生；最新一筆 failed 且未滿 6 小時 → 不產生；
    最新一筆 confirmed / rejected 且之後沒有新的衝突 / 未匹配項目 → 不產生
  - [ ] 內嵌偏好：在設定頁的過濾 prompt 補上「內嵌優先於內封」（使用者自行貼上，給文字）
  - 實作：`services/version_choice.rs`、`POST /links/:link_id/choose`、`ai/auto_trigger.rs`（`should_generate`、`parser_due`、
    `spawn_filter_generation_if_due`；抓取派送後與解析器確認後觸發）、`anime_link_conflict` upsert 重新打開時清欄位、作品季對話框按鈕
  - 驗證：DB 測試（指定版本、之後新版本不採用、過濾重算不影響指定版本、重新打開清欄位、解析器等待中不再產生、衝突分組）、
    判斷邏輯單元測試；本機起 core 打 `/links/0/choose` 回 404、`/links/1` 回含新欄位的列表
  - [ ] 部署後：使用者在無自覺聖女第 11 集對 CR 版按「改用此版本」→ 622 取消（qB 移除、半成品刪除）、CR 派送、同步後媒體庫 S01E11 出現
  - [ ] 部署後：下一次抓到新衝突時 log 出現「自動請 AI 產生過濾規則」，待確認清單出現結果；同訂閱不會再出第二筆
- [ ] 技術債：`tests/anime_api_tests.rs` 把資料 commit 進測試 DB（沒有 rollback），第二次跑就撞 unique constraint

- [ ] 2026-10-24 Prompt 簡化測試（使用者要求：用設定好的每個模型、搭配不同動畫標題測試 prompt 能否簡化）
  - 發現：預設解析器 prompt 的「單集範例」本身寫錯（parse_regex 只有 2 個擷取群組，範例卻用 `$2` 當集數、`$3` 當解析度）
  - [x] `/ai-providers/test-prompt` 加上可選的 `fixed_prompt` / `custom_prompt`：只用於這次測試，不寫進設定、不影響正式生成
  - [x] 簡化版 prompt（解析器、過濾規則各一份），修正範例錯誤（解析器 6736 → 1627 字元、過濾 2781 → 743 字元）
  - [ ] 測試矩陣：4 個 provider × {目前, 簡化} × 不同格式的訂閱（解析器：25 尼古喵喵、8 欺詐遊戲合集、30 藥屋第三季、19 無自覺聖女；
    過濾：28 青之箱、17 冰之城牆、7 地獄樂），共 56 次呼叫；腳本與結果在 scratchpad `prompt-eval/`
    - 解析器評分：JSON / regex 有效、標題匹配率、擷取的集數與 DB 既有連結一致的比例
    - 過濾評分：每個衝突群組恰好保留一個、且保留的是偏好版本（繁日 > 繁 > 簡，內嵌優先）
    - 預算：OpenRouter 兩組共用帳號免費額度（每天約 50 次），Groq 每分鐘 8K token → 依序執行、限流時等待重試
  - [x] 2026-10-24 測試完成（每格：解析器＝集數正確 / 標題數，過濾＝選對的群組 / 群組數）
    | provider | 解析器 目前 | 解析器 簡化 v1 / v2 / v3 | 過濾 目前 | 過濾 簡化 v1 / v2 |
    |---|---|---|---|---|
    | groq gpt-oss-120b | 23/23 | 17/23 / 23/23 / 23/23 | 9/9 | 9/9 / 9/9 |
    | mistral ministral-14b | 46/46（跑兩輪） | 6/23 / 14/23 / 26/46（兩輪） | 0/9 | 0/9 / **9/9** |
    | openRouter nemotron-3-super:free | 全部 404（上游 `Function … Not found`，模型已不可用） | | | |
    | openRouter（複本）nemotron-3.5-super-vl:free | 3/23 | 0/23 | 3/9 | 0/9 |
    - v1 的 `(also 【】)` 是我寫錯：讓模型寫出 `\【`，Rust regex 不接受
    - mistral 過濾失敗原因：每個版本各寫一條正向規則（以為是 OR），實際是 AND → 一個都不保留；v2 明講「只寫一條正向規則」後全對
    - mistral 解析器只有長 prompt 穩定（兩輪 8/8）；短 prompt 會寫出跟 parse_regex 不一致的 condition_regex、多餘的擷取群組
    - openRouter（複本）回應要 30–50 秒，常超過 AI client 的 60 秒逾時（`openai.rs:26`）→ `error decoding response body`
    - 目前預設解析器 prompt 的單集範例錯誤（`$2`/`$3`）仍在
    - 候選 prompt：scratchpad `prompt-eval/parser_simple3.txt`、`filter_simple2.txt`；結果 `results.jsonl`、`repeat.jsonl`
  - [x] 使用者決定：過濾 prompt 換成 v2？解析器維持長版（修範例）或做「全域 → 模型 → 客製化」三層？openRouter 兩組怎麼處理？
    - 2026-10-24 使用者決定：解析器與過濾都改成「全域 → 模型 → 客製化」三層；逾時改為可手動設定
- [ ] 三層 prompt 與可設定逾時
  - [x] migration：`ai_providers` 加 `timeout_secs`（預設 60，5–600）、`parser_prompt`、`filter_prompt`（NULL / 空白 = 用全域）
  - [x] chain：每個 provider 用自己的 prompt（模型 > 全域），手動重新生成時編輯的 prompt 仍套用到所有 provider；
    pending 的 used_fixed_prompt 記實際成功那家用的 prompt
  - [x] OpenAiClient 用 provider 的 timeout_secs；連線測試 / prompt 測試可帶未儲存的逾時
  - [x] prompt 測試：測試指定 > 該 provider 的模型 prompt > 全域；回應顯示實際用的
  - [x] API / 複製 / DTO 帶新欄位；前端 provider 編輯視窗加逾時與兩個模型 prompt
  - [x] 內建預設：過濾換成 v2；解析器維持長版但修正範例的 `$1` / `$2`
  - 驗證：chain 三個新測試（fallback 後用自己的模型 prompt、空白視為沒設定、重新生成指定的覆蓋模型 prompt）、
    預設 prompt 範例檢查測試（$N 不超過擷取群組數、過濾範例只留偏好版本）、handler 逾時範圍 / 空白 prompt 測試、
    複製帶新欄位的 DB 測試、前端對話框測試；本機起 core 實際打 API（逾時 3 → 400、建立 / 清除 / 複製 / 部分更新）
  - [x] 2026-10-25 部署（44a2103）：migration 已套用（4 個 provider timeout 60、模型 prompt 空）；groq 真實測試用到新的過濾預設，青之箱 3 組都只留繁日內嵌
  - [x] 使用者在 UI 把 groq 的解析器 prompt 設成 `docs/prompts/parser-short.txt`；openRouter 兩組停用或調逾時
    - 2026-10-25 使用者設定：openRouter 停用、groq 解析器用短版（2426 字元）、openRouter（複本）逾時 120 秒
    - 驗證（不指定 prompt，各模型用自己的設定，21 次）：groq 解析 23/23、過濾 9/9（全對，目前實際上的第一順位）；
      mistral 解析 22/23、過濾 6/9；openRouter（複本）解析 23/23、過濾 6/9（回應 15–117 秒，1 次超過 120 秒逾時）
    - mistral 的失誤都是 regex 寫不精準（冰之城牆把 `[繁体内嵌]` 放在集數前面、尼古喵喵寫成 `-\d+` 漏了空格），不是理解錯誤
- [ ] 2026-10-25 補強 prompt（使用者要求：讓模型更不容易失效）
  - 過濾：照標題順序、中間用 `.*` 跳過、集數一律 `\d+`、送出前逐組驗證；解析器：結尾加「送出前最後檢查」；groq 短版加精簡檢查
  - 測試（各模型用自己的那份）：groq 解析 23/23、過濾 9/9；mistral 兩輪解析 46/46、過濾 18/18（補強前 22/23、6/9）；
    openRouter（複本）成功的 2 次全對，其餘 5 次上游 502（OpenRouter 免費模型故障，與 prompt 無關）
  - [x] 內建預設換成補強版、`docs/prompts/parser-short.txt` 更新（aec337c）
  - [x] 2026-10-28 部署 core（補強版內建 prompt 生效）
  - [ ] 使用者把 groq 的解析器 prompt 換成新版 `parser-short.txt`

- [ ] 2026-10-28 狀態檢查發現：downloader 登入不了 qBittorrent（最高）
  - 現象：主機 10/27 重開後，每小時的「移除已結束 torrent」掃描都回 500；downloader log `Status query failed: 403 Forbidden`、
    `re-login failed: Re-login failed: `（內容空白），共 27 次
  - 根因：qBittorrent 5.2.0 起登入成功改回 `204 No Content`（無內容），程式只認 `200` + `Ok.` → 把成功當失敗、不重試原請求。
    10/17 啟動登入就已經「失敗」，但 qB 其實發了 session cookie，加上 622 / 630 每 60 秒輪詢讓 session 一直有效，所以看不出來；
    630 同步完、主機重開後只剩每小時一次的請求，qB session 閒置 1 小時過期 → 每次都 403 → 重新登入「失敗」
  - 影響：新集數派送時第一次加入 torrent 會失敗（之後要靠重試）；目前沒有待下載的集數
  - 參考：qbittorrent/qBittorrent#23270、Radarr/Radarr#11339
  - [x] 修正：`login_outcome()`（204 / `Ok.` / 空白 = 成功，`Fails.` = 帳密錯誤），登入與重新登入共用；單元 + 模擬 qB 5.2 的整合測試
  - [x] 2026-10-28 10:56 部署 downloader-qbittorrent + core（1ec1465）：啟動 log `Successfully logged in to qBittorrent`、狀態查詢回 200、core 掃描不再警告
- 2026-10-28 其他狀態：630（CR 第 11 集）10/26 已同步；新訂閱 31（羅梅莉亞戰記）AI 解析器確認後 4 集同步；過濾 #115 確認 → 青之箱衝突解決；
  仍未解決：藥屋 49/50/51（綠茶 10/21 後沒有新集數）；補強 prompt（aec337c）尚未部署、groq 的解析器 prompt 仍是舊版短版

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
