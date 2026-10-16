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
- [ ] 6. core 與 downloader-qbittorrent 掛載 `${DOWNLOADS_DIR}:/downloads:ro`（稽核 E2：video_file 全為 NULL、合集全部 batch_failed、資料夾型 torrent 同步失敗）
  - 挑影片改為「最大的影片檔」而非第一個（BD 資料夾含特典 / 選單）
- [ ] 7. downloader_error 恢復（稽核 E3）
  - `apply_completed_files` 同時處理 `downloading` 與 `downloader_error`
  - 每次輪詢都做恢復檢查，不再只在「沒有 downloading」時才做
  - 連續失敗 N 次才標 `downloader_error`，單次網路錯誤不全標
- [ ] 8. qB 回報 `not_found` 的處理（稽核 E4）
  - downloader 加入 URL 型 torrent 後輪詢 `torrents/info` 確認真的出現才回報成功
  - core：`downloading` 持續 `not_found` 超過 N 分鐘 → `failed`，交給既有重試
- [ ] 9. 派送前一律先偵測衝突（稽核 E5）
  - `pending_ai_results.rs:471`（rerun）與 `raw_items.rs:347`（單筆 reparse）漏了；收斂到共用函式 / `dispatch_new_links` 內
- [ ] 10. viewer 同步冪等（稽核 E6）：來源不存在但目標已存在 → 回報 `synced`
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
| 42 筆 `batch_failed`（6 個合集） | 項目 6 部署 | 改回 `batch_unmatched`、`sync_retry_count=0`，讓排程重新配對 |
| 421 魔法科劇場版（資料夾） | 項目 6 部署 | 改回 `completed` 並重新取得 `video_file`（或觸發一次完成處理） |
| 563–566（downloader_error，qB 已完成） | — | 重啟時被重試機制刪除，link 因衝突未重派，目前無 download 紀錄；qB 裡的 torrent 與檔案還在。解衝突後選中的 link 會重派，qB 已有該 torrent → 直接完成並同步 |
| ~~555（downloading，qB not_found）~~ | — | 已自動處理：重啟時重派為 609，qB 這次成功加入 |
| 559/560、576/579/581/582、563–566 | 使用者 | UI 上解決 6 筆衝突（1581/1583/1589/1591/1609/1628）；未選中且已下載的版本從 qB 移除並刪檔 |
| 506、508（sync_failed，實際已同步） | 無 | 改 `synced`、`file_path` 填媒體庫路徑，從 qB 移除 torrent |
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
