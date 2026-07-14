# qBittorrent Downloader 登入競態修復設計

**日期**: 2026-07-14
**狀態**: 已核准

## 背景(生產事故)

2026-06-09 02:50 生產環境全部容器同時重啟。downloader-qbittorrent 在 qBittorrent WebUI
就緒前嘗試啟動登入,失敗(connection error)。由於 `login()` 只在登入成功後才儲存憑證,
`credentials` 保持 `None`,導致後續所有 403 觸發的 `re_login()` 因「No stored credentials」
而失敗。自此每次新增 torrent 都收到 `403 Forbidden`,5 週內 32 筆下載全部 failed,
且 `downloads.error_message` 為空(dispatch 未落庫失敗原因),問題未被察覺。

## 根本原因

1. **憑證儲存時機錯誤**(`downloaders/qbittorrent/src/qbittorrent_client.rs`):
   憑證來自環境變數,卻要等一次成功登入才存入 client,使 403 自動重登機制在啟動競態下失效。
2. **403 重登只覆蓋 `add_torrents`**:`query_status`、`cancel_torrents`、`pause/resume/delete`
   無重登處理,session 正常過期也會使狀態輪詢失效。
3. **失敗原因被丟棄**(`core-service/src/services/download_dispatch.rs`):
   downloader 回傳的 `reason` 未寫入 `downloads.error_message`,故障不可見。

## 修正設計

### Fix 1: 憑證建構時注入(根因)

- `QBittorrentClient` 新增 `set_credentials(&self, username, password)`:僅存入
  `credentials`,不發送請求。
- `main.rs`:當環境變數有帳密時,先 `set_credentials()` 再嘗試 `login()`。
  啟動登入失敗仍為 WARN——之後任何 403 會以已存憑證自動重登,系統自癒,
  不依賴容器啟動順序。
- `login()` 成功後儲存憑證的既有行為保留(`/config/credentials` 與 `bangumi qb-login` 路徑)。

### Fix 2: 通用 403 重登 helper

- 新增 `send_with_relogin()`:接受可重建請求的 closure,發送後若收到 403,
  執行 `re_login()` 成功則重建請求再送一次(僅一次)。
- 套用至全部六個 API 方法,移除 `add_torrents` 內的手寫特例。

### Fix 3: dispatch 失敗原因落庫

- `dispatch_new_links()` 以 `HashMap<link_id, String>` 追蹤每個 link 最後一次被拒原因
  (格式 `{downloader 名稱}: {reason}`;網路錯誤同樣記錄)。
- `create_download_record()` 增加 `error_message: Option<&str>` 參數;
  `NewDownload` 增加 `error_message` 欄位(DB 欄位已存在,無需 migration)。
- 寫入 `failed` 記錄時帶入累積原因;其他狀態傳 `None`。

## 既有壞狀態遷移

**零程式碼**。Core 既有 `retry_failed_downloads()` 會在 downloader 註冊時自動重派
所有 `failed`/`downloader_error` 記錄。部署修復版後驗證:

1. Core 日誌出現 `Retried failed downloads: N dispatched, ..., 0 failed again`。
2. SQL 確認 `downloads` 無 `failed` 殘留、轉為 `downloading`/`completed`。

## 測試

- **client 層**(downloader crate,以 axum 起本地 mock qBittorrent server):
  - 未登入(啟動登入失敗)→ 首個請求 403 → 自動重登 → 重試成功。
  - 無憑證時請求失敗且不重試。
  - session 過期後 `query_status` 也能重登(覆蓋非 add 路徑)。
- **dispatch 層**(core-service 既有測試模式):rejected reason 寫入 `error_message`。
- `cargo fmt` / `cargo clippy` / `cargo test` 全數通過。

## 範圍外

- healthcheck 檢查 qBittorrent 連線(自癒後屬加分項)。
- 啟動退避重試登入(與惰性自癒重複)。
- raw_items 去重日誌降噪。
