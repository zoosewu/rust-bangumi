# 作品身分與 metadata 實作計畫（v2）

> 取代 `2026-07-16-metadata-identity.md`。舊計畫 Task 1–4 的程式碼片段仍可參考（`ExternalId`、mikan 作品頁解析），
> 但行號與周邊程式已過時，實作時一律以現行程式為準。

**Spec:** `docs/superpowers/specs/2026-07-15-metadata-identity-design.md`（2026-10-18 修訂版）

**Goal:** 作品身分來自 fetcher 提供的 bgm id；系列名稱沿前傳鏈取第一季中文名、各季統一；
簡介 / 日期 / 封面由 bgm 提供並真正寫進 Jellyfin（`tvshow.nfo`、`season.nfo`、單集 NFO、海報）；
回填產品環境既有資料並整理媒體庫。

**Tech Stack:** Rust / Axum / Diesel 2.1 + PostgreSQL 15 / reqwest / scraper

## 全域約束

- 每個 task：先寫會失敗的測試 → 實作 → `cargo fmt` / `cargo clippy`（不新增警告）/ `cargo test` → commit。
- 一次解決一個問題：每個階段可以單獨部署、單獨驗證，階段之間由使用者確認後才進下一段。
- Diesel migration 置於 `core-service/migrations/YYYY-MM-DD-HHMMSS-<name>/`，`down.sql` 必須可逆；
  `schema.rs` 依 migration 同步更新，並在 dev DB 上跑 up / down / up 驗證。
- 新增 payload 欄位一律 `#[serde(default)]`，舊服務送來的請求仍可解析。
- 產品環境：資料寫入、刪檔、重啟服務都只提供指令給使用者執行；回填與刪檔預設 dry-run。
- 新增或改動 API 時更新 `docs/api/openapi.yaml`。
- commit 訊息說明「為什麼」，結尾 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`。

## 階段總覽

| 階段 | 內容 | 部署後的可驗證結果 |
|---|---|---|
| **A. 身分** | A1–A10 | 新進項目掛到 bgm id 決定的季；回填後 18 個訂閱對到正確作品、重複作品合併、季有簡介與日期 |
| **B. Viewer 輸出** | B1–B4 | 新同步的集數有 `tvshow.nfo` / `season.nfo` / 單集 NFO / 海報 |
| **C. 媒體庫整理** | C1–C3 | 既有媒體庫依新名稱整理、補齊 NFO 與海報、重複檔案刪除，Jellyfin 每部作品只出現一次 |

```
A1 ─┬─ A2 (fetcher)
    └─ A3 (migration) ─┬─ A4 (namespace 註冊)
                       ├─ A5 (metadata service)
                       ├─ A6 (identity 服務) ── A7 (攝入接上) ── A8 (合集自動略過)
                       └─ A9 (手動改系列名稱)
A2 + A5 + A7 ── A10 (回填)
A ── B1 ─┬─ B2 (core 帶內容) ─┐
         └─ B3 (viewer NFO) ──┴─ B4 (封面改掛季)
B ── C1 (清空資料夾) ── C2 (resync-all) ── C3 (重複檔案刪除)
```

---

## 階段 A：身分

### A1. shared — `ExternalId` 與 payload 欄位

**Files:** 新增 `shared/src/external_id.rs`；改 `shared/src/lib.rs`、`shared/src/models.rs`（`RawAnimeItem`、`Capabilities`）

- `ExternalId { namespace, id }`：`FromStr`（只在第一個 `/` 切開）、`Display`、serde 以字串 `"bgm/548818"` 序列化。
  程式碼沿用舊計畫 Task 1。
- `RawAnimeItem.external_ids: Vec<ExternalId>`（`#[serde(default)]`）
- `Capabilities.namespaces: Vec<String>`（`#[serde(default)]`）

**測試**（舊計畫 Task 1 的 7 個）：解析、round-trip、id 內含 `/`、缺分隔符、空 namespace / id、序列化為字串、畸形字串反序列化失敗；
另加：不帶 `external_ids` 的舊 JSON 仍可反序列化。

### A2. mikanani — 回報 external ids

**Files:** 改 `fetchers/mikanani/src/detail_scraper.rs`、`fetch_task.rs`；新增 `identity_resolver.rs`；`handlers.rs`、`main.rs`（新路由）

- `pub fn mikan_id_from_rss_url(url: &str) -> Option<String>`：抽 `bangumiId`
- `pub fn bgm_subject_id_from_detail(html: &str) -> Option<String>`：抽 `bgm\.tv/subject/(\d+)`
- `IdentityResolver`：`resolve(mikan_id) -> Vec<ExternalId>`（`mikan/x` + `bgm/y`），以 mikan id 為鍵快取；
  作品頁抓取失敗時只回 `mikan/x` 並 warn，不中斷 fetch
- `fetch_task`：同一個 feed 的每個 item 掛上同一組 external ids
- `POST /identity/resolve { source_url } → { external_ids }`（回填用）

**測試：** 以 `https://mikanani.me/Home/Bangumi/3822` 的真實 HTML 存成 fixture（註明 mikan 改版需更新）抽出 `548818`；
沒有 bgm 連結的 HTML 回 `None`；RSS URL 帶 / 不帶 `bangumiId`；resolver 第二次查同 id 不再打 HTTP（mock client 計次）。

### A3. core — migration

**Files:** 新增 `core-service/migrations/2026-10-18-000000-metadata-identity/{up,down}.sql`；改 `schema.rs`、`models/db.rs`

up.sql 內容依 spec 第 4 節：

- `anime_external_ids`、`metadata_namespaces` 兩張新表（含 UNIQUE 與 FK）
- `anime_works`：`series_namespace`、`series_external_id`（UNIQUE 組合）、`title_source`（CHECK 三值，預設 `'parser'`）、
  `is_active`（預設 true）、`soft_deleted_at`、`description`、`premiered`
- `animes`：`title`、`original_title`
- `anime_cover_images`：`anime_id`（FK animes，NULL）
- `raw_anime_items`：`external_ids TEXT[] NOT NULL DEFAULT '{}'`

**驗證：** dev DB 上 up → down → up；`cargo test` 全過。讀取作品的既有查詢要加上 `is_active = true`（列出作品、訂閱頁的作品清單）。

### A4. core — 落地 metadata namespace 註冊

**Files:** 改 `core-service/src/handlers/services.rs`（`register`）；`metadata/src/main.rs`（註冊時帶 `namespaces: ["bgm"]`）

- metadata 模組註冊時，以 `capabilities.namespaces` 取代該 module 在 `metadata_namespaces` 的列（同一 transaction）
- `pub fn registered_namespaces(conn) -> HashSet<String>`（只算 enabled 模組）

**測試：** 重複註冊不產生重複列；改宣告後舊 namespace 被移除；disabled 模組的 namespace 不算。

### A5. metadata service — 依 external id 查詢、前傳鏈、刪除盲選

**Files:** 改 `metadata/src/bangumi_client.rs`、`handlers.rs`、`models.rs`

- 刪除 `search_anime`（`list[0]`）。
- `get_subject(id)`：一次 `/v0/subjects/{id}` 取 `name`、`name_cn`、`summary`、`date`、`eps`、`platform`、`images`
  （取代原本三個各打一次的方法）
- `find_series_root(id)`：反覆查 `/v0/subjects/{id}/subjects`，取 `relation == "前传"` 且 `type == 2` 的 subject，
  其 `platform` 為 TV / WEB 才繼續；深度上限 10、已走過的 id 不再走（防循環）；回傳第一季 subject
- `series_title(root, seasons)`：spec 第 3 節第 2–4 條的規則（第一季 `name_cn` → 其他季去季別後綴 → 原名）
- `POST /enrich/anime { namespace, external_id }`：namespace 不是 `bgm` → 404；回應見 spec 第 6 節，
  `series` 帶第一季的名稱、簡介、首播日與封面（即使沒訂閱第一季）
- `POST /enrich/episodes { namespace, external_id, episode_no }`

**測試：** 前傳鏈三季追到第一季、遇到劇場版停止、循環保護、深度上限；季別後綴去除（`第三季`、`第2期`、`Season 2`、
`2nd Season`、`第三季 第2部分`）；名稱優先順序；`/enrich/anime` 非 bgm namespace 回 404（mock HTTP）。

### A6. core — identity 服務

**Files:** 新增 `core-service/src/services/identity.rs`、`services/metadata_client.rs`

```rust
pub enum SeasonResolution {
    Resolved(Anime),          // 掛到這一季
    NoIdentity,               // 沒有可用 id → 呼叫端走現行做法
    Conflict(String),         // (work, series_no) 已綁另一個 bgm id → raw item 標 failed
}
pub async fn resolve_season(conn, metadata: &MetadataClient, external_ids: &[ExternalId], series_no: i32)
    -> Result<SeasonResolution, String>;
pub fn find_external_id(conn, anime_id: i32, namespace: &str) -> Result<Option<String>, String>;
```

流程依 spec 第 7 節。建立系列時名稱依 spec 第 3 節，`title_source='bgm'`，並寫入系列簡介與首播日；既有系列是 `manual` 時不改名
（簡介、首播日照常更新）。
`season_id` 由 `aired_date` 推出（年份 + 季度：1–3 月 winter、4–6 spring、7–9 summer、10–12 autumn），沿用 `create_or_get_season`。
並行建立靠 UNIQUE 約束：插入衝突時改為重新查詢。

**測試（integration，dev DB + mock metadata）：**
同 `bgm/X` 兩次 → 同一季；同系列兩季 → 同一作品、不同季；未註冊 namespace → `NoIdentity`；
`(work, series_no)` 已綁別的 id → `Conflict`；`source='manual'` 的身分與 `title_source='manual'` 的名稱不被覆寫。

### A7. core — 攝入與 reparse 接上身分

**Files:** 改 `core-service/src/handlers/fetcher_results.rs`（`receive_raw_fetcher_results`、`process_parsed_result`）、
`services/reparse.rs`、`handlers/raw_items.rs`、`handlers/pending_ai_results.rs`（`rerun_unmatched_raw_items`）

- 攝入時把 `external_ids` 存進 `raw_anime_items.external_ids`
- `process_parsed_result` 前三步（work / season / anime）改為先 `resolve_season`：
  `Resolved` → 用該季；`NoIdentity` → 現行做法 + `warn!`；`Conflict` → raw item 標 `failed`、`error_message` 寫原因
- reparse / rerun 從 `raw_anime_items.external_ids` 讀身分，行為一致

**測試：** 帶身分的 item 不再依 parser 作品名建作品；不帶身分的 item 行為與現在相同；`Conflict` 寫入 failed 與原因。

### A8. core — 合集自動略過

**Files:** 改 `core-service/src/services/conflict_detection.rs`（必要時 `db/repository/anime_link.rs`）

規則見 spec 第 8 節：一個合集 torrent（同一 `raw_item_id` 展開的多個 link）涵蓋的每一集，在同一季都已有其他 link 的
有效下載（狀態不是 failed / cancelled）→ 該合集的 link 全部標為略過（不派送、不列為衝突）；只覆蓋部分集數時照現行流程。
「略過」沿用既有欄位表達（實作時確認以 `filtered_flag` 或衝突解決紀錄表示，並在 commit 說明選擇理由）。

**測試：** 全部已下載 → 合集略過、無衝突；部分已下載 → 照常列衝突；已下載的那份是 failed → 不算已下載。

### A9. core + frontend — 手動改系列名稱

**Files:** `core-service/src/handlers/anime.rs`、`main.rs`（`PUT /anime-works/:work_id { title }`）；前端作品詳細頁加編輯；`openapi.yaml`

- 改名同時設 `title_source='manual'`；之後回填與身分解析都不再改這個名稱
- 已同步的集數不會自動搬資料夾：改名後提示可執行 C2 的 resync

**測試：** handler 單元測試（設定 manual）；identity 對 manual 作品不改名（A6 已涵蓋）。

### A10. 回填 — core admin endpoint + CLI

**Files:** 新增 `core-service/src/services/identity_backfill.rs`、`handlers/admin.rs`；`cli/src/commands/backfill.rs`；`main.rs` 路由

- `POST /admin/identity-backfill { apply: bool } → BackfillReport`；CLI `bangumi-cli backfill-identity [--apply]` 呼叫它
- 計畫（純函式，可單元測試）與執行分開：`plan_backfill(snapshot, resolved) -> BackfillPlan`，`apply` 時在一個 transaction 內執行
- 步驟依 spec 第 10 節 1–7。合併重複季時：link 改掛保留的季；同一集兩份已同步下載時保留合集版，
  另一份 download 標 `cancelled`、`error_message = 'merged duplicate of download N'`（C3 依此刪檔）
- 報表列出：每個訂閱 → bgm id → 系列名稱（舊 → 新）、季名與日期、合併的季與 link 數、將取消的重複下載、soft delete 的作品
  （逐一確認底下 0 link、0 download，否則該作品不刪並列為例外）

**測試：** 以目前產品環境的形狀做 fixture（18 訂閱、4 組重複季、39 殘骸）跑 `plan_backfill`，斷言：
`てんびん` 改為 bgm 中文名、4 組重複季各合併為一、超市後門 / 暴怒千金 / 轉校生各取消 12 筆單集版下載、
欺詐遊戲合併後無下載被取消（work 60 沒有下載）、39 個殘骸 soft delete、`manual` 名稱不變。

**階段 A 部署順序：** core（migration、A4 落地）→ metadata（宣告 bgm）→ fetcher（開始回報）→
使用者執行 `backfill-identity` 看報表 → 確認後 `--apply`。
順序反過來時 core 沒有 namespace 資料，所有 item 走現行做法，不會壞，只是沒有身分。

---

## 階段 B：Viewer 輸出

### B1. shared — 同步請求帶身分與內容

**Files:** 改 `shared/src/models.rs`（`ViewerSyncRequest`、`ViewerResyncRequest`）

- 移除 `bangumi_id`、`cover_image_url`；新增（皆 `#[serde(default)]`）：
  - `external_id: Option<ExternalId>`（季的身分，單集 NFO 用）
  - `series: Option<SeriesMeta { title, original_title, summary, premiered, poster_url }>`
  - `season: Option<SeasonMeta { title, summary, premiered, poster_url }>`
- `ViewerResyncRequest` 同樣加上這三個欄位
- `anime_title` 保留，語意改為「系列名稱」（資料夾名）

core 與 viewer 須同時部署（舊 viewer 會忽略新欄位，只是不產 NFO）。

### B2. core — 組請求時填入內容

**Files:** 改 `core-service/src/services/sync_service.rs`（`build_sync_request`、`build_resync_request`）

- `external_id` 來自 `find_external_id(anime_id, "bgm")`；`series` 取 `anime_works` 的名稱、簡介、首播日與系列海報
  （`anime_cover_images.anime_id IS NULL`）；`season` 取該季的內容與封面
- 沒有身分的季：`external_id = None`、`series` / `season` 只帶名稱（viewer 仍寫 `tvshow.nfo` 的名稱，不寫簡介）

**測試：** 有身分 / 沒身分兩種情況下請求內容正確（純函式從查好的資料組請求）。

### B3. viewer — 產出 `tvshow.nfo`、`season.nfo`、海報、單集 NFO

**Files:** 改 `viewers/jellyfin/src/nfo_generator.rs`、`handlers.rs`（`fetch_and_generate_metadata`、`do_resync`）、`metadata_client.rs`

- `generate_tvshow_nfo(series_dir, &SeriesMeta)`、`generate_season_nfo(season_dir, &SeasonMeta)`：XML 跳脫、每次覆寫
- 海報：系列資料夾 `poster.jpg`、季資料夾 `poster.jpg`；已存在且 resync 時覆寫（封面可能換過）
- 單集 NFO：`metadata_client.enrich_episodes(&ExternalId, episode_no)`；沒有身分時不產（log）
- resync 一律重寫上述檔案

**測試：** NFO 內容（含需要跳脫的字元）、resync 覆寫、沒有身分時只寫名稱（tempdir）。

### B4. core — 封面改掛季、依身分抓取

**Files:** 改 `core-service/src/handlers/anime.rs`（`fetch_and_store_covers`）及其呼叫端

- 簽章改為 `(db, anime_id, ExternalId)`，請求 `/enrich/anime { namespace, external_id }`：
  季封面寫 `anime_id = 該季`；回應 `series.cover_images` 寫成系列海報（`anime_id = NULL`）。`work_id` 都填所屬作品，前端作品封面維持可用
- 新季建立時（A6）與回填時（A10）觸發

**階段 B 部署：** core + metadata + viewer 一起；之後新同步的集數就有完整 metadata。

---

## 階段 C：媒體庫整理

### C1. viewer — 搬移後清掉空資料夾

**Files:** 改 `viewers/jellyfin/src/file_organizer.rs`（`move_episode`）

- 搬移後若舊的季資料夾只剩 NFO / 海報、或已空 → 刪除；系列資料夾同理
- 只刪「我們產生的檔案」（`.nfo`、`poster.jpg`）與空資料夾，遇到其他檔案就保留並 warn

**測試：** 搬走最後一集後舊資料夾被移除；資料夾裡還有其他影片時不刪（tempdir）。

### C2. 全部重新同步 — `resync-all`

**Files:** `core-service/src/handlers/admin.rs`（`POST /admin/resync-all { apply }`）、`cli/src/commands/sync.rs`

- 列出所有 `synced` 的下載，dry-run 顯示「舊路徑 → 新路徑」與將產出的 NFO；`--apply` 時以佇列方式逐筆送 viewer resync
  （一次一筆，避免外接硬碟 I/O 塞住 viewer，對應稽核 E6）
- 跳過 A10 標為 merged duplicate 的下載

**驗證（產品環境）：** 18 個資料夾 → 15 個（3 組重複合併後），每個有 `tvshow.nfo` 與 `poster.jpg`，每集有 NFO。

### C3. 刪除重複檔案

- `bangumi-cli duplicates [--apply]`：列出 A10 取消的重複下載及其媒體庫檔案路徑與大小（預期 36 個檔案、約 19 GB）；
  `--apply` 時呼叫 viewer `/delete`
- 由使用者確認清單後執行

---

## 每個階段結束時

- 更新 `tasks/todo.md` 第 5 項的進度與部署紀錄
- 產品環境驗證（唯讀查詢 + 媒體庫檢查），結果寫進 `tasks/todo.md`
- 使用者確認後才進下一階段
