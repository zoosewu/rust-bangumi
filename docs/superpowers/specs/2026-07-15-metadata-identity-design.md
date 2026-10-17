# 作品身分（identity）確定性化設計：external id 取代 AI 猜測

**日期**: 2026-07-15（初版）／2026-10-18（修訂）
**狀態**: 已與使用者確認（2026-10-18），實作計畫見 `docs/superpowers/plans/2026-10-18-metadata-identity-v2.md`

> **2026-10-18 修訂摘要**
> - 重新實測產品環境：問題除了「撈錯作品」，還有**完全沒有產出任何 metadata**（媒體庫 0 個 NFO、0 張海報）。
> - 新增：系列名稱沿 bgm 前傳鏈取第一季中文名，各季統一；viewer 產出 `tvshow.nfo` / `season.nfo` / 海報；
>   媒體庫依新名稱整理並合併重複作品；合集集數都已下載時自動略過。
> - 移除：待認領佇列（目前 18/18 訂閱都拿得到 bgm id，沒有 id 時沿用現行做法並寫 log）；
>   以及任何 AI 參與命名的設計。

## 背景

回報症狀是「synopsis（簡介）沒有抓到」以及「metadata 撈錯作品」。

### 因果鏈（2026-10-18 實測）

synopsis 出不來有四層原因，前兩層是「完全沒有」，後兩層是「抓錯或沒存」：

| 層 | 問題 | 證據 |
|---|---|---|
| 1 | core 每次同步都送 `bangumi_id: None`、`cover_image_url: None`，viewer 收到後直接跳過 metadata | `core-service/src/services/sync_service.rs`（`build_sync_request`）、`viewers/jellyfin/src/handlers.rs`（`fetch_and_generate_metadata`） |
| 2 | viewer 只會寫單集 NFO，**不會寫作品層級的 `tvshow.nfo`**，作品簡介無處可放 | `viewers/jellyfin/src/nfo_generator.rs` 只有 `generate_episode_nfo` |
| 3 | core 向 metadata 查詢時回應裡有簡介與開播日，但只存了封面 | `handlers/anime.rs` `fetch_and_store_covers`；`animes` 23 筆的 description / aired_date 全為 NULL |
| 4 | bgm id 從未存下；每次都拿 AI 取的作品名去 bgm.tv 模糊搜尋、直接取第一筆 | `metadata/src/bangumi_client.rs` `search_anime` 取 `list[0]` |

第 4 層的上游是 AI 生成的 parser 用 `anime_title_source = 'static'` 硬編作品名：

```
AI 生成 parser（硬編作品名）
  → 作品名是 AI 憑單一 RSS 標題發明的字串
    → 同一部作品的單集 parser 與合集 parser 給出不同名字 → 拆成兩個作品
    → metadata 拿這個字串去 bgm.tv 模糊搜尋取第一筆 → 撈錯 / 撈不到
```

### 實測災情（2026-10-18，產品環境唯讀）

| 指標 | 數值 |
|------|------|
| anime_works | 60（其中 **39 個無法從任何訂閱到達**，37 個連季都沒有） |
| animes | 23，description / aired_date **全為 NULL** |
| anime_cover_images | 11（18 個訂閱中只有 9 個作品有封面） |
| 媒體庫 | 18 個資料夾，**0 個 NFO、0 張海報** |
| 同一訂閱被拆成兩個作品 | 4 個訂閱：欺詐遊戲、超市後門、暴怒千金、轉校生 |
| 重複下載 | 超市後門、暴怒千金、轉校生各有兩組 1–12 集，**逐位元組相同**（LoliHouse 合集就是單集重新打包），重複佔約 19 GB |
| seasons | 只有 3 筆（2025/unknown、2026/unknown、2025/3），季度資訊無意義 |

AI 發明的作品名與 bgm.tv 權威名稱對照：

| bgm id | 目前作品名 | bgm.tv 中文名 | 診斷 |
|---|---|---|---|
| 456080 | `てんびん` + `转校生美少女竟是…`（兩個作品） | 转学后班上的清纯可爱美少女，竟是小时候玩在一起的哥们儿 | 名稱錯誤 + 拆成兩個 |
| 548818 | `金牌得主` | 金牌得主 第二季 | 季別遺失 |
| 580133 | `欺诈游戏 / 诈欺游戏 / LIAR GAME` + `欺诈游戏` | 欺诈游戏 | 別名污染 + 拆成兩個 |
| 568244 | `药屋少女的呢喃` | 药屋少女的呢喃 第三季 | 季別遺失 |

### 關鍵發現：權威身分一直都在，只是沒被使用

1. **18/18 訂閱的 `source_url` 都帶 mikan id**：`mikanani.me/RSS/Bangumi?bangumiId={id}&subgroupid={g}`。
2. **mikan 作品頁公開 bgm 連結**：`https://mikanani.me/Home/Bangumi/3822` 內含
   `<a href="https://bgm.tv/subject/548818">`。2026-10-18 抽查 4/4 成功。
3. **bgm API 有完整內容**：`/v0/subjects/548818` → 中文名「金牌得主 第二季」、開播 2026-01-24、400 字簡介、9 集。
4. **bgm 有確定的季別關聯**：`/v0/subjects/{id}/subjects` 標出「前传 / 续集」，可沿前傳追到第一季：
   `药屋少女的呢喃 第三季 → 第二季 → 药屋少女的呢喃`；`金牌得主 第二季 → 金牌得主`。

## 設計

### 1. 核心原則：身分、內容、檔案屬性分離

| 面向 | 來源 | 決定者 |
|------|------|--------|
| **身分**（哪部作品的哪一季） | fetcher 提供的 external id（`bgm/548818`） | 確定性推導，**AI 永不參與** |
| **內容**（名稱 / 簡介 / 封面 / 日期 / 單集資訊） | metadata service 依 external id 查詢 | bgm.tv 權威資料 |
| **檔案屬性**（第幾集 / 第幾季 / 字幕組 / 畫質） | title parser | regex |

parser 解析出的作品名只在「沒有可用 external id」時使用（現行做法，並寫 log）。

### 2. 掛載層級：bgm id 屬於「季」，系列名稱來自第一季

`anime_works` 是**系列**，`animes` 是**季**。bgm subject 是季別層級（548818 = 金牌得主 第二季）。

```
anime_works (系列)                     series identity = bgm/430699（第一季）
  16 | 金牌得主                         title_source = 'bgm'

animes (季)
  16 | work 16 | series_no=2 | title「金牌得主 第二季」| aired 2026-01-24
     └─ anime_external_ids: bgm/548818
```

- **季的身分**：`anime_external_ids (namespace, external_id)` UNIQUE，同一個 bgm id 不可能長出兩季。
- **系列的身分**：沿 bgm 前傳鏈（只走 TV / WEB 動畫，跳過劇場版、番外、衍生）追到第一季，
  以第一季的 bgm id 作為系列身分（`anime_works.series_namespace / series_external_id`，UNIQUE）。
  同系列的每一季因此掛到同一個作品下。
- **季數（series_no）**：維持由 parser 從標題解析（使用者決定，2026-10-18）。

### 3. 名稱規則（使用者決定，2026-10-18）

只要求「中文、各季統一」，不要求繁體，**不使用 AI、不做簡繁轉換**。

**系列名稱**（資料夾、`tvshow.nfo`、前端作品名）依序取：

1. 手動設定的名稱（`title_source = 'manual'`，之後永不被自動覆寫）
2. 第一季的 bgm 中文名（`name_cn`）
3. 其他季的 `name_cn` 去掉季別後綴（`第X季`、`第X期`、`Season N`、`Nth Season`、`第X部分`）
4. 第一季的原名（`name`，通常是日文）

**季名稱**：該季的 `name_cn`，沒有則 `name`；只用在季層級顯示與 `season.nfo`。
原名與中文名都保存，NFO 中以 `originaltitle` 提供給 Jellyfin 搜尋。

### 4. 資料模型

```sql
-- 新表：季 → 外部身分
anime_external_ids
  external_ref_id | anime_id (FK animes ON DELETE CASCADE) | namespace | external_id | source | created_at
  UNIQUE (namespace, external_id)
  UNIQUE (anime_id, namespace)
  source: 'fetcher' | 'manual'      -- 人工修正永遠蓋過自動

-- 新表：metadata service 認領的 namespace
metadata_namespaces
  module_id (FK service_modules ON DELETE CASCADE) | namespace | priority
  UNIQUE (module_id, namespace)

-- 既有表變更
anime_works         加 series_namespace TEXT NULL, series_external_id TEXT NULL
                       UNIQUE (series_namespace, series_external_id)
                    加 title_source TEXT NOT NULL DEFAULT 'parser'   -- 'parser' | 'bgm' | 'manual'
                    加 is_active BOOL NOT NULL DEFAULT true, soft_deleted_at TIMESTAMP NULL
                    加 description TEXT NULL, premiered DATE NULL      -- 系列簡介與首播日（取自第一季）
animes              加 title TEXT NULL, original_title TEXT NULL     -- 季名（bgm name_cn / name）
                    既有 description / aired_date / end_date 開始會被填；season_id 依 aired_date 推出年份與季度
anime_cover_images  加 anime_id INT NULL (FK animes)                 -- 有值：季的封面；NULL：系列海報（第一季封面）
raw_anime_items     加 external_ids TEXT[] NOT NULL DEFAULT '{}'      -- 讓 reparse 也能用身分
```

### 5. Fetcher 契約

`RawAnimeItem` 增加 `external_ids: Vec<ExternalId>`（`#[serde(default)]`），線上格式 `"{namespace}/{id}"`：

```json
{ "title": "[绿茶字幕组] 金牌得主 第二季 [22][1080p]",
  "external_ids": ["mikan/3822", "bgm/548818"] }
```

mikanani：訂閱 URL 的 `bangumiId` → 作品頁抽 `bgm\.tv/subject/(\d+)`，以 mikan id 為鍵快取。
混合型 feed（`RSS/MyBangumi`）目前沒有訂閱使用，本期不支援（item 不帶 id → 走現行做法）。

fetcher 另提供 `POST /identity/resolve { source_url }` → `external_ids`，供回填使用。

### 6. Metadata service 契約

```
註冊：  POST /register  capabilities.namespaces = ["bgm"]
查詢：  POST /enrich/anime     { namespace, external_id }
          → { title, title_cn, summary, air_date, end_date, total_episodes, cover_images,
              series: { namespace, external_id, title, title_cn, summary, air_date, cover_images } }
                                                                   -- 沿前傳鏈找到的第一季（即使沒訂閱第一季也拿得到）
        POST /enrich/episodes  { namespace, external_id, episode_no }
```

**`search_anime` 取 `list[0]` 的路徑刪除。** 不提供模糊搜尋（待認領佇列已移除）。

### 7. Core 解析流程

```
raw item → external_ids（存到 raw_anime_items.external_ids）
   → 濾出已註冊 namespace 的 id（metadata_namespaces）
   → 查 anime_external_ids
        命中   → 掛到該季
        未命中 → /enrich/anime 取權威資料
                 → 依 series 身分找系列（anime_works.series_*），沒有就建立（名稱依第 3 節）
                 → 在該系列下建立季（series_no 來自 parser，season_id 來自 aired_date）
                 → 寫 anime_external_ids (source='fetcher')
   → 沒有可用 id → 現行做法（parser 作品名）+ warn log
```

`(work_id, series_no)` 已被另一個 bgm id 佔用時（parser 季數與 bgm 不一致），不覆寫既有綁定：
該 raw item 標為 `failed` 並寫明原因，留給人工修正 parser。

### 8. 同集多來源：合集自動略過（使用者決定，2026-10-18）

身分統一後，同一字幕組的單集版與合集版會掛到同一季，形成衝突（anime, group, episode）。規則：

- 一個合集 torrent 涵蓋的**每一集**，在同一季都已有其他 link 的有效下載（狀態不是 failed / cancelled），
  → 該合集的所有 link 自動標為略過，不派送、不列為待處理衝突。
- 只覆蓋部分集數時，照現行衝突流程由使用者決定。

### 9. Viewer 輸出

同步與 resync 請求都帶上身分與系列 / 季的內容，viewer 產出 Jellyfin 標準結構：

```
金牌得主/                         ← 系列名稱（第 3 節）
  tvshow.nfo                      ← title / originaltitle / plot / premiered（系列層級，取自第一季）
  poster.jpg                      ← 系列海報（第一季封面）
  Season 02/
    season.nfo                    ← 季名 / plot / premiered
    poster.jpg                    ← 該季封面
    金牌得主 - S02E01.nfo          ← 單集名 / 簡介 / 播出日（/enrich/episodes）
    金牌得主 - S02E01.mkv
```

resync 時一律重寫 NFO，搬移後清掉變空的舊資料夾。

### 10. 遷移與回填

產品環境由維運者手動執行，預設 dry-run：

```
bangumi-cli backfill-identity [--apply]      # 呼叫 core POST /admin/identity-backfill
```

1. 每個訂閱：`source_url` → fetcher `/identity/resolve` → bgm id
2. `/enrich/anime` 取權威資料與系列身分
3. 該訂閱可到達的季：寫入身分、季名、簡介、日期、season_id；系列改名（`title_source='manual'` 的不動）
4. 同 bgm id 的重複季合併：link 搬到保留的季；同一集兩份已同步下載時保留合集版，另一份標為 cancelled 並註記
5. 同系列身分的作品合併
6. **無法從任何訂閱到達**的作品 → soft delete（不硬刪）
7. 回填 `raw_anime_items.external_ids`

之後以 `bangumi-cli resync-all [--apply]` 讓 viewer 依新名稱搬移檔案並產出 NFO / 海報；
重複檔案列出清單、經使用者確認後才由 viewer 刪除。

## 測試策略

- **Unit**（`shared`）：`ExternalId` 解析 / 格式化（含畸形輸入）。
- **Unit**（`fetchers/mikanani`）：以真實 HTML fixture 測作品頁抽 bgm 連結、RSS URL 抽 mikan id。
- **Unit**（`metadata`）：前傳鏈追溯（跳過劇場版、深度上限、循環保護）、季別後綴去除、名稱優先順序。
- **Unit**（`core-service`）：namespace 過濾、合集自動略過規則、回填計畫（重複季合併、soft delete 判定）。
- **Integration**（`core-service`，dev DB）：
  - 帶 `bgm/X` → 建立季與身分；第二筆同 `bgm/X` → 掛到同一季
  - 同系列不同季 → 同一個作品
  - 沒有可用 id → 現行做法，不寫身分
  - `source='manual'` / `title_source='manual'` 不被覆寫
- **Viewer**：`tvshow.nfo` / `season.nfo` 內容、resync 後舊資料夾清理（tempdir）。

## 決策記錄

| 決策 | 選擇 | 理由 |
|------|------|------|
| 身分結構 | 一組 namespaced external id | 換 metadata 站時舊資料不用重洗 |
| 身分錨點 | external id，parser 不再決定作品名 | 根治重複作品與 AI 亂命名 |
| 掛載層級 | 季（animes）；系列身分取第一季 | bgm subject 是季別層級；前傳鏈讓各季名稱統一 |
| 名稱 | 中文即可、各季統一；手動 > 第一季中文名 > 去後綴 > 原名 | 2026-10-18 使用者決定；不需要 AI 與簡繁轉換 |
| 季數 | parser 解析 | 2026-10-18 使用者決定 |
| 沒有 bgm id | 沿用現行做法 + log，不做待認領佇列 | 18/18 訂閱可解析；YAGNI |
| 重複集數 | 保留合集版（內容與單集版相同） | 2026-10-18 使用者決定 |
| 合集已全部下載過 | 自動略過 | 2026-10-18 使用者決定 |
| 殘骸作品 | soft delete，執行前看報表 | 可逆 |

## 未納入範圍（YAGNI）

- 待認領佇列與模糊搜尋候選（需要時再加）。
- 混合型 feed（`RSS/MyBangumi`）的身分解析。
- 多個 metadata service 認領同一 namespace 的仲裁（`metadata_namespaces.priority` 預留）。
- 既有重複 title_parsers 的清理（身分統一後重複 parser 不再造成重複作品）。
