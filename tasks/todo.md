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

## 部署後驗證(runbook 第 5 節)

- [ ] `Retried failed downloads: 22 dispatched` 且只出現一次
- [ ] downloads 無重複 link_id
- [ ] qBittorrent 種子帶完整 tracker 清單且有進度
- [ ] 下次 RSS 抓取無重複攝入(raw_items 不爆增)

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

- fetcher pub_date 解析(mikanani torrent:pubDate 命名空間欄位)
- 主機對外 UDP / DHT 環境檢查
- download_scheduler 對 not_found 狀態的處理
