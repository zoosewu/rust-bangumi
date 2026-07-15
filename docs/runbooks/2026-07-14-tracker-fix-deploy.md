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

**只啟動 postgres**,其他服務保持停止——尤其 fetcher 絕不能在遷移前啟動,
否則會以舊格式繼續攝入,造成重複:

```bash
docker compose up -d postgres
until docker exec bangumi-postgres pg_isready -U bangumi >/dev/null 2>&1; do sleep 1; done
```

**執行前務必確認映射涵蓋率。** 映射表是「收割當下的 RSS 快照」,若停止 stack 前
fetcher 又抓進新集數,那些項目不在映射內,執行後會殘留 magnet 並在部署後重複攝入
(2026-07-15 即因此從 190 補到 192)。下列檢查在 DB 查不到資料時會**明確報錯**
而非誤判通過:

```bash
grep -oE "^  \('[0-9a-f]{40}'" scripts/2026-07-14-fix-magnet-urls.sql \
  | grep -oE "[0-9a-f]{40}" | sort -u > /tmp/sql_hashes.txt
docker exec bangumi-postgres psql -U bangumi -d bangumi -t -A -c \
  "SELECT substring(download_url from 'btih:([0-9a-f]+)') FROM raw_anime_items \
     WHERE download_url LIKE 'magnet:%';" | sort -u > /tmp/db_hashes.txt

sql_n=$(wc -l < /tmp/sql_hashes.txt); db_n=$(wc -l < /tmp/db_hashes.txt)
uncovered=$(comm -13 /tmp/sql_hashes.txt /tmp/db_hashes.txt | wc -l)
echo "sql=$sql_n db=$db_n uncovered=$uncovered"
if [ "$sql_n" -eq 0 ] || [ "$db_n" -eq 0 ]; then
  echo "ABORT: 清單為空 — DB 未啟動或查詢失敗，這不是通過"
elif [ "$uncovered" -ne 0 ]; then
  echo "ABORT: 有 $uncovered 筆未涵蓋 — 需重新自 RSS 收割並更新映射表"
else
  echo "OK: 涵蓋率 100%，可以執行遷移"
fi
```

**兩個數字都非 0 且 uncovered=0 才能往下做。** 確認後執行:

```bash
docker exec -i bangumi-postgres psql -U bangumi -d bangumi -v ON_ERROR_STOP=1 \
  < scripts/2026-07-14-fix-magnet-urls.sql
```

腳本末尾會輸出驗證:`raw_items magnet remaining` 與 `anime_links magnet remaining`
應為 **0**,downloads 應出現 `failed=23`(等待自動重派)。腳本冪等,重跑無害。

### 4. 部署新版鏡像並啟動

重建 fetcher-mikanani 與 core-service 鏡像後 `docker compose up -d`。

### 5. 驗證

1. **自動重派**(downloader 註冊時觸發,約啟動後數秒):
   ```bash
   docker logs bangumi-core 2>&1 | grep "Retried failed downloads"
   # 期望: Retried failed downloads: 23 dispatched, 0 no_downloader, 0 failed again
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
