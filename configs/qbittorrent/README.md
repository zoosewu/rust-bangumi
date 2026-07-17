# qBittorrent 設定

## 兩個檔案的角色

| 檔案 | 用途 | 是否被掛載 |
|------|------|-----------|
| `dev.conf` | 開發環境 | **是** — `docker-compose.dev.yaml` 直接掛載為容器內的 `/config/qBittorrent/qBittorrent.conf` |
| `prod.conf` | 生產環境的**初始範本** | **否** — 見下方說明 |

## 為什麼 prod.conf 沒有被掛載

生產的 compose 掛載的是**目錄**(`${QBITTORRENT_CONFIG_DIR}` → `/config`,預設 `./data/qbittorrent`),
qBittorrent 在裡面自行維護 `qBittorrent.conf`。原因是這個檔案是 **runtime state**:
WebUI 改設定、Core 透過 API 改設定,qBittorrent 都會寫回這個檔案。唯讀掛載會讓 WebUI 無法存檔。

**代價是它會與 repo drift。** `prod.conf` 的定位因此是:

1. 新環境部署時的起點(複製到 `$QBITTORRENT_CONFIG_DIR/qBittorrent/qBittorrent.conf`)
2. **關鍵設定的期望值文件** — 生產若被手動調整,必須同步回這裡

`data/` 未納入版控,所以生產的實際 conf 不在 git 裡。

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
