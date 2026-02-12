# 前端頁面重構 — 實作計畫

## 概要

以 anime_series 為主要頁面，整合篩選規則、解析器、下載資訊到各實體的 dialog 中，移除獨立的 Downloads/Filters/Parsers 頁面。

---

## Phase 1：後端 — 資料庫遷移

### 1.1 TitleParser 新增 created_from 欄位

**檔案**: 新增 migration `YYYY-MM-DD-add-parser-created-from`

```sql
ALTER TABLE title_parsers
  ADD COLUMN created_from_type filter_target_type DEFAULT NULL,
  ADD COLUMN created_from_id INTEGER DEFAULT NULL;
```

**修改檔案**:
- `core-service/src/schema.rs` — diesel print-schema 重新產生
- `core-service/src/models/db.rs` — TitleParser struct 加 `created_from_type`, `created_from_id`
- `core-service/src/dto.rs` — CreateParserRequest / TitleParser response 加對應欄位

### 1.2 確認 filtered_flag 現狀

AnimeLink 已有 `filtered_flag: bool`，但目前建立時永遠設為 `false`。需要：

**修改檔案**:
- `core-service/src/handlers/fetcher_results.rs` — 建立 AnimeLink 時呼叫 FilterEngine 計算 filtered_flag
- `core-service/src/handlers/filters.rs` — 新增/刪除 FilterRule 時批次重算受影響的 AnimeLinks

---

## Phase 2：後端 — 新增/修改 API 端點

### 2.1 GET /anime/series — 豐富表示（主要改動）

**修改檔案**: `core-service/src/handlers/anime.rs`

新增 handler `list_all_anime_series`，回傳：

```json
{
  "series": [
    {
      "series_id": 1,
      "anime_id": 5,
      "anime_title": "葬送的芙莉蓮",
      "series_no": 1,
      "season": { "year": 2023, "season": "fall" },
      "episode_downloaded": 12,
      "episode_found": 28,
      "subscriptions": [
        { "subscription_id": 3, "name": "Mikanani - 葬送" }
      ],
      "description": null,
      "aired_date": "2023-10-01",
      "end_date": null,
      "created_at": "...",
      "updated_at": "..."
    }
  ]
}
```

**SQL 聚合邏輯**:
- `episode_found`: `SELECT COUNT(DISTINCT episode_no) FROM anime_links WHERE series_id = ? AND filtered_flag = false`
- `episode_downloaded`: `SELECT COUNT(DISTINCT al.episode_no) FROM anime_links al INNER JOIN downloads d ON d.link_id = al.link_id WHERE al.series_id = ? AND al.filtered_flag = false AND d.status = 'completed'`
- `anime_title`: JOIN animes 表
- `season`: JOIN seasons 表
- `subscriptions`: JOIN anime_links → raw_anime_items → subscriptions（經由 raw_item_id）

**新增 DTO**: `AnimeSeriesRichResponse` in `core-service/src/dto.rs`

**新增路由**: `GET /anime/series` （注意：與現有 `GET /anime/series/:series_id` 和 `GET /anime/:anime_id/series` 不衝突）

### 2.2 GET /links/:series_id — 加入下載狀態和字幕組名

**修改檔案**: `core-service/src/handlers/links.rs`

回傳格式改為：

```json
{
  "links": [
    {
      "link_id": 1,
      "series_id": 1,
      "group_id": 2,
      "group_name": "SubGroup-A",
      "episode_no": 1,
      "title": "[SubA] Frieren Ep01 1080p",
      "url": "...",
      "source_hash": "...",
      "filtered_flag": false,
      "download": {
        "download_id": 10,
        "status": "completed",
        "progress": 100,
        "torrent_hash": "..."
      },
      "created_at": "..."
    }
  ]
}
```

**新增 DTO**: `AnimeLinkRichResponse` in `core-service/src/dto.rs`

### 2.3 GET /raw-items — 加入下載狀態和過濾狀態

**修改檔案**: `core-service/src/handlers/raw_items.rs`

在每個 RawAnimeItem 回傳中加入：
- `download`: nullable Download 資訊（經由 raw_item_id → anime_links → downloads）
- `filter_passed`: nullable bool（經由 raw_item_id → anime_links.filtered_flag）

### 2.4 POST /filters — 建立後觸發 filtered_flag 重算

**修改檔案**: `core-service/src/handlers/filters.rs`

建立或刪除 FilterRule 後：
1. 根據 target_type + target_id 找出受影響的 AnimeLinks
2. 重新載入所有適用的 filter rules
3. 對每條 AnimeLink 的 title 重新跑 FilterEngine::should_include()
4. 批次 UPDATE filtered_flag

**新增 service**: `core-service/src/services/filter_recalc.rs`

```rust
pub async fn recalculate_filtered_flags(
    pool: &DbPool,
    target_type: FilterTargetType,
    target_id: Option<i32>,
) -> Result<usize, String>
```

### 2.5 GET /parsers — 支援 created_from 查詢

**修改檔案**: `core-service/src/handlers/parsers.rs`

加入 query params：`?created_from_type=anime&created_from_id=5`

### 2.6 GET /anime/:anime_id — 附帶 series 概要

**修改檔案**: `core-service/src/handlers/anime.rs`

修改 `get_anime` 回傳加入 series 列表（復用 AnimeSeriesRichResponse）。

### 2.7 Dashboard 概覽 API

**新增路由**: `GET /dashboard/stats`

```json
{
  "total_anime": 15,
  "total_series": 23,
  "active_subscriptions": 5,
  "total_downloads": 150,
  "downloading": 3,
  "completed": 140,
  "failed": 7,
  "pending_raw_items": 12,
  "pending_conflicts": 2,
  "services": [
    { "name": "mikanani-fetcher", "type": "fetcher", "is_healthy": true },
    { "name": "qbittorrent", "type": "downloader", "is_healthy": true }
  ]
}
```

---

## Phase 3：前端 — 共通 Components

### 3.1 FilterRuleEditor 共通元件

**新增檔案**: `frontend/src/components/shared/FilterRuleEditor.tsx`

Props:
```typescript
interface FilterRuleEditorProps {
  targetType: "global" | "anime" | "anime_series" | "subtitle_group" | "fetcher"
  targetId: number | null
  onRulesChange?: () => void  // 規則變更後的 callback（觸發父元件 refetch）
}
```

功能：
- 顯示當前 target 的 filter rules 列表
- 新增規則：regex input + is_positive toggle + debounce 300ms preview
- 預覽：呼叫 `POST /filters/preview`，以 diff 風格（綠色 +、紅色 -）顯示
- 刪除規則：ConfirmDialog 內顯示移除後的預覽（傳 exclude_filter_id）
- 刪除/新增成功後呼叫 `onRulesChange`

### 3.2 ParserEditor 共通元件

**新增檔案**: `frontend/src/components/shared/ParserEditor.tsx`

Props:
```typescript
interface ParserEditorProps {
  createdFromType: "global" | "anime" | "anime_series" | "subtitle_group" | "subscription"
  createdFromId: number | null
  onParsersChange?: () => void
}
```

功能：
- 列出 `created_from_type` + `created_from_id` 對應的 parsers
- 新增/編輯 parser 表單（condition_regex, parse_regex, 各 source/value 欄位）
- 預覽表格：原始標題 → 解析結果各欄位（anime_title, episode_no, series_no, subtitle_group, resolution）
- 分區：屬於此項目且匹配 / 不屬於此項目但匹配（⚠ 警告）
- 解析失敗欄位以 `—` + 紅色標示

### 3.3 FullScreenDialog 元件

**新增檔案**: `frontend/src/components/shared/FullScreenDialog.tsx`

基於 shadcn Dialog，但 content 為全螢幕。支援：
- 標題列 + 關閉按鈕
- Dialog 堆疊（z-index 自動遞增）
- 內部捲動

### 3.4 DiffList 元件

**新增檔案**: `frontend/src/components/shared/DiffList.tsx`

以 git diff 風格顯示 passed/filtered 項目：
- `+` 綠色背景：通過篩選
- `-` 紅色背景：被過濾

---

## Phase 4：前端 — Schema / API 更新

### 4.1 更新 schemas

**修改檔案**: `frontend/src/schemas/anime.ts`
- 新增 `AnimeSeriesRich` schema（包含 anime_title, season, episode_downloaded, episode_found, subscriptions）
- 更新 `AnimeLink` 加入 `filtered_flag`, `group_name`, `download` (nullable)

**修改檔案**: `frontend/src/schemas/download.ts`
- 更新 `RawAnimeItem` 加入 `download` (nullable), `filter_passed` (nullable)

**修改檔案**: `frontend/src/schemas/parser.ts`
- 更新 `TitleParser` 加入 `created_from_type`, `created_from_id`

**新增檔案**: `frontend/src/schemas/dashboard.ts`
- `DashboardStats` schema

### 4.2 更新 API Layer

**修改檔案**: `frontend/src/layers/ApiLayer.ts`

新增 methods：
- `getAllAnimeSeries()` → `GET /api/core/anime/series` → `AnimeSeriesRich[]`
- `getDashboardStats()` → `GET /api/core/dashboard/stats` → `DashboardStats`

修改 methods：
- `getAnimeLinks(seriesId)` → 更新 schema 使用 `AnimeLinkRich`
- `getRawItems(params)` → 更新 schema 反映新欄位
- `getParsers(params?)` → 支援 `created_from_type`, `created_from_id` 查詢參數
- `createParser(req)` → request 包含 `created_from_type`, `created_from_id`

**修改檔案**: `frontend/src/services/CoreApi.ts`
- 對應更新 interface

---

## Phase 5：前端 — 頁面重構

### 5.1 AnimeSeries 主頁面（新建）

**新增檔案**: `frontend/src/pages/anime-series/AnimeSeriesPage.tsx`

表格欄位：
| 動畫名稱 | 季度 | 集數（已下載/已發現） | 來源訂閱 |

點擊行 → 開啟 AnimeSeriesDialog

### 5.2 AnimeSeriesDialog（新建）

**新增檔案**: `frontend/src/pages/anime-series/AnimeSeriesDialog.tsx`

結構：
```
FullScreenDialog
├── 動畫資訊區塊（上）
│   ├── anime_title, season, aired_date, end_date
│   └── episode_downloaded / episode_found, subscriptions
├── 主 Tab: [ 詳細資訊 | 動畫連結 ]
│   ├── Tab 1: 子 Tab [ 篩選規則 | 解析器 ]
│   │   ├── <FilterRuleEditor targetType="anime_series" targetId={seriesId} />
│   │   └── <ParserEditor createdFromType="anime_series" createdFromId={seriesId} />
│   └── Tab 2: AnimeLink 列表（DiffList 風格）
│       ├── + Ep01 SubGroup-A 1080p  ██████ 100% 完成
│       ├── + Ep02 SubGroup-A 1080p  ██░░░░  30% 下載中
│       └── - Ep01 SubGroup-B 720p   — (已過濾)
```

字幕組名稱可點擊 → 開啟 SubtitleGroupDialog（dialog 堆疊）

### 5.3 Anime 頁面改造

**修改檔案**: `frontend/src/pages/anime/AnimePage.tsx`

保留表格列表，點擊 → 開啟 AnimeDialog（而非導航到 detail page）

**新增檔案**: `frontend/src/pages/anime/AnimeDialog.tsx`

結構：
```
FullScreenDialog
├── 動畫資訊（上）
├── 所屬系列列表（可點擊 → 開啟 AnimeSeriesDialog）
├── 子 Tab [ 篩選規則 | 解析器 ]
│   ├── <FilterRuleEditor targetType="anime" targetId={animeId} />
│   └── <ParserEditor createdFromType="anime" createdFromId={animeId} />
```

### 5.4 SubtitleGroup 頁面改造

**修改檔案**: `frontend/src/pages/subtitle-groups/SubtitleGroupsPage.tsx`

點擊 → 開啟 SubtitleGroupDialog

**新增檔案**: `frontend/src/pages/subtitle-groups/SubtitleGroupDialog.tsx`

結構：
```
FullScreenDialog
├── 字幕組資訊（上）
├── 子 Tab [ 篩選規則 | 解析器 ]
│   ├── <FilterRuleEditor targetType="subtitle_group" targetId={groupId} />
│   └── <ParserEditor createdFromType="subtitle_group" createdFromId={groupId} />
```

### 5.5 Subscription 頁面改造

**修改檔案**: `frontend/src/pages/subscriptions/SubscriptionsPage.tsx`

點擊訂閱 → 開啟 SubscriptionDialog

**新增檔案**: `frontend/src/pages/subscriptions/SubscriptionDialog.tsx`

結構：
```
FullScreenDialog
├── 訂閱資訊（上）
├── 子 Tab [ 篩選規則 | 解析器 ]
│   ├── <FilterRuleEditor targetType="fetcher" targetId={subscriptionId} />
│   └── <ParserEditor createdFromType="subscription" createdFromId={subscriptionId} />
```

### 5.6 Dashboard 改為概覽頁

**修改檔案**: `frontend/src/pages/Dashboard.tsx`

顯示：
- 服務狀態（各 fetcher/downloader/viewer 的健康狀態）
- 統計卡片：活躍訂閱數、下載中、已完成、失敗、待解析 raw items、待處理衝突
- Global Filter Rules（使用 FilterRuleEditor targetType="global"）
- Global Parsers（使用 ParserEditor createdFromType="global"）

### 5.7 Raw Items 頁面更新

**修改檔案**: `frontend/src/pages/raw-items/RawItemsPage.tsx`

表格新增欄位：
- 下載狀態（進度條 + 狀態 badge）
- 過濾狀態（filter_passed badge）
- 來源訂閱名稱

---

## Phase 6：前端 — 路由 & 導航更新

### 6.1 更新路由

**修改檔案**: `frontend/src/App.tsx`

```tsx
<Route index element={<Dashboard />} />
<Route path="series" element={<AnimeSeriesPage />} />
<Route path="subscriptions" element={<SubscriptionsPage />} />
<Route path="raw-items" element={<RawItemsPage />} />
<Route path="conflicts" element={<ConflictsPage />} />
<Route path="anime" element={<AnimePage />} />
<Route path="subtitle-groups" element={<SubtitleGroupsPage />} />
```

移除的路由：
- `/anime/:animeId` → 改用 dialog
- `/anime-series/:seriesId` → 改用 dialog
- `/subtitle-groups/:groupId` → 改用 dialog
- `/downloads` → 合併到各 dialog 和 raw-items
- `/filters` → 合併到各 dialog 和 dashboard
- `/parsers` → 合併到各 dialog 和 dashboard

### 6.2 更新 Sidebar

**修改檔案**: `frontend/src/components/layout/Sidebar.tsx`

```typescript
const navItems = [
  { to: "/", icon: LayoutDashboard, labelKey: "sidebar.dashboard" },
  { to: "/series", icon: Film, labelKey: "sidebar.animeSeries" },
  { to: "/subscriptions", icon: Rss, labelKey: "sidebar.subscriptions" },
  { to: "/raw-items", icon: FileText, labelKey: "sidebar.rawItems" },
  { to: "/conflicts", icon: AlertTriangle, labelKey: "sidebar.conflicts" },
  { to: "/anime", icon: Library, labelKey: "sidebar.anime" },
  { to: "/subtitle-groups", icon: Users, labelKey: "sidebar.subtitleGroups" },
]
```

### 6.3 更新 i18n

**修改檔案**: `frontend/src/i18n/en.json`, `zh-TW.json`, `ja.json`

新增 keys：
- `sidebar.animeSeries`
- `animeSeries.title`, `animeSeries.animeTitle`, `animeSeries.season`, `animeSeries.episodes`, `animeSeries.downloaded`, `animeSeries.found`, `animeSeries.subscriptions`
- `dialog.details`, `dialog.animeLinks`, `dialog.filterRules`, `dialog.parsers`
- `filter.addRule`, `filter.removeRule`, `filter.preview`, `filter.passed`, `filter.filtered`, `filter.confirmRemove`
- `parser.belongsToTarget`, `parser.notBelongsToTarget`, `parser.warning`
- `dashboard.stats.*` (totalAnime, activeSubs, downloading, completed, etc.)
- `download.status.*` (completed, downloading, pending, failed, filtered)

移除 keys：
- `sidebar.downloads`, `sidebar.filters`, `sidebar.parsers`

---

## Phase 7：清理

### 7.1 移除前端檔案

- `frontend/src/pages/downloads/DownloadsPage.tsx`
- `frontend/src/pages/filters/FiltersPage.tsx`
- `frontend/src/pages/parsers/ParsersPage.tsx`
- `frontend/src/pages/anime/AnimeDetailPage.tsx`（被 AnimeDialog 取代）
- `frontend/src/pages/subtitle-groups/SubtitleGroupDetailPage.tsx`（被 SubtitleGroupDialog 取代）
- `frontend/src/pages/anime-series/AnimeSeriesDetailPage.tsx`（被 AnimeSeriesDialog 取代）

### 7.2 後端清理

- 保留所有現有 API 端點（向後相容）
- 新端點與舊端點共存，前端逐步遷移

---

## 實作順序 & 依賴關係

```
Phase 1 (DB migration)
  ↓
Phase 2 (Backend API) — 可與 Phase 3 部分並行
  ↓
Phase 3 (Shared components) — 可與 Phase 4 並行
  ↓
Phase 4 (Schema/API layer update)
  ↓
Phase 5 (Page restructuring) — 依賴 Phase 3 + 4
  ↓
Phase 6 (Routes/navigation) — 依賴 Phase 5
  ↓
Phase 7 (Cleanup) — 最後
```

## 預估改動檔案總覽

| 類別 | 新增 | 修改 |
|------|------|------|
| DB Migration | 1 | 0 |
| Backend handlers | 1 (dashboard) | 4 (anime, links, raw_items, filters, parsers) |
| Backend services | 1 (filter_recalc) | 0 |
| Backend models/dto | 0 | 2 (db.rs, dto.rs) |
| Frontend components | 4 (FilterRuleEditor, ParserEditor, FullScreenDialog, DiffList) | 0 |
| Frontend pages | 4 (AnimeSeriesPage, AnimeSeriesDialog, AnimeDialog, SubtitleGroupDialog, SubscriptionDialog) | 4 (Dashboard, AnimePage, SubtitleGroupsPage, SubscriptionsPage, RawItemsPage) |
| Frontend schemas | 1 (dashboard) | 3 (anime, download, parser) |
| Frontend API | 0 | 2 (ApiLayer, CoreApi) |
| Frontend routing | 0 | 2 (App.tsx, Sidebar.tsx) |
| Frontend i18n | 0 | 3 (en, zh-TW, ja) |
| 移除 | — | 6 files |
