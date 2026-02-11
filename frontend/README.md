# Bangumi Frontend

Bangumi 動畫管理系統的 Web 前端介面。

## 技術棧

- **React 19** + TypeScript
- **Vite 7** — 開發伺服器與建構工具
- **Effect-TS** — 類型安全的 API 呼叫、Schema 驗證、錯誤處理
- **Shadcn/UI** — Radix UI + Tailwind CSS 4 元件庫
- **Caddy** — 生產環境反向代理與靜態檔案伺服器

## 開發

```bash
# 安裝依賴
npm install

# 啟動開發伺服器（http://localhost:5173）
npm run dev

# TypeScript 檢查 + 建構
npm run build

# 預覽建構結果
npm run preview

# ESLint 檢查
npm run lint
```

### API 代理

開發伺服器（Vite）自動代理 API 請求至後端服務：

| 前綴 | 目標 |
|------|------|
| `/api/core/` | `http://localhost:8000` (Core Service) |
| `/api/downloader/` | `http://localhost:8002` (Downloader) |
| `/api/viewer/` | `http://localhost:8003` (Viewer) |

> 啟動前端前，請確保後端服務已在本地運行。

## 頁面功能

| 路由 | 功能 |
|------|------|
| `/` | Dashboard — 服務健康狀態 |
| `/anime` | Anime CRUD 管理 |
| `/anime/:id` | Anime 詳情 — 系列、Filter 規則 |
| `/subscriptions` | RSS 訂閱瀏覽 |
| `/raw-items` | 原始 RSS 項目（狀態篩選、分頁） |
| `/downloads` | 下載進度管理（自動刷新） |
| `/filters` | Filter 規則 CRUD + 即時 before/after 預覽 |
| `/parsers` | Title Parser CRUD + 即時解析預覽 |
| `/conflicts` | Fetcher 衝突解決 |

## 架構

```
src/
├── services/CoreApi.ts    # Effect.Context.Tag — 定義所有 API 方法
├── layers/ApiLayer.ts     # Effect Layer — HttpClient 實作 CoreApi
├── runtime/AppRuntime.ts  # ManagedRuntime — 提供 BrowserHttpClient
├── schemas/               # Effect Schema — 後端 DTO 型別定義與驗證
├── hooks/                 # useEffectQuery / useEffectMutation
├── components/
│   ├── ui/                # Shadcn/UI 生成的元件
│   ├── layout/            # AppLayout, Sidebar, Header
│   └── shared/            # DataTable, StatusBadge, ConfirmDialog, RegexInput
└── pages/                 # 各功能頁面元件
```

## Docker 部署

```bash
# 建構映像
docker build -t bangumi-frontend .

# 運行（需要後端服務在同一 Docker network）
docker run -p 3000:80 bangumi-frontend
```

生產環境使用 Caddy 提供靜態檔案並反向代理 API 至後端服務。設定見 `Caddyfile`。
