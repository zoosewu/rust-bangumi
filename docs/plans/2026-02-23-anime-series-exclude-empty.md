# Anime Series Exclude Empty Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 `GET /api/core/series` 加入 `?exclude_empty=true` 查詢參數，讓前端可以自動過濾掉 `episode_found == 0` 的動畫季度。

**Architecture:** 後端 `list_all_anime_series` handler 加入 `Query<ExcludeEmptyParams>` 提取 `exclude_empty` 旗標，在 for loop 組裝結果後套用過濾。前端 `getAllAnimeSeries` 由靜態 Effect 改為接受 optional 參數的函式，`AnimeSeriesPage` 及 `AnimeDialog` 呼叫時固定帶 `exclude_empty: true`；`ParsersPage` 中的呼叫也傳入此參數。

**Tech Stack:** Rust (Diesel ORM, axum, serde), React/TypeScript (Effect.ts, HttpClient)

---

### Task 1: 後端 — `list_all_anime_series` 加入 `exclude_empty` 參數

**Files:**
- Modify: `core-service/src/handlers/anime.rs`

**背景：**
`list_all_anime_series` 在 line 223 定義，目前簽名只有 `State(state): State<AppState>`。
它在 for loop 內計算每個 series 的 `episode_found: i64`，然後 `results.push(...)` 。
需要加入 axum 的 `Query` extractor 讀取 `exclude_empty: bool`，並在 push 前過濾。

**Step 1: 在 `list_all_anime_series` 函式前加入 Query 結構體**

在 line 222（`/// List all anime series...` comment）之前插入：

```rust
#[derive(serde::Deserialize, Default)]
struct ExcludeEmptyParams {
    #[serde(default)]
    exclude_empty: bool,
}
```

**Step 2: 修改函式簽名，加入 Query extractor**

原本（line 223–225）：
```rust
pub async fn list_all_anime_series(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
```

改為：
```rust
pub async fn list_all_anime_series(
    State(state): State<AppState>,
    Query(params): Query<ExcludeEmptyParams>,
) -> (StatusCode, Json<serde_json::Value>) {
```

注意：`Query` 已在 axum 的 prelude 中，但需確認 use 宣告。在檔案頂部的 `use axum::{...}` 區塊加入 `extract::Query`（若尚未存在）。

**Step 3: 在 results push 後加入過濾**

找到 line 320（`results.push(AnimeSeriesRichResponse { ... });` 的結尾 `}` 後的第一行）附近：

原本：
```rust
    (StatusCode::OK, Json(json!({ "series": results })))
```

改為：
```rust
    if params.exclude_empty {
        results.retain(|r| r.episode_found > 0);
    }

    (StatusCode::OK, Json(json!({ "series": results })))
```

**Step 4: 確認 `use axum::extract::Query` 存在**

執行：
```bash
cd /workspace/core-service && grep "extract::Query" src/handlers/anime.rs
```

若無輸出，找到 `use axum::{` 區塊並加入 `extract::Query,`。

**Step 5: 確認編譯通過**

```bash
cd /workspace/core-service && cargo build 2>&1 | head -60
```

Expected: 無錯誤（可能有 warning）。

**Step 6: Commit**

```bash
git add core-service/src/handlers/anime.rs
git commit -m "feat(api): add exclude_empty query param to list_all_anime_series"
```

---

### Task 2: 前端 — `CoreApi` 介面與 `ApiLayer` 實作更新

**Files:**
- Modify: `frontend/src/services/CoreApi.ts`
- Modify: `frontend/src/layers/ApiLayer.ts`

**背景：**
目前 `getAllAnimeSeries` 在 `CoreApi.ts` line 66 定義為：
```typescript
readonly getAllAnimeSeries: Effect.Effect<readonly AnimeSeriesRich[]>
```
這是靜態 Effect（無參數）。需要改為接受 optional 參數的函式。

在 `ApiLayer.ts` line 201–204：
```typescript
getAllAnimeSeries: fetchJson(
  HttpClientRequest.get("/api/core/series"),
  Schema.Struct({ series: Schema.Array(AnimeSeriesRich) }),
).pipe(Effect.map((r) => r.series)),
```
也需要改為函式形式。

**Step 1: 修改 `CoreApi.ts` 中 `getAllAnimeSeries` 的型別**

找到 line 66：
```typescript
    readonly getAllAnimeSeries: Effect.Effect<readonly AnimeSeriesRich[]>
```

改為：
```typescript
    readonly getAllAnimeSeries: (params?: { excludeEmpty?: boolean }) => Effect.Effect<readonly AnimeSeriesRich[]>
```

**Step 2: 修改 `ApiLayer.ts` 中 `getAllAnimeSeries` 的實作**

找到：
```typescript
    getAllAnimeSeries: fetchJson(
      HttpClientRequest.get("/api/core/series"),
      Schema.Struct({ series: Schema.Array(AnimeSeriesRich) }),
    ).pipe(Effect.map((r) => r.series)),
```

改為：
```typescript
    getAllAnimeSeries: (params) => {
      const url = params?.excludeEmpty ? "/api/core/series?exclude_empty=true" : "/api/core/series"
      return fetchJson(
        HttpClientRequest.get(url),
        Schema.Struct({ series: Schema.Array(AnimeSeriesRich) }),
      ).pipe(Effect.map((r) => r.series))
    },
```

**Step 3: 確認 TypeScript 型別無誤**

```bash
cd /workspace/frontend && npx tsc --noEmit 2>&1 | head -40
```

Expected: 有型別錯誤，因為 callers 尚未更新為函式呼叫形式（下一個 task 修正）。記錄錯誤行號。

**Step 4: Commit**

```bash
git add frontend/src/services/CoreApi.ts frontend/src/layers/ApiLayer.ts
git commit -m "feat(frontend): change getAllAnimeSeries to accept optional excludeEmpty param"
```

---

### Task 3: 前端 — 更新所有 `getAllAnimeSeries` 呼叫處

**Files:**
- Modify: `frontend/src/pages/anime-series/AnimeSeriesPage.tsx`
- Modify: `frontend/src/pages/anime/AnimeDialog.tsx`
- Modify: `frontend/src/pages/parsers/ParsersPage.tsx`

**背景：**
`getAllAnimeSeries` 現在是函式而非靜態 Effect。所有呼叫處需要從
`api.getAllAnimeSeries` 改為 `api.getAllAnimeSeries({ excludeEmpty: true })`。

各檔案使用方式：
- `AnimeSeriesPage.tsx`：使用 `useEffectQuery` 傳入 `Effect.flatMap(CoreApi, (api) => api.getAllAnimeSeries)`
- `AnimeDialog.tsx`：使用 `useEffectQuery` 傳入 `Effect.flatMap(CoreApi, (api) => api.getAllAnimeSeries)`
- `ParsersPage.tsx`：在 `handleEntityClick` 中使用 `AppRuntime.runPromise(Effect.flatMap(CoreApi, (api) => api.getAllAnimeSeries))`

**Step 1: 更新 `AnimeSeriesPage.tsx`**

執行以下指令定位：
```bash
grep -n "getAllAnimeSeries" /workspace/frontend/src/pages/anime-series/AnimeSeriesPage.tsx
```

找到類似：
```typescript
Effect.flatMap(CoreApi, (api) => api.getAllAnimeSeries)
```

改為：
```typescript
Effect.flatMap(CoreApi, (api) => api.getAllAnimeSeries({ excludeEmpty: true }))
```

**Step 2: 更新 `AnimeDialog.tsx`**

```bash
grep -n "getAllAnimeSeries" /workspace/frontend/src/pages/anime/AnimeDialog.tsx
```

找到：
```typescript
Effect.flatMap(CoreApi, (api) => api.getAllAnimeSeries)
```

改為：
```typescript
Effect.flatMap(CoreApi, (api) => api.getAllAnimeSeries({ excludeEmpty: true }))
```

**Step 3: 更新 `ParsersPage.tsx`**

```bash
grep -n "getAllAnimeSeries" /workspace/frontend/src/pages/parsers/ParsersPage.tsx
```

找到：
```typescript
Effect.flatMap(CoreApi, (api) => api.getAllAnimeSeries)
```

改為：
```typescript
Effect.flatMap(CoreApi, (api) => api.getAllAnimeSeries({ excludeEmpty: true }))
```

**Step 4: 確認 TypeScript 型別無誤**

```bash
cd /workspace/frontend && npx tsc --noEmit 2>&1 | head -40
```

Expected: 無錯誤（或只有與此次無關的既有警告）。

**Step 5: Commit**

```bash
git add frontend/src/pages/anime-series/AnimeSeriesPage.tsx \
        frontend/src/pages/anime/AnimeDialog.tsx \
        frontend/src/pages/parsers/ParsersPage.tsx
git commit -m "feat(frontend): pass excludeEmpty:true to getAllAnimeSeries in all callers"
```
