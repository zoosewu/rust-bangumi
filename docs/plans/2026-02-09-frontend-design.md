# Bangumi Frontend Design

## Context

目前系統是純後端微服務架構（Core/Fetcher/Downloader/Viewer），所有操作只能透過 CLI 或 HTTP API。此設計為專案新增 React SPA 前端介面，讓使用者能透過瀏覽器管理 Anime、訂閱、下載、Filter 和 Title Parser，並支援 Filter/Parser 的即時預覽功能。

此設計基於原始 `frontend-implementation-plan.md`，進行以下調整：
1. Preview 模式改為左右對比佈局（before/after）
2. 技術棧改為 React 19 + Effect-TS，移除功能重複的框架
3. 部署方案涵蓋開發環境和生產環境（Caddy 取代 Nginx）

---

## 技術棧

| 用途 | 技術 | 說明 |
|------|------|------|
| Framework | React 19 + TypeScript | 使用 React 19 的 `use()` 和 Suspense |
| Build | Vite | 開發伺服器 + 打包 |
| 核心框架 | Effect-TS | HttpClient、Schema、Service/Layer、錯誤處理 |
| UI 元件 | Shadcn/UI | Radix UI + Tailwind CSS + lucide-react |
| Routing | React Router v7 | 頁面路由 |
| 靜態伺服器 | Caddy | 生產環境靜態檔 + 反向代理 |

**移除的框架**（由 Effect-TS 取代）：

| 原計劃 | Effect 替代方案 |
|--------|----------------|
| Axios | `@effect/platform` HttpClient |
| TanStack Query | Effect + custom hooks (useEffectQuery) |
| Zod | `@effect/schema` (Schema) |
| React Hook Form 驗證 | Effect Schema 驗證 |

---

## Phase 1: Backend Preview APIs

在 core-service 新增兩個預覽端點，支援 before/after 對比。

### 1.1 `POST /filters/preview`

**檔案**: `core-service/src/handlers/filters.rs`

接收 filter 設定 + `exclude_filter_id`，回傳「排除當前 filter」和「加上當前 filter」兩組結果。

**Request**:
```json
{
  "regex_pattern": "1080p",
  "is_positive": true,
  "subscription_id": 1,
  "exclude_filter_id": 5,
  "limit": 50
}
```

**Response**:
```json
{
  "regex_valid": true,
  "regex_error": null,
  "before": {
    "passed_items": [
      { "item_id": 1, "title": "[SubGroup] Anime Title - 01 [1080p]" },
      { "item_id": 5, "title": "[SubGroup] Anime Title - 02 [720p]" }
    ],
    "filtered_items": [
      { "item_id": 10, "title": "[Other] Something [480p]" }
    ]
  },
  "after": {
    "passed_items": [
      { "item_id": 1, "title": "[SubGroup] Anime Title - 01 [1080p]" }
    ],
    "filtered_items": [
      { "item_id": 5, "title": "[SubGroup] Anime Title - 02 [720p]" },
      { "item_id": 10, "title": "[Other] Something [480p]" }
    ]
  }
}
```

- `before`: 套用所有 filters（排除 `exclude_filter_id`）的結果
- `after`: 套用所有 filters（含當前 filter 設定）的結果
- 新建 filter 時 `exclude_filter_id` 為 null
- `subscription_id` 可選，限定 raw items 範圍

### 1.2 `POST /parsers/preview`

**檔案**: `core-service/src/handlers/parsers.rs`

接收 parser 設定 + `exclude_parser_id`，回傳「排除當前 parser」和「加上當前 parser」兩組結果，展示優先權分配和解析詳情。

**Request**:
```json
{
  "condition_regex": "\\[喵萌奶茶屋\\]",
  "parse_regex": "\\[(.+?)\\]\\s*(.+?)\\s*-\\s*(\\d+)",
  "priority": 5,
  "anime_title_source": "regex",
  "anime_title_value": "2",
  "episode_no_source": "regex",
  "episode_no_value": "3",
  "subtitle_group_source": "regex",
  "subtitle_group_value": "1",
  "series_no_source": "static",
  "series_no_value": "1",
  "exclude_parser_id": 3,
  "subscription_id": null,
  "limit": 20
}
```

**Response**:
```json
{
  "condition_regex_valid": true,
  "parse_regex_valid": true,
  "regex_error": null,
  "results": [
    {
      "title": "[喵萌奶茶屋] 進擊的巨人 - 01 [1080p]",
      "before_matched_by": "parser-A",
      "after_matched_by": "parser-A",
      "is_newly_matched": false,
      "is_override": false,
      "parse_result": null
    },
    {
      "title": "[字幕組] 鬼滅之刃 - 03",
      "before_matched_by": null,
      "after_matched_by": "當前 parser",
      "is_newly_matched": true,
      "is_override": false,
      "parse_result": {
        "anime_title": "鬼滅之刃",
        "episode_no": 3,
        "series_no": 1,
        "subtitle_group": "字幕組",
        "resolution": null,
        "season": null,
        "year": null
      }
    },
    {
      "title": "[other] 咒術迴戰 - 05",
      "before_matched_by": "parser-B",
      "after_matched_by": "當前 parser",
      "is_newly_matched": false,
      "is_override": true,
      "parse_result": {
        "anime_title": "咒術迴戰",
        "episode_no": 5,
        "series_no": 1,
        "subtitle_group": "other",
        "resolution": null,
        "season": null,
        "year": null
      }
    }
  ]
}
```

- `before_matched_by`: 排除當前 parser 時，匹配到的 parser name（null = 未匹配）
- `after_matched_by`: 加上當前 parser 時，匹配到的 parser name
- `is_newly_matched`: 從未匹配 → 被當前 parser 匹配
- `is_override`: 從其他 parser → 被當前 parser 優先權覆蓋
- `parse_result`: 當前 parser 的完整解析結果（僅 after_matched_by = 當前 parser 時有值）

### 1.3 Backend Changes

| File | Change |
|------|--------|
| `core-service/src/services/title_parser.rs:79` | `fn try_parser` → `pub fn try_parser` |
| `core-service/src/handlers/filters.rs` | 新增 `preview_filter` handler |
| `core-service/src/handlers/parsers.rs` | 新增 `preview_parser` handler |
| `core-service/src/main.rs` | 新增 2 個 preview 路由 |

---

## Phase 2: Frontend Project Scaffolding

### 2.1 專案結構

```
frontend/
├── package.json
├── vite.config.ts
├── tailwind.config.ts
├── tsconfig.json
├── components.json          # Shadcn/UI config
├── index.html
├── Dockerfile               # Multi-stage: node build → caddy serve
├── Caddyfile                # 生產環境 reverse proxy + SPA
└── src/
    ├── main.tsx              # React 19 entry + Effect runtime provider
    ├── App.tsx               # React Router v7 路由定義
    ├── index.css             # Tailwind imports
    │
    ├── lib/
    │   └── utils.ts          # Shadcn cn() utility
    │
    ├── runtime/
    │   └── AppRuntime.ts     # Effect ManagedRuntime (提供所有 Layers)
    │
    ├── layers/
    │   └── ApiLayer.ts       # HttpClient Layer + base URL config
    │
    ├── schemas/              # Effect Schema (型別 + 驗證)
    │   ├── anime.ts
    │   ├── subscription.ts
    │   ├── filter.ts
    │   ├── parser.ts
    │   ├── download.ts
    │   └── common.ts
    │
    ├── services/             # Effect Services (API 客戶端)
    │   ├── CoreApi.ts
    │   ├── DownloaderApi.ts
    │   └── ViewerApi.ts
    │
    ├── hooks/                # React hooks (Effect → React bridge)
    │   ├── useEffectQuery.ts
    │   ├── useEffectMutation.ts
    │   ├── useAnime.ts
    │   ├── useSubscriptions.ts
    │   ├── useFilters.ts
    │   ├── useParsers.ts
    │   ├── useDownloads.ts
    │   └── useHealth.ts
    │
    ├── components/
    │   ├── ui/               # Shadcn/UI 生成的元件
    │   ├── layout/
    │   │   ├── AppLayout.tsx
    │   │   ├── Sidebar.tsx
    │   │   └── Header.tsx
    │   └── shared/
    │       ├── DataTable.tsx
    │       ├── StatusBadge.tsx
    │       ├── ConfirmDialog.tsx
    │       └── RegexInput.tsx   # Regex 輸入 + 即時驗證
    │
    └── pages/
        ├── Dashboard.tsx
        ├── anime/
        │   ├── AnimePage.tsx
        │   └── AnimeDetailPage.tsx
        ├── subscriptions/
        │   └── SubscriptionsPage.tsx
        ├── raw-items/
        │   └── RawItemsPage.tsx
        ├── filters/
        │   └── FiltersPage.tsx       # 左右對比 preview
        ├── parsers/
        │   └── ParsersPage.tsx       # 匹配分配表 + 解析結果
        ├── downloads/
        │   └── DownloadsPage.tsx
        └── conflicts/
            └── ConflictsPage.tsx
```

### 2.2 Effect-TS 架構

**Services** — 使用 Effect Context.Tag 定義 API 客戶端：

```typescript
// services/CoreApi.ts
import { Effect, Context } from "effect"
import { HttpClient, HttpClientRequest } from "@effect/platform"

class CoreApi extends Context.Tag("CoreApi")<CoreApi, {
  getAnimes: Effect.Effect<Anime[], HttpClientError>
  previewFilter: (req: FilterPreviewRequest) => Effect.Effect<FilterPreviewResponse, HttpClientError>
  previewParser: (req: ParserPreviewRequest) => Effect.Effect<ParserPreviewResponse, HttpClientError>
  // ... 其他 API methods
}>() {}
```

**Layers** — 依賴注入和環境配置：

```typescript
// layers/ApiLayer.ts
const CoreApiLive = Layer.succeed(CoreApi, {
  getAnimes: HttpClient.get("/api/core/animes").pipe(
    Effect.flatMap(res => res.json),
    Effect.flatMap(Schema.decodeUnknown(Schema.Array(AnimeSchema)))
  ),
  // ...
})
```

**Runtime** — ManagedRuntime 提供所有 layers：

```typescript
// runtime/AppRuntime.ts
const AppLayer = Layer.mergeAll(CoreApiLive, DownloaderApiLive, ViewerApiLive)
const AppRuntime = ManagedRuntime.make(AppLayer)
```

**Hooks** — Effect → React bridge：

```typescript
// hooks/useEffectQuery.ts
function useEffectQuery<A, E>(
  effect: Effect.Effect<A, E, CoreApi>,
  deps: unknown[]
): { data: A | null; error: E | null; isLoading: boolean; refetch: () => void }
```

### 2.3 API Client 架構

| Prefix | Target | Description |
|--------|--------|-------------|
| `/api/core` | core-service:8000 | 主要 CRUD 及 preview APIs |
| `/api/downloader` | downloader-qbittorrent:8002 | 下載狀態與控制 |
| `/api/viewer` | viewer-jellyfin:8003 | Viewer 健康檢查 |

- 開發環境：Vite proxy
- 生產環境：Caddy reverse proxy

---

## Phase 3: Layout + Dashboard

### 3.1 AppLayout
- 左側 Sidebar：Dashboard / Anime / 訂閱 / Raw Items / 下載 / Filters / Parsers / Conflicts
- 頂部 Header + breadcrumb
- 主內容區 via `<Outlet />`

### 3.2 Dashboard Page (`/`)
- 統計卡片：Anime 數量、活躍訂閱數、待解析 Raw Items 數、活躍下載數
- 服務健康狀態（輪詢 30 秒）：各服務 green/red 指示燈
- 最近活動：最新 10 筆 raw items

---

## Phase 4: Anime + Series Management

### 4.1 Anime Page (`/anime`)
- DataTable（title, series count, created_at）
- 新增 / 刪除 Anime Dialog

### 4.2 Anime Detail Page (`/anime/:animeId`)
- Tabs: Series | Links | Filters
- Series: 新增/瀏覽（含 season 選擇器）
- Links: 依 series 顯示 download links
- Filters: 此 anime 的 filter rules

### 4.3 相關管理
- Season CRUD（year + season enum）
- Subtitle Group CRUD

---

## Phase 5: Subscription + Raw Items

### 5.1 Subscriptions Page (`/subscriptions`)
- DataTable: name, source_url, fetch_interval, last/next fetch, status badge
- 新增 / 刪除訂閱

### 5.2 Raw Items Page (`/raw-items`)
- 篩選欄：status dropdown, subscription dropdown
- DataTable + 分頁（limit/offset）
- 操作：Reparse / Skip
- Status badge: pending / parsed / no_match / failed / skipped

---

## Phase 6: Filter Rules + Live Preview

### Filters Page (`/filters`) — 左右對比

**上方** — Filter 表單：
- regex_pattern 輸入（含即時 regex 驗證）
- include/exclude 切換
- subscription 選擇（限定範圍）

**下方** — 左右對比面板：

```
┌────────────────────────┬────────────────────────────┐
│  Before (無此 filter)   │  After (套用此 filter)      │
│                        │                            │
│  ✅ 通過: 45            │  ✅ 通過: 32               │
│  ❌ 過濾: 5             │  ❌ 過濾: 18               │
│                        │                            │
│  ● [Sub] Anime - 01   │  ✅ [Sub] Anime - 01 1080p │
│  ● [Sub] Anime - 02   │  ❌ [Sub] Anime - 02 720p  │
│  ● [Sub] Anime - 03   │  ✅ [Sub] Anime - 03 1080p │
└────────────────────────┴────────────────────────────┘
```

- 左欄：套用所有 filters（排除當前 filter）的結果
- 右欄：加上當前 filter 的結果
- Debounce 500ms 呼叫 `POST /filters/preview`
- 右欄用顏色標記：通過（綠）/ 被過濾（灰/紅）
- 上方顯示通過/過濾數量統計

---

## Phase 7: Title Parser + Live Preview

### Parsers Page (`/parsers`) — 匹配分配 + 解析結果

**上方** — Parser 表單：
- name, priority, is_enabled
- condition_regex, parse_regex
- 各欄位 source/value 對（anime_title, episode_no, series_no, subtitle_group, resolution, season, year）

**下方** — 兩部分預覽：

**Part 1: 匹配分配表 (Match Assignment)**

```
┌──────────────────────────────┬────────────────┬──────────┐
│ 標題                          │ Before         │ After    │
├──────────────────────────────┼────────────────┼──────────┤
│ [喵萌] 進擊的巨人 - 01 [1080p]│ parser-A (p:1) │ 同左     │
│ [字幕組] 鬼滅之刃 - 03        │ ❌ 未匹配       │ ⭐ 當前   │
│ [other] 咒術迴戰 - 05         │ parser-B (p:3) │ ⭐ 當前   │
│  └ 優先權覆蓋                                             │
└──────────────────────────────┴────────────────┴──────────┘
```

- 每行顯示一個 raw item 標題
- Before 欄：排除當前 parser 時被哪個 parser 匹配
- After 欄：加上當前 parser 後被哪個 parser 匹配
- 高亮新匹配（is_newly_matched）和優先權覆蓋（is_override）

**Part 2: 解析結果 (Parse Results)**

僅顯示當前 parser 匹配到的 items，展開完整解析欄位：

```
┌─────────────────────────────────────────────────────┐
│ 原始標題: [字幕組] 鬼滅之刃 - 03                      │
│ 動畫: 鬼滅之刃 │ 集數: 3 │ 季: 1 │ 字幕組: 字幕組     │
│ 解析度: null │ Season: null │ Year: null              │
├─────────────────────────────────────────────────────┤
│ 原始標題: [other] 咒術迴戰 - 05                       │
│ 動畫: 咒術迴戰 │ 集數: 5 │ 季: 1 │ 字幕組: other      │
│ 解析度: null │ Season: null │ Year: null              │
└─────────────────────────────────────────────────────┘
```

- Debounce 500ms 呼叫 `POST /parsers/preview`
- 展示所有解析欄位：anime_title, episode_no, series_no, subtitle_group, resolution, season, year

---

## Phase 8: Download Management

### Downloads Page (`/downloads`)
- 查詢 `GET /api/downloader/downloads?hashes=...`
- DataTable: title, status badge, progress bar, size, actions
- 操作：Pause / Resume / Cancel / Delete
- 自動 5 秒刷新活躍下載

---

## Phase 9: Conflicts + Polish

- Conflicts Page：衝突列表 + fetcher 選擇解決
- Toast 通知（所有 mutation）
- Loading skeleton + error state
- 響應式佈局

---

## Phase 10: 部署方案

### 開發環境

```
本機 (localhost)
├── Vite Dev Server (:5173) + HMR
│   └── Proxy:
│       /api/core/*       → localhost:8000
│       /api/downloader/* → localhost:8002
│       /api/viewer/*     → localhost:8003
├── cargo run -p core-service (:8000)
├── cargo run -p fetcher-mikanani (:8001)
├── cargo run -p downloader-qbittorrent (:8002)
└── cargo run -p viewer-jellyfin (:8003)

Docker (docker-compose.dev.yaml)
├── PostgreSQL (:5432)
├── Adminer (:8081)
├── qBittorrent (:8080)
└── Jellyfin (:8096)
```

啟動流程：
```bash
docker compose -f docker-compose.dev.yaml up -d   # 基礎設施
cargo run -p core-service                          # 後端服務
cd frontend && npm run dev                         # 前端 Vite
```

**vite.config.ts proxy 設定：**
```typescript
export default defineConfig({
  server: {
    proxy: {
      '/api/core': {
        target: 'http://localhost:8000',
        rewrite: (path) => path.replace(/^\/api\/core/, ''),
      },
      '/api/downloader': {
        target: 'http://localhost:8002',
        rewrite: (path) => path.replace(/^\/api\/downloader/, ''),
      },
      '/api/viewer': {
        target: 'http://localhost:8003',
        rewrite: (path) => path.replace(/^\/api\/viewer/, ''),
      },
    },
  },
})
```

### 生產環境

```
Docker Compose (docker-compose.yaml)
├── Caddy (:80/:443) ← 唯一對外
│   ├── /* → /srv (SPA 靜態檔)
│   ├── /api/core/* → core-service:8000
│   ├── /api/downloader/* → downloader:8002
│   └── /api/viewer/* → viewer:8003
├── core-service (:8000)
├── fetcher-mikanani (:8001)
├── downloader-qbittorrent (:8002)
├── viewer-jellyfin (:8003)
└── PostgreSQL (:5432)
```

**frontend/Dockerfile（multi-stage）：**
```dockerfile
FROM node:22-alpine AS builder
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM caddy:alpine
COPY --from=builder /app/dist /srv
COPY Caddyfile /etc/caddy/Caddyfile
EXPOSE 80 443
```

**frontend/Caddyfile：**
```
:80 {
    encode gzip

    handle /api/core/* {
        uri strip_prefix /api/core
        reverse_proxy core-service:8000
    }
    handle /api/downloader/* {
        uri strip_prefix /api/downloader
        reverse_proxy downloader-qbittorrent:8002
    }
    handle /api/viewer/* {
        uri strip_prefix /api/viewer
        reverse_proxy viewer-jellyfin:8003
    }

    handle {
        root * /srv
        file_server
        try_files {path} /index.html
    }
}
```

**docker-compose.yaml 新增：**
```yaml
frontend:
  build:
    context: ./frontend
    dockerfile: Dockerfile
  container_name: bangumi-frontend
  depends_on:
    core-service:
      condition: service_healthy
  ports:
    - "${FRONTEND_PORT:-3000}:80"
  networks:
    bangumi-network:
      ipv4_address: 172.25.0.20
  restart: always
```

---

## Verification Checklist

1. `cargo build` - core-service 編譯通過
2. `curl -X POST localhost:8000/filters/preview` - filter preview API 正常
3. `curl -X POST localhost:8000/parsers/preview` - parser preview API 正常
4. `cd frontend && npm run dev` - 前端開發伺服器啟動
5. `docker compose up --build frontend` - Docker 部署正常
6. E2E: Dashboard 顯示健康狀態 → Anime CRUD → 訂閱管理 → Filter/Parser 預覽
