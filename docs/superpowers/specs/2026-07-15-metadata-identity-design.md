# 作品身分（identity）確定性化設計：external id 取代 AI 猜測

**日期**: 2026-07-15
**狀態**: 待審

## 背景

回報症狀是「metadata 沒有撈取正確或是撈錯作品」。對產品環境（`yeh3f@192.168.68.101`，
唯讀查詢）與上游站台的實測顯示，metadata 撈錯只是表徵，根因在其上游兩層。

### 因果鏈

```
AI 生成 parser（anime_title_source = 'static'，硬編碼作品名）
  → work title 是 AI 憑單一 RSS 標題發明的字串
    → metadata service 拿該字串去 bgm.tv 模糊搜尋
      → bangumi_client.rs:45 取 list[0].id，零驗證
        → 撈錯作品 / 撈不到
```

### 實測災情（2026-07-15，產品環境）

| 指標 | 數值 |
|------|------|
| anime_works | 51（其中 **39 個無法從任何訂閱到達**，皆為解析殘骸） |
| animes | 14 |
| anime_cover_images | 7 |
| raw_anime_items | 99 failed / 97 parsed / 8 no_match（**約半數解析失敗**） |
| title_parsers | 18（其中 **7 個在解同一部《欺诈游戏》**，給出 3 種不同 title） |
| animes.aired_date | **全部 NULL** |
| animes.season_id | 全部指向 season 1（2025/unknown），但這些是 2026 年番 |

AI 發明的 title 與 bgm.tv 權威名稱對照（12/12 全數實測）：

| bgm id | AI 發明的 title | bgm.tv 權威名稱 | 診斷 |
|---|---|---|---|
| 456080 | `てんびん` | 转学后班上的清纯可爱美少女，竟是小时候玩在一起的哥儿们 | **完全撈錯** |
| 548818 | `金牌得主` | 金牌得主 第二季 | 季別遺失 |
| 172494 | `魔法科高中的劣等生:呼唤繁星的少女` | 剧场版 魔法科高校的劣等生 呼唤星辰的少女 | 劇場版標記遺失 |
| 580133 | `欺诈游戏 / 诈欺游戏 / LIAR GAME` | 欺诈游戏 | 別名污染 → 衍生 7 個重複 parser |
| 571784 | `Super no Ura de Yani Suu Futari` | 在超市后门吸烟的二人 | 羅馬字未轉譯 |

### 關鍵發現：權威身分一直都在，只是沒被使用

1. **12/12 訂閱都攜帶 mikan id**：`source_url` 全為
   `mikanani.me/RSS/Bangumi?bangumiId={id}&subgroupid={g}`。
2. **每個訂閱精確對應 1 個 work**（`distinct_works` 全為 1），零歧義。
   51 個 work 中 12 個可從訂閱到達，其餘 39 個為殘骸（37 個連 anime 都沒有，
   2 個有 anime 但零 links、零 downloads）。
3. **mikan detail 頁公開 bgm 連結**：`https://mikanani.me/Home/Bangumi/3822` 內含
   `<a href="https://bgm.tv/subject/548818">`。12/12 實測全數解析成功。
4. **Episode 頁可反查**：`/Home/Episode/{hash}` 內含 `/Home/Bangumi/{id}`，
   故混合型 feed（`RSS/MyBangumi`）亦可解析。

即：mikan 維護著一份人工校對過的 mikan→bgm 對應表，而系統一直在用最髒的來源
（RSS 標題）去猜一個訂閱早已知道的答案。**存量修正完全不需要 AI。**

### 附帶病灶

- `bangumi_id` 從未被持久化。`fetch_and_store_covers` 當場用完即丟，
  故無任何機制可以修正或釘住一部作品的對應，每次 enrich 都重賭一次。
- `sync_service.rs:187` 傳 `bangumi_id: None` → resync 路徑的單集 metadata
  在 `viewers/jellyfin/src/handlers.rs:212` 直接 skip，永遠生不出來。

## 設計

### 1. 核心原則：身分與內容分離

| 面向 | 來源 | 決定者 |
|------|------|--------|
| **身分**（哪部作品的哪一季） | fetcher 提供的 external id | 確定性推導，**AI 永不參與** |
| **內容**（名稱/簡介/封面/集資訊/日期） | metadata service 依 external id 查詢 | bgm.tv 權威資料 |
| **檔案屬性**（第幾集/字幕組/畫質） | title parser | regex，可驗證 |

`anime_title_source = 'static'` 用法廢止——它正是「てんびん」的來源。
parser 職責收窄為 `episode_no` / `subtitle_group` / `resolution`。

### 2. 掛載層級：bgm id 屬於「季」

實測確認既有模型語意正確：`anime_works` 是**系列**（work 16「金牌得主」），
`animes` 是**季**（`series_no = 2`）。而 bgm subject 是**季別層級**的
（548818 = 金牌得主 第二季，S1 為另一 subject）。

故 external id 掛在 `animes`，而非 `anime_works`：

```
anime_works (系列)
  16 | 金牌得主

animes (季)
  16 | work 16 | series_no=2 | aired 2026-01-24
     └─ bgm/548818 「金牌得主 第二季」

未來訂閱 S1：
  99 | work 16 | series_no=1 | aired 2025-01-05
     └─ bgm/389156 「金牌得主」
  → 同系列兩季，各自正確的封面與簡介
```

### 3. 資料模型

```sql
-- 新表：季 → 外部身分
anime_external_ids
  external_ref_id | anime_id (FK animes) | namespace | external_id | source | created_at
  UNIQUE (namespace, external_id)   -- 樞紐：同一 bgm id 不可能長出兩季
  UNIQUE (anime_id, namespace)      -- 一季在一個 namespace 只有一個 id
  source: 'fetcher' | 'manual'      -- 人工修正永遠蓋過自動

-- 新表：metadata service 認領的 namespace
metadata_namespaces
  module_id (FK service_modules) | namespace | priority
  UNIQUE (module_id, namespace)

-- 新表：待認領佇列（fallback）
pending_identities
  id | raw_item_id | subscription_id | source_title | status
     | resolved_namespace | resolved_external_id | created_at
  status: 'pending' | 'resolved' | 'skipped'

-- 既有表變更
animes              加 title VARCHAR(255) NULL（季名，來自 bgm）
                    既有 description / aired_date / end_date 終於會被填
anime_works         加 is_active BOOL NOT NULL DEFAULT true
                    加 soft_deleted_at TIMESTAMP NULL
anime_cover_images  work_id → anime_id（封面本來就是季別的）
```

`UNIQUE (namespace, external_id)` 是整個設計的樞紐：它在**資料庫層**讓
「欺诈游戏 / LIAR GAME / 欺诈游戏 三個 work」變成不可能發生，而非靠應用層自律。

### 4. Fetcher 契約

`RawAnimeItem` 增加 `external_ids: Vec<String>`，格式 `"{namespace}/{id}"`：

```json
{ "title": "[绿茶字幕组] 金牌得主 第二季 [22][1080p]",
  "external_ids": ["mikan/3822", "bgm/548818"] }
```

fetcher 回報**它所知道的全部身分**，不預設誰會用。mikanani 取得路徑（皆已實測）：

- 訂閱 URL 有 `bangumiId` → detail 頁抽 `bgm\.tv/subject/(\d+)`
- 混合型 feed → Episode 頁 → `/Home/Bangumi/{id}` → detail 頁
- 以 mikan id 為鍵快取，避免每個 item 重打兩次 HTTP

未來換 metadata 站時，只要新 fetcher 也吐得出對應 namespace（如 `tmdb/`）即可接上，
舊資料的 `bgm/` 記錄不受影響、不需重洗。

### 5. Metadata service 契約

```
註冊：  POST /register  { module_type: "metadata", namespaces: ["bgm"] }
查詢：  POST /enrich/anime     { namespace: "bgm", external_id: "548818" }
          → { title, title_cn, summary, air_date, end_date, cover_images }
        POST /enrich/episodes  { namespace, external_id, episode_no }
候選：  GET  /search/candidates?q=...   → 只建議，永不寫入
```

**`search_anime` 取 `list[0]` 的路徑刪除**（`metadata/src/bangumi_client.rs:34-47`）。
模糊搜尋降級為只服務 fallback UI 的建議來源。

### 6. Core 解析流程

```
raw item → external_ids
   → 濾出已註冊 namespace 的 id（查 metadata_namespaces）
   → lookup anime_external_ids by (namespace, external_id)
        命中   → 掛到該 anime
        未命中 → enrich 取權威資料 → 建 anime(+work) → 寫 external id (source='fetcher')
   → 一個可用 id 都沒有 → 寫 pending_identities（不建 work、不抓 metadata）
```

連帶修正 `sync_service.rs:187`：改為查 `anime_external_ids` 帶出真實 id，
resync 路徑的單集 metadata 得以生成。

### 7. Fallback：待認領佇列（零 AI）

身分解析失敗時（mikan detail 無 bgm 連結、或未來的非 mikan fetcher），
item 進待認領佇列。UI 呈現 `/search/candidates` 的 Top-5 建議（名稱/年份/封面），
或允許直接貼上 bgm 網址。**搜尋只能建議，永遠不能自己寫入；人是唯一的決定者。**

依實測 12/12 的解析率，此路徑應為罕見例外而非常態。

### 8. 遷移與回填

產品環境唯讀，故回填以 CLI 提供，由維運者自行執行：

```
bangumi-cli backfill-identity [--dry-run | --apply]
```

預設 `--dry-run` 印出完整報表，核對後才 `--apply`：

```
BACKFILL  12 works <- 12 subscriptions
  work 16  金牌得主   -> bgm/548818  改為「金牌得主 第二季」
  work 50  てんびん     -> bgm/456080  改為「转学后班上的...」 [修正錯誤]
  work 46  魔法科...   -> bgm/172494  改為「剧场版 魔法科...」
  ...
SOFT-DELETE 39 works（無法從任何訂閱到達的解析殘骸）
  work 1-15   [绿茶字幕组] 金牌得主...      (37 個無 anime)
  work 17-37  [桜都字幕组] 我推的孩子...
  work 40     动漫国字幕组
  work 38/39  我推的孩子 / Oshi no Ko       (2 個有 anime，訂閱已刪)
```

步驟：

1. 讀 `subscriptions.source_url` 抽 mikan id（12/12 可得）
2. 經 fetcher 解析 → bgm id
3. 呼叫 metadata enrich → 權威 title / air_date / end_date / covers
4. 更新 `animes.title` / `aired_date` / `season_id`、`anime_works.title`
5. 同 bgm id 的 anime 合併（UNIQUE 約束保證收斂）
6. **無法從任何訂閱到達**的 work → soft delete（**不硬刪**，跡到底可逆）

判定條件為「訂閱可達性」而非「有無 anime」：work 38/39 有 anime 但訂閱已被刪除，
若以後者為條件會漏掉。實測 39 個待刪 work 底下**皆為零 links、零 downloads**，
故 soft delete 不影響任何下載記錄；`--apply` 前仍須以報表核對此前提是否仍成立。

## 測試策略

- **Unit**（`fetchers/mikanani`）：以真實 HTML fixture 測 detail 頁抽 bgm 連結；
  沿用既有 `REAL_BANGUMI_DETAIL_HTML` 慣例，並註明「mikan 改版時需更新」。
- **Unit**（`shared`）：`external_id` 的 `"{ns}/{id}"` 解析與格式化（含畸形輸入）。
- **Unit**（`core-service`）：namespace 過濾邏輯——未註冊的 namespace 必須被忽略。
- **Integration**（`core-service`）：
  - raw item 帶 `bgm/X` → 建立 anime + external id（MockMetadataClient）
  - 第二個 raw item 帶同一 `bgm/X` → 掛到同一 anime，**不新建**
  - raw item 無可用 id → 進 `pending_identities`，且不建 work
  - `source='manual'` 的 id 不被 fetcher 覆寫
- **DB 約束**：`UNIQUE (namespace, external_id)` 衝突時的行為需有明確測試。
- **回填**：以 12 筆 prod-like fixture 跑 `--dry-run`，斷言報表內容。

## 決策記錄

| 決策 | 選擇 | 理由 |
|------|------|------|
| 身分結構 | 一組 external id（新對應表） | 未來換 metadata 站時舊資料不用重洗 |
| 身分錨點 | external id 為準，parser 不再管作品名 | 根治重複 work 與 AI 亂命名 |
| 掛載層級 | `animes`（季） | bgm subject 是季別層級；保住系列層關聯 |
| Fallback | 搜尋提候選 + 人工確認，零 AI | 12/12 解析率使 AI 無必要；AI 上次的成果是「てんびん」 |
| 存量清理 | 自動回填 + soft delete + 稽核報表 | 跡到底可逆 |

## 未納入範圍（YAGNI）

- **系列層 external id**：bgm.tv 無系列層 id，目前無實際需求。
- **多 metadata service 同 namespace 競合**：`metadata_namespaces.priority` 欄位預留，
  但本期不實作仲裁邏輯。
- **既有 18 個 title_parsers 的清理**：新設計下重複 parser 不再造成重複 work
  （身分由 external id 決定），故降為無害冗餘，不在本期處理。
