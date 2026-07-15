# 死 tracker magnet 轉換 + 註冊重試競態修復設計

**日期**: 2026-07-14(第二階段;第一階段見 `2026-07-14-qbittorrent-login-race-fix-design.md`)
**狀態**: 已核准

## 背景

第一階段修復 qBittorrent 登入競態後,自動重試成功派送 22 個種子,但:

1. **下載全部卡 0%**:fetcher 把 mikanani `.torrent` URL 轉成僅含 hash 的 magnet,
   掛上 3 個硬編碼 tracker(acgtracker / nyaatracker / openbittorrent)——實測全部已死。
   真實的 mikanani torrent 檔內含十餘個現役 tracker,被轉換丟棄。
   過去靠 DHT 掩蓋;主機環境 DHT 現為 0 節點(對外 UDP 疑似不通)→ 完全無 peer 來源。
2. **downloads 重複記錄(22×2)**:每個 downloader 註冊都觸發全域
   `retry_failed_downloads()`,qbittorrent 與 pikpak 同毫秒註冊 → 兩個併發 retry
   read-before-delete 競態 → 同批 failed 被重派兩次。

## 修正設計

### Fix A: fetcher 保留原始 .torrent URL

- `fetchers/mikanani/src/rss_parser.rs`:移除 `TRACKERS` 與 `torrent_url_to_magnet()`,
  download_url 直接使用 RSS enclosure 的原始 URL。
- entry→RawAnimeItem 映射抽為純函式 `entry_to_raw_item()`,以樣本 RSS XML 單元測試。
- 下游相容性(已逐一確認):
  - `download_type_detector` 已把 `.torrent` URL 判為 `torrent` 型別(含測試)。
  - qBittorrent downloader `torrents/add` 原生支援 URL;`extract_hash_from_url()`
    已支援 `{hash}.torrent` 檔名。
  - PikPak 生產中未被使用(0 筆下載),其 capabilities 為 [Magnet, Http],
    torrent 型別自然只派給 qBittorrent,不需改動。

### Fix B: 註冊重試原子化

- `retry_failed_downloads()` 與 `retry_no_downloader_links()`:
  「先 SELECT 再 DELETE」改為單一 `DELETE ... RETURNING link_id`。
  併發呼叫各自取得不相交集合,天然防止重複派送,無需鎖。

### 資料遷移(scripts/2026-07-14-fix-magnet-urls.sql)

既有 190 筆 magnet URL 是去重鍵,不改寫會在 fetcher 升級後造成整批重複攝入。
`pub_date` 全為 NULL(另一潛在 bug)無法重建日期路徑,故映射表取自
**部署當日實際收割的 RSS**(與 DB hash 100% 對應,已驗證):

1. `raw_anime_items.download_url`:magnet → 對應 .torrent URL。
2. `anime_links`:`url` 改寫、`download_type='torrent'`、
   `source_hash` 重算為 `sha256(new_url)`(保留批次 `#epN` 後綴;reparse 依賴此不變量,
   UNIQUE 約束因映射 1:1 不會碰撞)。
3. 去除重複的 downloading 記錄(每 link 保留最早一筆)。
4. 卡 0% 的 downloading 重置為 failed → 部署後註冊重試自動以新 URL 重派。

已在拋棄式 Postgres(套用全部 repo 遷移)上端到端驗證,含冪等性(重跑全 0)。

### 部署順序

見 `docs/runbooks/2026-07-14-tracker-fix-deploy.md`。關鍵:stack 停止時執行 SQL,
避免新舊 fetcher 與資料格式交錯造成重複攝入;qBittorrent 中 0% 種子先刪除
(只掛死 tracker,留著永遠卡住)。

## 範圍外(記錄為後續事項)

- fetcher `pub_date` 解析(mikanani `<torrent:pubDate>` 為命名空間欄位,feed_rs 未映射)。
- 主機對外 UDP / DHT 環境問題(修好 tracker 後不再依賴,但值得修)。
- download_scheduler 對 `not_found` 狀態的靜默忽略(`_ => continue`)。
