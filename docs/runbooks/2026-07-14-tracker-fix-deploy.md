# 2026-07-14 死 tracker 修復部署 Runbook

## 背景

- **修了什麼**:fetcher 不再把 mikanani `.torrent` URL 轉成「hash + 3 個已死 tracker」的
  magnet(`fetchers/mikanani/src/rss_parser.rs`);core 的註冊觸發重試改為原子化,
  杜絕多 downloader 同時註冊造成的重複派送(`core-service/src/services/download_dispatch.rs`)。
- **為何要照順序**:資料庫裡 190 筆既有 magnet URL 是去重鍵(`raw_anime_items.download_url`
  UNIQUE)。若新 fetcher 先跑、SQL 後跑(或反之),中間任何一次 RSS 抓取都會把同一批集數
  當成新項目重複攝入。**必須在整個 stack 停止時完成資料修復**。

## 部署步驟

### 1. 停止 bangumi stack(qBittorrent 與 postgres 除外)

```bash
docker stop bangumi-core bangumi-fetcher-mikanani bangumi-downloader-qbittorrent \
  bangumi-downloader-pikpak bangumi-viewer-jellyfin bangumi-metadata bangumi-frontend
```

### 2. 刪除 qBittorrent 中卡住的種子(0%,無資料損失)

這些種子只掛著死 tracker,留著只會繼續卡住。取出 hash 清單並刪除
(`deleteFiles=false`;它們本來就沒下載到東西):

```bash
HASHES=$(docker exec bangumi-postgres psql -U bangumi -d bangumi -t -A -c \
  "SELECT string_agg(DISTINCT torrent_hash, '|') FROM downloads WHERE status='downloading' AND COALESCE(progress,0)=0;")
docker exec bangumi-qbittorrent sh -c "
  COOKIE=\$(curl -s -i --data 'username=admin&password=zoo-qbit-bangumi' http://localhost:18080/api/v2/auth/login | grep -i set-cookie | sed 's/.*SID=\([^;]*\).*/\1/');
  curl -s -b \"SID=\$COOKIE\" --data 'hashes=$HASHES&deleteFiles=false' http://localhost:18080/api/v2/torrents/delete"
```

（或直接在 qBittorrent WebUI `:18080` 全選 0% 的種子刪除。）

### 3. 執行資料修復 SQL

```bash
docker exec -i bangumi-postgres psql -U bangumi -d bangumi -v ON_ERROR_STOP=1 \
  < scripts/2026-07-14-fix-magnet-urls.sql
```

腳本末尾會輸出驗證:`raw_items magnet remaining` 與 `anime_links magnet remaining`
應為 **0**,downloads 應出現 `failed=22`(等待自動重派)。腳本冪等,重跑無害。

### 4. 部署新版鏡像並啟動

重建 fetcher-mikanani 與 core-service 鏡像後 `docker compose up -d`。

### 5. 驗證

1. **自動重派**(downloader 註冊時觸發,約啟動後數秒):
   ```bash
   docker logs bangumi-core 2>&1 | grep "Retried failed downloads"
   # 期望: Retried failed downloads: 22 dispatched, 0 no_downloader, 0 failed again
   # 且整個啟動過程只出現一次(原子化修復生效)
   ```
2. **無重複記錄**:
   ```bash
   docker exec bangumi-postgres psql -U bangumi -d bangumi -c \
     "SELECT link_id, COUNT(*) FROM downloads WHERE status='downloading' GROUP BY link_id HAVING COUNT(*)>1;"
   # 期望: 0 rows
   ```
3. **實際有進度**(等 2–5 分鐘):
   qBittorrent WebUI 應顯示種子帶有完整 tracker 清單(nyaa.tracker.wf、t.acg.rip、
   opentrackr 等)且 progress > 0。若個別老種子(2018 年的)無 seeder 屬正常,
   會停在 stalled;近期新番應會動。
4. **下一次 RSS 抓取無重複攝入**(整點後檢查):
   ```bash
   docker exec bangumi-postgres psql -U bangumi -d bangumi -c \
     "SELECT COUNT(*) FROM raw_anime_items;"
   # 期望: 190 + 僅有真正的新集數,不會爆增
   ```

## 已知殘留(與本次修復無關,建議另行處理)

- **主機對外 UDP 疑似不通**:qBittorrent `dht_nodes=0`、`connection_status=firewalled`、
  UDP tracker 全部 timeout。修好 tracker 後不再依賴 DHT,但建議檢查路由器/防火牆
  對外 UDP 與 16881 埠轉發,DHT 恢復可再提升 peer 來源。
- **fetcher 未解析出 pubDate**:全部 raw items 的 `pub_date` 為 NULL
  (mikanani 的 `<torrent:pubDate>` 是命名空間擴充欄位,feed_rs 未映射)。
  不影響下載流程,屬資料品質問題,待後續修復。
