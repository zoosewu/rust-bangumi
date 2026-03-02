# Downloader Priority & Subscription Preferred Downloader

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** (A) 讓使用者可透過 API 與前端 UI 調整 downloader 優先級；(B) 讓每個 subscription 可設定優先 downloader，分配時優先送給該 downloader，不支援才 fallback 到一般 cascade。

**Architecture:**
- (A) 後端：`service_modules` 表已有 `priority` 欄位，只需補充 repository 方法與 PATCH endpoint。前端新增 downloader 管理 section 允許調整優先級數字。
- (B) DB 加 `preferred_downloader_id` 到 `subscriptions`；dispatch 時先 JOIN 取得每條 link 的 preferred downloader，嘗試後再 fallback 到原有 cascade。

**Tech Stack:** Rust/Diesel/Axum（後端），React/Effect-ts（前端），PostgreSQL migration。

---

## 重要架構背景

### DB 關聯 (查 dispatch 時需要)
```
anime_links.raw_item_id → raw_anime_items.item_id
raw_anime_items.subscription_id → subscriptions.subscription_id
subscriptions.preferred_downloader_id → service_modules.module_id
```
schema.rs 已有 `diesel::joinable!(raw_anime_items -> subscriptions (subscription_id))` 和 `diesel::joinable!(anime_links -> raw_anime_items (raw_item_id))`，JOIN 路徑已通。

### 現有 dispatch 邏輯 (download_dispatch.rs:130-185)
目前：按 `download_type` 分組 → 找符合類型的 downloaders（priority DESC）→ cascade 所有 links。
新增後：每條 link 先查 preferred_downloader_id → Phase 1 按 preferred 分組嘗試 → rejected 進 Phase 2 一般 cascade。

### Frontend 模式
- Schema 定義在 `frontend/src/schemas/*.ts` 用 `effect` 的 `Schema.Struct`
- API 呼叫用 `CoreApi` Effect Tag（`frontend/src/services/CoreApi.ts`）
- 前端無法直接 import，需對照實作 `CoreApiLive`（`frontend/src/services/CoreApiLive.ts`）

---

## Task 1: DB Migration — subscriptions 加 preferred_downloader_id

**Files:**
- Create: `core-service/migrations/2026-03-02-000000-subscription-preferred-downloader/up.sql`
- Create: `core-service/migrations/2026-03-02-000000-subscription-preferred-downloader/down.sql`

**Step 1: 建立 migration 檔案**

`up.sql`:
```sql
ALTER TABLE subscriptions
ADD COLUMN preferred_downloader_id INTEGER REFERENCES service_modules(module_id) ON DELETE SET NULL;
```

`down.sql`:
```sql
ALTER TABLE subscriptions DROP COLUMN IF EXISTS preferred_downloader_id;
```

**Step 2: 執行 migration（在 core-service 目錄）**

```bash
cd /workspace/core-service
diesel migration run
```

預期輸出：`Running migration 2026-03-02-000000-subscription-preferred-downloader`

**Step 3: 重新產生 schema.rs**

```bash
diesel print-schema > src/schema.rs
```

確認 `src/schema.rs` 的 `subscriptions` 表新增了：
```rust
preferred_downloader_id -> Nullable<Int4>,
```

**Step 4: Commit**

```bash
git add core-service/migrations/2026-03-02-000000-subscription-preferred-downloader/
git add core-service/src/schema.rs
git commit -m "feat(db): add preferred_downloader_id to subscriptions"
```

---

## Task 2: 後端 Model 與 DTO 更新

**Files:**
- Modify: `core-service/src/models/db.rs`
- Modify: `core-service/src/handlers/subscriptions.rs`

### Step 1: 更新 `Subscription` 與 `NewSubscription` model

在 `core-service/src/models/db.rs` 的 `Subscription` 結構加欄位：

```rust
// Subscription struct (Queryable) — 在 updated_at 後加：
pub preferred_downloader_id: Option<i32>,
```

`NewSubscription` struct 同樣加：
```rust
pub preferred_downloader_id: Option<i32>,
```

### Step 2: 更新 `SubscriptionResponse` DTO

在 `core-service/src/handlers/subscriptions.rs` 的 `SubscriptionResponse` 加：
```rust
pub preferred_downloader_id: Option<i32>,
```

並在所有 `SubscriptionResponse { ... }` 的建構處補上：
```rust
preferred_downloader_id: subscription.preferred_downloader_id,
```

### Step 3: 更新 `UpdateSubscriptionRequest` DTO

```rust
#[derive(Debug, serde::Deserialize)]
pub struct UpdateSubscriptionRequest {
    pub name: Option<String>,
    pub fetch_interval_minutes: Option<i32>,
    pub is_active: Option<bool>,
    // Option<Option<i32>>: 外層 None 表示不更新，內層 None 表示清除
    pub preferred_downloader_id: Option<Option<i32>>,
}
```

### Step 4: 更新 `CreateSubscriptionRequest` DTO

```rust
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct CreateSubscriptionRequest {
    // ... 原有欄位 ...
    pub preferred_downloader_id: Option<i32>,  // 新增
}
```

### Step 5: 更新 `update_subscription` handler 以處理新欄位

找到 `update_subscription` handler 中執行 diesel update 的部分，加入 preferred_downloader_id 的更新邏輯。由於 Diesel 不直接支援條件式 update builder，用 `diesel::sql_query` 或讀後改寫方式處理：

```rust
// 在 update 邏輯中加入
if let Some(pref_dl) = &payload.preferred_downloader_id {
    // Some(Some(id)) → 設定；Some(None) → 清除
    diesel::update(subscriptions::table.find(id))
        .set(subscriptions::preferred_downloader_id.eq(pref_dl))
        .execute(&mut conn)?;
}
```

### Step 6: 更新 `create_subscription` handler

在建立 `NewSubscription` 時加入：
```rust
preferred_downloader_id: payload.preferred_downloader_id,
```

### Step 7: 確認編譯

```bash
cd /workspace/core-service
cargo check 2>&1 | head -50
```

預期：無 error（warning 可接受）

**Step 8: Commit**

```bash
git add core-service/src/models/db.rs core-service/src/handlers/subscriptions.rs
git commit -m "feat(core): add preferred_downloader_id to subscription model and DTOs"
```

---

## Task 3: 後端 — Downloader Priority API

**Files:**
- Modify: `core-service/src/handlers/services.rs`
- Modify: `core-service/src/main.rs`

### Step 1: 在 services.rs 加新 handler

新增 `UpdateServiceRequest` DTO 和 `update_service` handler：

```rust
#[derive(Debug, serde::Deserialize)]
pub struct UpdateServiceRequest {
    pub priority: Option<i32>,
    pub is_enabled: Option<bool>,
}

/// Update service module (priority, is_enabled)
pub async fn update_service(
    State(state): State<AppState>,
    Path(service_id): Path<i32>,
    Json(payload): Json<UpdateServiceRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    use crate::schema::service_modules;

    let Ok(mut conn) = state.db.get() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "db_error"})));
    };

    let now = chrono::Utc::now().naive_utc();

    // Build update dynamically
    let mut updated = false;

    if let Some(priority) = payload.priority {
        if diesel::update(service_modules::table.find(service_id))
            .set((service_modules::priority.eq(priority), service_modules::updated_at.eq(now)))
            .execute(&mut conn)
            .is_ok()
        {
            updated = true;
        }
    }

    if let Some(is_enabled) = payload.is_enabled {
        if diesel::update(service_modules::table.find(service_id))
            .set((service_modules::is_enabled.eq(is_enabled), service_modules::updated_at.eq(now)))
            .execute(&mut conn)
            .is_ok()
        {
            updated = true;
        }
    }

    if updated {
        // Return updated record
        match service_modules::table
            .find(service_id)
            .first::<crate::models::ServiceModule>(&mut conn)
        {
            Ok(module) => (StatusCode::OK, Json(json!({
                "module_id": module.module_id,
                "name": module.name,
                "module_type": module.module_type.to_string(),
                "priority": module.priority,
                "is_enabled": module.is_enabled,
                "base_url": module.base_url,
                "updated_at": module.updated_at,
            }))),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
        }
    } else {
        (StatusCode::BAD_REQUEST, Json(json!({"error": "no fields to update"})))
    }
}
```

同時新增 `list_downloader_modules` handler（回傳 DB 中的 downloader 模組，帶 priority 資訊）：

```rust
/// List downloader service modules from DB (with priority info)
pub async fn list_downloader_modules(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    use crate::schema::service_modules;
    use crate::models::{ModuleTypeEnum, ServiceModule};

    let Ok(mut conn) = state.db.get() else {
        return Json(json!({"modules": []}));
    };

    match service_modules::table
        .filter(service_modules::module_type.eq(ModuleTypeEnum::Downloader))
        .filter(service_modules::is_enabled.eq(true))
        .order(service_modules::priority.desc())
        .load::<ServiceModule>(&mut conn)
    {
        Ok(modules) => {
            let result: Vec<serde_json::Value> = modules.iter().map(|m| json!({
                "module_id": m.module_id,
                "name": m.name,
                "module_type": m.module_type.to_string(),
                "priority": m.priority,
                "is_enabled": m.is_enabled,
                "base_url": m.base_url,
                "description": m.description,
                "updated_at": m.updated_at,
            })).collect();
            Json(json!({"modules": result}))
        }
        Err(e) => {
            tracing::error!("Failed to list downloader modules: {}", e);
            Json(json!({"modules": []}))
        }
    }
}
```

### Step 2: 在 main.rs 加路由

找到 services 相關路由區塊，加入：
```rust
.route("/services/downloader-modules", get(handlers::services::list_downloader_modules))
.route("/services/:service_id/update", patch(handlers::services::update_service))
```

> 注意：`/services/:service_id` 的 `:service_id` 在現有路由中是 UUID（用於 health_check）。新的 update 用整數 module_id，路徑改用 `/services/:service_id/update` 以避免型別衝突。

### Step 3: 確認編譯

```bash
cd /workspace/core-service
cargo check 2>&1 | head -50
```

**Step 4: Commit**

```bash
git add core-service/src/handlers/services.rs core-service/src/main.rs
git commit -m "feat(core): add downloader module list and priority update endpoints"
```

---

## Task 4: 後端 — Dispatch 邏輯加入 Preferred Downloader

**Files:**
- Modify: `core-service/src/services/download_dispatch.rs`

### 背景

`dispatch_new_links` 在 load links 後，需額外查詢每條 link 對應的 subscription preferred_downloader_id。然後在 cascade loop 前將 links 分為「有 preferred」和「無 preferred」兩批。

### Step 1: 在 dispatch_new_links 加 preferred downloader map 查詢

在 load `links` 之後（第 ~82 行，links 非空確認後），加入：

```rust
use crate::schema::{raw_anime_items, subscriptions as subscriptions_schema};

// Build map: link_id → preferred_downloader_id (from subscription via raw_anime_items)
let link_preferred_map: std::collections::HashMap<i32, i32> = {
    let result: Vec<(i32, i32)> = raw_anime_items::table
        .inner_join(
            anime_links::table
                .on(anime_links::raw_item_id.eq(raw_anime_items::item_id.nullable())),
        )
        .inner_join(
            subscriptions_schema::table
                .on(subscriptions_schema::subscription_id.eq(raw_anime_items::subscription_id)),
        )
        .filter(anime_links::link_id.eq_any(&candidate_link_ids))
        .filter(subscriptions_schema::preferred_downloader_id.is_not_null())
        .select((
            anime_links::link_id,
            subscriptions_schema::preferred_downloader_id.assume_not_null(),
        ))
        .load::<(i32, i32)>(&mut conn)
        .unwrap_or_default();
    result.into_iter().collect()
};
```

> `candidate_link_ids` 已在現有程式碼的 active download check 中定義（第 ~61 行）。

### Step 2: 在 cascade loop 中加入 preferred downloader 邏輯

將現有的 cascade loop（第 ~130-185 行）重構為：

```rust
for (download_type, type_links) in groups {
    let downloaders = self.find_capable_downloaders(&mut conn, &download_type)?;

    if downloaders.is_empty() {
        // 原有的 no_downloader 處理，不變
        for link in &type_links { ... }
        total_no_downloader += type_links.len();
        continue;
    }

    // 建立 capable downloader ID set，供快速查詢
    let capable_ids: std::collections::HashSet<i32> =
        downloaders.iter().map(|d| d.module_id).collect();

    // Phase 1: 有效的 preferred downloader 分組
    let mut cascade_pending: Vec<&AnimeLink> = Vec::new();
    let mut by_preferred: std::collections::HashMap<i32, Vec<&AnimeLink>> = std::collections::HashMap::new();

    for link in &type_links {
        match link_preferred_map.get(&link.link_id) {
            Some(&pref_id) if capable_ids.contains(&pref_id) => {
                by_preferred.entry(pref_id).or_default().push(link);
            }
            _ => cascade_pending.push(link),
        }
    }

    // 先處理各個 preferred downloader 分組
    for (pref_id, pref_links) in &by_preferred {
        let pref_dl = downloaders.iter().find(|d| d.module_id == *pref_id).unwrap();
        let items: Vec<DownloadRequestItem> = pref_links
            .iter()
            .map(|link| DownloadRequestItem {
                url: link.url.clone(),
                save_path: "/downloads".to_string(),
            })
            .collect();

        let download_url = format!("{}/downloads", pref_dl.base_url);
        match self.send_batch_to_downloader(&download_url, items).await {
            Ok(response) => {
                for (i, result) in response.results.iter().enumerate() {
                    if i >= pref_links.len() { break; }
                    let link = pref_links[i];
                    if result.status == "accepted" {
                        self.create_download_record(
                            &mut conn, link.link_id, &download_type,
                            "downloading", Some(pref_dl.module_id), result.hash.as_deref(),
                        )?;
                        total_dispatched += 1;
                    } else {
                        cascade_pending.push(link); // fallback to cascade
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    "Preferred downloader {} failed for {} links: {}",
                    pref_dl.name, pref_links.len(), e
                );
                cascade_pending.extend(pref_links.iter().copied());
            }
        }
    }

    // Phase 2: 一般 cascade（原有邏輯，對象改為 cascade_pending）
    let mut pending_links = cascade_pending;
    for downloader in &downloaders {
        if pending_links.is_empty() { break; }
        // ... 原有的 cascade loop 內容，對象為 pending_links ...
    }

    // 最後仍未分配的 → failed
    for link in &pending_links { ... }
}
```

### Step 3: 確認編譯

```bash
cd /workspace/core-service
cargo build 2>&1 | grep -E "error|warning: unused" | head -30
```

### Step 4: Commit

```bash
git add core-service/src/services/download_dispatch.rs
git commit -m "feat(core): dispatch respects subscription preferred_downloader_id"
```

---

## Task 5: 前端 — Schema 與 CoreApi 更新

**Files:**
- Modify: `frontend/src/schemas/subscription.ts`
- Create: `frontend/src/schemas/service-module.ts`
- Modify: `frontend/src/services/CoreApi.ts`
- Modify: `frontend/src/services/CoreApiLive.ts`（需確認此檔案路徑）

### Step 1: 更新 Subscription schema

```typescript
// frontend/src/schemas/subscription.ts
export const Subscription = Schema.Struct({
  subscription_id: Schema.Number,
  fetcher_id: Schema.Number,
  source_url: Schema.String,
  name: Schema.NullOr(Schema.String),
  description: Schema.NullOr(Schema.String),
  last_fetched_at: Schema.NullOr(Schema.String),
  next_fetch_at: Schema.NullOr(Schema.String),
  fetch_interval_minutes: Schema.Number,
  is_active: Schema.Boolean,
  preferred_downloader_id: Schema.NullOr(Schema.Number),  // 新增
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type Subscription = typeof Subscription.Type
```

### Step 2: 建立 ServiceModule schema

```typescript
// frontend/src/schemas/service-module.ts
import { Schema } from "effect"

export const ServiceModule = Schema.Struct({
  module_id: Schema.Number,
  name: Schema.String,
  module_type: Schema.String,
  priority: Schema.Number,
  is_enabled: Schema.Boolean,
  base_url: Schema.String,
  description: Schema.NullOr(Schema.String),
  updated_at: Schema.String,
})
export type ServiceModule = typeof ServiceModule.Type
```

### Step 3: 更新 CoreApi interface

在 `CoreApi` 的 Effect Tag 定義中加入：

```typescript
// 新增 import
import type { ServiceModule } from "@/schemas/service-module"

// 在 CoreApi 的型別定義加入：
readonly getDownloaderModules: Effect.Effect<readonly ServiceModule[]>
readonly updateServiceModule: (id: number, req: { priority?: number; is_enabled?: boolean }) => Effect.Effect<ServiceModule>
readonly createSubscription: (req: {
  source_url: string
  name?: string
  fetch_interval_minutes?: number
  preferred_downloader_id?: number | null  // 更新
}) => Effect.Effect<Subscription>
readonly updateSubscription: (id: number, req: {
  name?: string
  fetch_interval_minutes?: number
  is_active?: boolean
  preferred_downloader_id?: number | null  // 更新
}) => Effect.Effect<Subscription>
```

### Step 4: 更新 CoreApiLive 實作

找到 CoreApiLive（通常在 `frontend/src/services/CoreApiLive.ts` 或類似路徑）加入對應實作：

```typescript
getDownloaderModules: pipe(
  fetch(`${BASE_URL}/services/downloader-modules`),
  Effect.flatMap(r => r.json()),
  Effect.map(data => data.modules),
  Effect.flatMap(Schema.decodeUnknown(Schema.Array(ServiceModule))),
),
updateServiceModule: (id, req) => pipe(
  fetch(`${BASE_URL}/services/${id}/update`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
  }),
  Effect.flatMap(r => r.json()),
  Effect.flatMap(Schema.decodeUnknown(ServiceModule)),
),
```

（createSubscription / updateSubscription 的 body 型別已更新，無需其他改動）

### Step 5: 確認前端型別無誤

```bash
cd /workspace/frontend
npx tsc --noEmit 2>&1 | head -30
```

**Step 6: Commit**

```bash
git add frontend/src/schemas/
git add frontend/src/services/CoreApi.ts frontend/src/services/CoreApiLive.ts
git commit -m "feat(frontend): add ServiceModule schema, downloader API methods, subscription preferred_downloader_id"
```

---

## Task 6: 前端 — Downloader Priority 管理 UI

**Files:**
- Modify: `frontend/src/pages/subscriptions/SubscriptionsPage.tsx` 或新增獨立頁面

考量到下載器管理是全域設定，建議新增在 Dashboard 頁或獨立的 `ServicesPage`。最簡單做法：在 `Dashboard.tsx` 或導覽列已有的位置，加一個 downloader 列表讓使用者直接調整優先級數字。

### Step 1: 新增 DownloaderPrioritySection 元件

```typescript
// frontend/src/components/shared/DownloaderPrioritySection.tsx
import { useState } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { toast } from "sonner"
import type { ServiceModule } from "@/schemas/service-module"

export function DownloaderPrioritySection() {
  const { data: modules, refetch } = useEffectQuery(
    () => Effect.gen(function* () {
      const api = yield* CoreApi
      return yield* api.getDownloaderModules
    }),
    [],
  )

  const { mutate: doUpdate } = useEffectMutation(
    ({ id, priority }: { id: number; priority: number }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.updateServiceModule(id, { priority })
      }),
  )

  const [drafts, setDrafts] = useState<Record<number, number>>({})

  const handleSave = (module: ServiceModule) => {
    const priority = drafts[module.module_id] ?? module.priority
    doUpdate({ id: module.module_id, priority }).then(() => {
      toast.success(`${module.name} 優先級已更新為 ${priority}`)
      refetch()
    })
  }

  return (
    <div className="space-y-2">
      <h3 className="text-sm font-medium">Downloader 優先級</h3>
      <p className="text-xs text-muted-foreground">數字越大優先級越高（預設 50）</p>
      {(modules ?? []).map((m) => (
        <div key={m.module_id} className="flex items-center gap-3">
          <span className="text-sm flex-1">{m.name}</span>
          <Input
            type="number"
            className="w-20 h-7 text-sm"
            defaultValue={m.priority}
            onChange={(e) =>
              setDrafts((d) => ({ ...d, [m.module_id]: Number(e.target.value) }))
            }
          />
          <Button size="sm" variant="outline" className="h-7" onClick={() => handleSave(m)}>
            儲存
          </Button>
        </div>
      ))}
    </div>
  )
}
```

### Step 2: 將此元件加入適當頁面

在 Dashboard 或設定頁找到合適的位置加入：
```tsx
import { DownloaderPrioritySection } from "@/components/shared/DownloaderPrioritySection"
// 在 JSX 中加入
<DownloaderPrioritySection />
```

**Step 3: Commit**

```bash
git add frontend/src/components/shared/DownloaderPrioritySection.tsx
git add frontend/src/pages/  # 被修改的頁面
git commit -m "feat(frontend): add downloader priority management UI"
```

---

## Task 7: 前端 — 訂閱表單加入 Preferred Downloader 選擇器

**Files:**
- Modify: `frontend/src/pages/subscriptions/SubscriptionsPage.tsx`
- Modify: `frontend/src/pages/subscriptions/SubscriptionDialog.tsx`

### Step 1: 在 Create 對話框加入 downloader 選擇器

在 `SubscriptionsPage.tsx` 中：

1. 加入 downloader 清單查詢（與 Task 6 相同的 `useEffectQuery`）
2. 在 `createOpen` 的 state 旁加 `newPreferredDl`：`const [newPreferredDl, setNewPreferredDl] = useState<number | null>(null)`
3. 在 Create Dialog 的表單加入 Select 元件：

```tsx
<div className="space-y-2">
  <Label>優先 Downloader（可選）</Label>
  <select
    className="w-full text-sm border rounded px-2 py-1"
    value={newPreferredDl ?? ""}
    onChange={(e) => setNewPreferredDl(e.target.value ? Number(e.target.value) : null)}
  >
    <option value="">無（使用全域優先級）</option>
    {(downloaderModules ?? []).map((m) => (
      <option key={m.module_id} value={m.module_id}>{m.name}</option>
    ))}
  </select>
</div>
```

4. 在 createSubscription 呼叫加入 `preferred_downloader_id: newPreferredDl`

### Step 2: 在 SubscriptionDialog 編輯模式加入 downloader 選擇器

在 `SubscriptionDialog.tsx` 中：

1. 在 `editForm` state 加 `preferred_downloader_id: subscription.preferred_downloader_id ?? null`
2. 在頁面頂部加 downloader 清單查詢
3. 在 editing 模式的欄位中加入 Select 元件（同上）
4. `handleSave` 時傳入 `preferred_downloader_id: editForm.preferred_downloader_id`
5. 非編輯模式顯示 preferred_downloader 名稱（從 module 清單查名稱）

### Step 3: 確認型別與編譯

```bash
cd /workspace/frontend
npx tsc --noEmit 2>&1 | head -30
```

**Step 4: Commit**

```bash
git add frontend/src/pages/subscriptions/
git commit -m "feat(frontend): subscription forms support preferred_downloader_id selection"
```

---

## 測試驗證清單

### 後端

```bash
# 1. 測試 downloader 模組列表
curl http://localhost:8000/services/downloader-modules

# 2. 測試更新 priority（service_id 換成實際 module_id）
curl -X PATCH http://localhost:8000/services/1/update \
  -H "Content-Type: application/json" \
  -d '{"priority": 80}'

# 3. 測試建立有 preferred_downloader_id 的訂閱
curl -X POST http://localhost:8000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{"source_url": "https://...", "preferred_downloader_id": 1}'

# 4. 測試更新訂閱的 preferred_downloader_id
curl -X PATCH http://localhost:8000/subscriptions/1 \
  -H "Content-Type: application/json" \
  -d '{"preferred_downloader_id": 1}'

# 5. 測試清除 preferred_downloader_id
curl -X PATCH http://localhost:8000/subscriptions/1 \
  -H "Content-Type: application/json" \
  -d '{"preferred_downloader_id": null}'
```

### 前端

1. 開啟 Downloader 管理 UI → 修改優先級 → 儲存 → 確認數字更新
2. 新增訂閱時選擇 preferred downloader → 建立後確認 subscription 有設定值
3. 編輯訂閱 → 更改 preferred downloader → 儲存 → 重新開啟確認顯示正確名稱
4. 清除 preferred downloader → 儲存 → 確認顯示「無」

### Dispatch 邏輯

1. 建立一個有 preferred_downloader_id 的訂閱
2. 新增 raw item → 觸發 dispatch
3. 確認 `downloads` 表中 `module_id` 為 preferred downloader 的 ID
4. 停用 preferred downloader → 再次 dispatch → 確認 fallback 到下一個 capable downloader

---

## 注意事項

1. **schema.rs 是 auto-generated**：執行 `diesel migration run` 後必須 `diesel print-schema > src/schema.rs` 重新產生，不要手動編輯。

2. **UPSERT 不覆蓋 priority**：`services.rs` 的 register UPSERT 故意不更新 `priority`，這樣手動調整的優先級在服務重啟後不會被重置。如果目前 UPSERT 有覆蓋 priority，需要從 `ON CONFLICT DO UPDATE SET` 中移除 `priority` 欄位。

3. **`preferred_downloader_id: Option<Option<i32>>`**：UpdateSubscriptionRequest 中用雙層 Option，外層 None = 不改、`Some(None)` = 清除、`Some(Some(id))` = 設定。前端傳 `null` JSON 值會 deserialize 為 `Some(None)`。

4. **Route 型別衝突**：現有 `/services/:service_id` 的 `:service_id` 型別是 UUID（用在 health_check），新的 update endpoint 用整數 module_id，故取路徑 `/services/:service_id/update` 避免衝突。
