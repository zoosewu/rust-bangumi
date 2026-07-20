# qBittorrent 設定

## 兩個檔案的角色

| 檔案 | 用途 | 如何生效 |
|------|------|-----------|
| `dev.conf` | 開發環境 | `docker-compose.dev.yaml` 直接掛載為容器內的 `/config/qBittorrent/qBittorrent.conf` |
| `prod.conf` | 生產環境的**初始設定** | 由 `qbittorrent-init` 容器於首次啟動時複製(見下方) |

## 生產環境如何套用 prod.conf

生產的 compose 掛載的是**目錄**(`${QBITTORRENT_CONFIG_DIR}` → `/config`,預設 `./data/qbittorrent`),
而非單一 conf 檔。原因是 `qBittorrent.conf` 是 **runtime state**:WebUI 改設定、Core 透過 API 改設定,
qBittorrent 都會寫回這個檔案——唯讀掛載會讓 WebUI 無法存檔。

因此無法像 dev 那樣直接掛載 `prod.conf`。改由 `docker-compose.override.yaml` 中的
**`qbittorrent-init` 一次性容器**處理:qBittorrent 啟動前,它會檢查設定是否已存在,
不存在才把 `prod.conf` 複製過去。這讓**新部署自動獲得與生產一致的設定**,無需手動步驟。

- **冪等**:設定已存在時不覆蓋,保留該環境後續透過 WebUI/API 做的 runtime 調整。
- **`depends_on: service_completed_successfully`** 確保複製完成後 qBittorrent 才啟動。

### 之後如何讓 prod.conf 生效的變更套用到既有環境

init 只在「設定不存在」時執行,所以**改了 `prod.conf` 不會自動套用到已有設定的環境**。
既有環境要套用,需透過 WebUI/API 手動調整(並依下方流程同步回 repo),
或在停止服務後刪除 `$QBITTORRENT_CONFIG_DIR/qBittorrent/qBittorrent.conf` 讓 init 重新複製。

`data/` 未納入版控,所以各環境的實際 conf(含 runtime 調整)不在 git 裡——
這也是為什麼 `prod.conf` 必須是關鍵設定的**期望值文件**。

## 關鍵設定與理由

`[BitTorrent]` 區段中,以下設定是**刻意為之**,不要隨意改動:

| 設定 | 值 | 理由 |
|------|-----|------|
| `Session\QueueingSystemEnabled` | `true` | 啟用佇列,避免同時下載過多 |
| `Session\MaxActiveDownloads` | `10` | 同時下載上限 |
| `Session\IgnoreSlowTorrentsForQueueing` | `true` | **見下方 2026-07-17 變更** |
| `Session\SlowTorrentsInactivityTimer` | `1200` | 同上(20 分鐘) |

### 2026-07-17:IgnoreSlowTorrentsForQueueing

**問題**:`MaxActiveDownloads=10` 的名額被 4 個 `seeds=0` 的死種子佔住(狀態 `stalledDL`)。
它們永遠不會有進度,但也永遠佔著名額,導致後面 `queuedDL` 的新集數**無限期排隊**,
即使那些新集數其實有 seeder 可下載。

**修正**:開啟 `IgnoreSlowTorrentsForQueueing`,並將 `SlowTorrentsInactivityTimer` 設為 1200 秒。
種子連續 20 分鐘低於 `slow_torrent_dl_rate_threshold`(2 KB/s)才不計入 active 名額。

選 20 分鐘而非預設 60 秒,是為了不誤踢「慢但仍在傳輸」的種子;死種子則會在 20 分鐘後讓出名額。

**效果**:套用後 active 名額由 10/10 立即降至 4/10,排隊中的集數隨即開始下載並全數完成。

## 修改生產設定的流程

透過 WebUI 或 API 調整生產設定後,**必須同步回本目錄**,否則:

- 容器重建 / 換機時設定遺失
- 沒有版本控制與變更理由的紀錄
- dev 與 prod 行為不一致

```bash
# 1. 從生產取出實際 conf(注意:含 WebUI 密碼,勿直接貼入 repo)
docker exec bangumi-qbittorrent cat /config/qBittorrent/qBittorrent.conf

# 2. 將**關鍵設定**同步到 prod.conf(及 dev.conf 以保持一致),
#    並在本檔案記錄變更理由
```

密碼(`WebUI\Password_PBKDF2`)、runtime 痕跡(`Network\Cookies`)、
以及由環境變數決定的埠號(`WebUI\Port`、`Session\Port`)**不需要**同步回 `prod.conf`。
