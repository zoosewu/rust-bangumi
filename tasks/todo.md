# qBittorrent 登入競態修復 (2026-07-14)

Spec: `docs/superpowers/specs/2026-07-14-qbittorrent-login-race-fix-design.md`

## 計畫

- [x] Fix 1: `QBittorrentClient::set_credentials()` + `main.rs` 啟動時先注入憑證
- [x] Fix 2: `send_with_relogin()` helper 套用至六個 API 方法,移除 add_torrents 特例
- [x] Fix 3: dispatch 失敗原因寫入 `downloads.error_message`(`NewDownload` + `create_download_record`)
- [x] 測試: client 層 mock qBittorrent server 測 403 自動重登(含非 add 路徑)
- [x] 測試: dispatch 層 `format_fail_reason` 單元測試
- [x] `cargo fmt` / `cargo clippy` / `cargo test` 全過
- [x] Review 章節總結

## 遷移(部署後人工驗證,零程式碼)

- [ ] Core 日誌: `Retried failed downloads: N dispatched, ..., 0 failed again`
- [ ] SQL: `SELECT status, COUNT(*) FROM downloads GROUP BY status;` 無 failed 殘留

## Review

- `downloaders/qbittorrent/src/qbittorrent_client.rs`:
  - 新增 `set_credentials()`(只存憑證不發請求)與 `send_with_relogin()`(403 → 重登 → 重試一次)。
  - 六個 API 方法(`add_torrents`/`cancel_torrents`/`query_status`/`pause`/`resume`/`delete`)
    全部改走 `send_with_relogin()`,移除原本只在 `add_torrents` 的手寫 403 特例。
- `downloaders/qbittorrent/src/main.rs`: 啟動時先 `set_credentials()` 再嘗試登入;
  登入失敗訊息改為說明會於 403 時自動重登。
- `core-service`:
  - `NewDownload` 增加 `error_message` 欄位(DB 欄位既有,無需 migration)。
  - `create_download_record()` 改收 `DownloadRecord` 參數結構(具名欄位,同時修 clippy too_many_arguments)。
  - dispatch 以 `fail_reasons: HashMap<link_id, String>` 追蹤最後被拒/出錯原因,
    寫入 failed 記錄的 `error_message`;新增純函式 `format_fail_reason()`。
- 測試: 新增 `tests/integration/relogin_tests.rs`(axum mock qBittorrent,3 個測試)
  與 `format_fail_reason` 2 個單元測試。全 workspace `cargo test` 通過,
  clippy 無新增警告(僅存 repo 既有警告)。
- 既有 32 筆 failed 的遷移依賴 Core 既有 `retry_failed_downloads()`
  (downloader 註冊時自動全量重派),不需要資料遷移。
