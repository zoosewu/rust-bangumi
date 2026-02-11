# Bangumi Frontend — Implementation Summary

> 原始計劃：`2026-02-09-frontend-implementation-plan.md`（16 Tasks）
> 狀態：**已完成**

## 技術棧（實際採用）

| 類別 | 技術 | 版本 |
|------|------|------|
| Framework | React | 19.2 |
| Language | TypeScript | 5.9 |
| Build Tool | Vite | 7.3 |
| Data Layer | Effect-TS (effect, @effect/schema, @effect/platform) | 3.19 |
| UI Components | Shadcn/UI (New York style) | Radix UI + Tailwind CSS 4 |
| Routing | React Router | 7.13 |
| Icons | Lucide React | 0.563 |
| Toast | Sonner | 2.0 |
| Runtime / Build | Bun | Alpine |
| Production Server | Caddy | Alpine |

## Backend Changes

### 新增端點

| Method | Path | Handler | 說明 |
|--------|------|---------|------|
| `POST` | `/filters/preview` | `handlers::filters::preview_filter` | Filter 即時預覽（before/after 比較） |
| `POST` | `/parsers/preview` | `handlers::parsers::preview_parser` | Parser 即時預覽（priority-based 匹配比較） |
| `GET` | `/downloads` | `handlers::downloads::list_downloads` | 下載記錄查詢（含 anime_link title join） |

### 修改的檔案

| 檔案 | 變更 |
|------|------|
| `core-service/src/services/title_parser.rs:79` | `fn try_parser` → `pub fn try_parser` |
| `core-service/src/handlers/filters.rs` | +207 行（preview DTOs + handler + apply logic） |
| `core-service/src/handlers/parsers.rs` | +226 行（preview DTOs + handler + find_matching_parser） |
| `core-service/src/handlers/downloads.rs` | 新檔案 100 行（DownloadRow + list_downloads） |
| `core-service/src/handlers/mod.rs` | +1 行（`pub mod downloads`） |
| `core-service/src/main.rs` | +3 路由 |

## Frontend Structure

```
frontend/                          # 68 files, ~14,400 lines
├── src/
│   ├── services/CoreApi.ts        # 17 個 API 端點定義
│   ├── layers/ApiLayer.ts         # Effect Layer 實作（HttpClient → CoreApi）
│   ├── runtime/AppRuntime.ts      # ManagedRuntime（BrowserHttpClient）
│   ├── schemas/                   # 6 個 Effect Schema 檔案
│   │   ├── anime.ts               # Anime, AnimeSeries, Season, SubtitleGroup
│   │   ├── common.ts              # PreviewItem
│   │   ├── download.ts            # RawAnimeItem, DownloadRow
│   │   ├── filter.ts              # FilterRule, FilterPreviewPanel, FilterPreviewResponse
│   │   ├── parser.ts              # TitleParser, ParsedFields, ParserPreviewResult/Response
│   │   └── subscription.ts        # Subscription
│   ├── hooks/
│   │   ├── useEffectQuery.ts      # Effect → React query state bridge
│   │   └── useEffectMutation.ts   # Effect → React mutation bridge
│   ├── components/
│   │   ├── ui/                    # 15 個 Shadcn/UI 元件
│   │   ├── layout/                # AppLayout, Sidebar (8 nav items), Header
│   │   └── shared/                # DataTable<T>, StatusBadge, ConfirmDialog, RegexInput
│   └── pages/
│       ├── Dashboard.tsx           # 服務健康狀態
│       ├── anime/                  # AnimePage (CRUD) + AnimeDetailPage (tabs)
│       ├── subscriptions/          # SubscriptionsPage (read-only table)
│       ├── raw-items/              # RawItemsPage (status filter + pagination)
│       ├── downloads/              # DownloadsPage (auto-refresh 5s + progress bars)
│       ├── filters/                # FiltersPage (CRUD + debounced before/after preview)
│       ├── parsers/                # ParsersPage (CRUD + priority-based match preview)
│       └── conflicts/              # ConflictsPage (resolve with fetcher selection)
├── Dockerfile                     # Multi-stage: Bun Alpine → Caddy Alpine
├── Caddyfile                      # Reverse proxy + SPA fallback
└── .dockerignore
```

## Routing

| Path | Page | Features |
|------|------|----------|
| `/` | Dashboard | Core Service 健康狀態 |
| `/anime` | AnimePage | DataTable, 新增/刪除 Dialog |
| `/anime/:animeId` | AnimeDetailPage | Filter 規則 tab |
| `/subscriptions` | SubscriptionsPage | 訂閱列表 |
| `/raw-items` | RawItemsPage | Status 篩選, limit/offset 分頁 |
| `/downloads` | DownloadsPage | Progress bars, 5 秒自動刷新 |
| `/filters` | FiltersPage | Regex 輸入 + debounce 500ms before/after 預覽 |
| `/parsers` | ParsersPage | 完整 parser 表單 + priority-based match 預覽 |
| `/conflicts` | ConflictsPage | Fetcher 選擇解決 |

## Deployment

### Docker Compose

```yaml
frontend:
  build: ./frontend
  container_name: bangumi-frontend
  depends_on:
    core-service: { condition: service_healthy }
  ports: ["${FRONTEND_PORT:-3000}:80"]
  networks:
    bangumi-network: { ipv4_address: 172.25.0.20 }
```

### Caddy Reverse Proxy

| Path | Target |
|------|--------|
| `/api/core/*` | `core-service:8000` |
| `/api/downloader/*` | `downloader-qbittorrent:8002` |
| `/api/viewer/*` | `viewer-jellyfin:8003` |
| `/*` | SPA (try_files → index.html) |

### 開發環境

- `bun run dev` — Vite dev server on :5173
- Vite proxy 配置對應 Caddy 代理路徑
- `bun run build` — TypeScript check + Vite production build

## Commit History

```
e28ca3b feat(frontend): add Caddy + Docker deployment config
e6c0c93 feat: add Downloads page, Conflicts page, and Toaster
175b2a1 feat(frontend): add Parsers page with CRUD and match preview
adb5151 feat(frontend): add Filters page with CRUD and before/after preview
21cdef2 feat(frontend): add Subscriptions and Raw Items pages
a06c32b feat(frontend): add Anime list and detail pages
f689039 feat(frontend): implement Dashboard page with service health status
ea76bd7 feat(frontend): add shared components — DataTable, StatusBadge, ConfirmDialog, RegexInput
0789cbf feat(frontend): add AppLayout with Sidebar, Header, and React Router setup
8a33425 feat(frontend): add useEffectQuery and useEffectMutation React hooks
35bff58 feat(frontend): add Effect-TS schemas, CoreApi service, layers, and runtime
723168e feat(frontend): scaffold Vite + React 19 + TypeScript + Tailwind project
2b1ccae feat(core): add POST /parsers/preview endpoint with priority-based match assignment
96ecec7 feat(core): add POST /filters/preview endpoint with before/after comparison
```
