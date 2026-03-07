# 訂閱精靈：返回按鈕、取消清理、同步爬取 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 新增返回按鈕、取消時刪除訂閱、Step 1 完成後同步等待爬取完成再進 Step 2。

**Architecture:** 方案 A——Step 1 仍立即建立訂閱；返回/取消時呼叫 DELETE 清理；後端新增 `POST /subscriptions/:id/fetch` 同步觸發爬取，前端 Step 1 完成後等待此端點回應再切換到 Step 2。

**Tech Stack:** Rust/Axum（後端）、React/Effect（前端）

---

### Task 1: 後端——新增 `POST /subscriptions/:id/fetch` handler

**Files:**
- Modify: `core-service/src/handlers/subscriptions.rs`（在檔案尾端新增 handler）
- Modify: `core-service/src/main.rs`（新增路由）

**Step 1: 在 `subscriptions.rs` 尾端新增 handler**

在 `trigger_immediate_fetch` 函數之後（目前第 1099 行之後）新增：

```rust
/// 同步觸發訂閱爬取（等待完成後才回應）
pub async fn trigger_fetch_now(
    State(state): State<AppState>,
    Path(subscription_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "db_error", "message": e.to_string() })),
            )
        }
    };

    // 查出訂閱的 source_url 與 fetcher_id
    let sub = match subscriptions::table
        .filter(subscriptions::subscription_id.eq(subscription_id))
        .filter(subscriptions::is_active.eq(true))
        .select(Subscription::as_select())
        .first::<Subscription>(&mut conn)
    {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "not_found", "message": "Subscription not found" })),
            )
        }
    };

    drop(conn);

    match trigger_immediate_fetch(&state.db, sub.subscription_id, &sub.source_url, sub.fetcher_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "subscription_id": subscription_id,
                "message": "Fetch completed"
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "fetch_failed", "message": e })),
        ),
    }
}
```

**Step 2: 在 `main.rs` 新增路由**

找到現有的 `/subscriptions/:id` 路由（約第 192-196 行）：
```rust
.route(
    "/subscriptions/:id",
    delete(handlers::subscriptions::delete_subscription)
        .patch(handlers::subscriptions::update_subscription),
)
```

在它之後插入：
```rust
.route(
    "/subscriptions/:id/fetch",
    post(handlers::subscriptions::trigger_fetch_now),
)
```

**Step 3: 確認編譯通過**

```bash
cd /workspace
cargo check -p core-service 2>&1 | tail -20
```

Expected: 無 error（可能有 warnings，可忽略）

**Step 4: Commit**

```bash
cd /workspace
git add core-service/src/handlers/subscriptions.rs core-service/src/main.rs
git commit -m "feat(core): add POST /subscriptions/:id/fetch sync trigger endpoint"
```

---

### Task 2: 前端——CoreApi 新增 `triggerFetch` 方法

**Files:**
- Modify: `frontend/src/services/CoreApi.ts`（在 interface 中新增方法簽名）
- Modify: `frontend/src/layers/ApiLayer.ts`（在 implementation 中新增實作）

**Step 1: 在 `CoreApi.ts` 的 interface 中新增**

找到 `deleteSubscription` 那行（第 89 行）：
```typescript
readonly deleteSubscription: (id: number, purge?: boolean) => Effect.Effect<void>
```

在其後新增：
```typescript
readonly triggerFetch: (subscriptionId: number) => Effect.Effect<void>
```

**Step 2: 在 `ApiLayer.ts` 的 `deleteSubscription` 實作之後新增**

找到 `deleteSubscription` 實作（約第 281-284 行）：
```typescript
deleteSubscription: (id, purge) =>
  client
    .execute(HttpClientRequest.del(`/api/core/subscriptions/${id}${purge ? '?purge=true' : ''}`))
    .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),
```

在其後新增：
```typescript
triggerFetch: (subscriptionId) =>
  client
    .execute(HttpClientRequest.post(`/api/core/subscriptions/${subscriptionId}/fetch`))
    .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),
```

**Step 3: 確認前端 TypeScript 無型別錯誤**

```bash
cd /workspace/frontend
npx tsc --noEmit 2>&1 | head -30
```

Expected: 無 error

**Step 4: Commit**

```bash
cd /workspace
git add frontend/src/services/CoreApi.ts frontend/src/layers/ApiLayer.ts
git commit -m "feat(frontend): add triggerFetch to CoreApi"
```

---

### Task 3: 前端——修改 Step 1 的 handleCreate，同步等待爬取

**Files:**
- Modify: `frontend/src/pages/subscriptions/CreateSubscriptionWizard.tsx`

**背景：** 目前 `handleCreate` 呼叫 `createSub()` 後立即 `setStep(2)`。這時爬取是 fire-and-forget，Step 2 進去時可能還沒有資料。

目標：`createSub()` 成功後，等待 `triggerFetch()` 完成，再 `setStep(2)`。

**Step 1: 新增 `fetching` state 和 `triggerFetch` mutation**

在 `const [fetcherId, ...]` 那一組 state 宣告之後（約第 52 行附近），新增：

```typescript
const [fetching, setFetching] = useState(false)
```

在 `createSub` mutation 之後（約第 206 行），新增：

```typescript
const { mutate: runTriggerFetch } = useEffectMutation((subId: number) =>
  Effect.flatMap(CoreApi, (api) => api.triggerFetch(subId)),
)
```

**Step 2: 修改 `handleCreate`**

現有 `handleCreate`（第 208-215 行）：
```typescript
const handleCreate = () => {
  createSub().then((sub) => {
    if (sub) {
      setSubscriptionId(sub.subscription_id)
      setStep(2)
    }
  })
}
```

替換為：
```typescript
const handleCreate = () => {
  createSub().then(async (sub) => {
    if (sub) {
      setSubscriptionId(sub.subscription_id)
      setFetching(true)
      await runTriggerFetch(sub.subscription_id).catch(() => {})
      setFetching(false)
      setStep(2)
    }
  })
}
```

**Step 3: 修改 Step 1 的「建立訂閱」按鈕，顯示爬取中狀態**

找到 Step 1 footer（約第 383-393 行）：
```tsx
<Button onClick={handleCreate} disabled={!url.trim() || creating}>
  {creating && <Loader2 className="mr-1 size-4 animate-spin" />}
  建立訂閱
</Button>
```

替換為：
```tsx
<Button onClick={handleCreate} disabled={!url.trim() || creating || fetching}>
  {(creating || fetching) && <Loader2 className="mr-1 size-4 animate-spin" />}
  {fetching ? "爬取中..." : "建立訂閱"}
</Button>
```

**Step 4: 驗證**

```bash
cd /workspace/frontend
npx tsc --noEmit 2>&1 | head -20
```

Expected: 無 error

---

### Task 4: 前端——新增輔助函數 `goBackToStep1` 和 `closeAndCleanup`

**Files:**
- Modify: `frontend/src/pages/subscriptions/CreateSubscriptionWizard.tsx`

這兩個函數封裝「刪除訂閱 + 清理狀態」，供 Step 2/3 的返回/取消按鈕共用。

**Step 1: 在 `reset()` 函數之後新增兩個輔助函數**

在 `reset()` 函數（第 181-195 行）之後新增：

```typescript
// Step 2 返回 Step 1：刪除訂閱，保留表單內容
const goBackToStep1 = async () => {
  stopStep2Polling()
  if (subscriptionId !== undefined) {
    await AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) => api.deleteSubscription(subscriptionId)),
    ).catch(() => {})
  }
  setSubscriptionId(undefined)
  setRawItems([])
  setParserPendings([])
  setFilterPendings([])
  setStep2Polling(false)
  setStep(1)
}

// Step 2/3 取消：刪除訂閱，關閉 wizard
const closeAndCleanup = async () => {
  stopStep2Polling()
  stopStep3Polling()
  if (subscriptionId !== undefined) {
    await AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) => api.deleteSubscription(subscriptionId)),
    ).catch(() => {})
  }
  reset()
  onOpenChange(false)
}
```

**Step 2: 修改 `onOpenChange` handler（X 按鈕/外部點擊關閉）**

找到 Dialog 的 `onOpenChange`（約第 241-244 行）：
```tsx
onOpenChange={(v) => {
  if (!v) reset()
  onOpenChange(v)
}}
```

替換為：
```tsx
onOpenChange={(v) => {
  if (!v) {
    if (subscriptionId !== undefined) {
      // fire-and-forget 刪除，不阻塞關閉
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.deleteSubscription(subscriptionId)),
      ).catch(() => {})
    }
    reset()
    onOpenChange(false)
  } else {
    onOpenChange(true)
  }
}}
```

**Step 3: 驗證 TypeScript**

```bash
cd /workspace/frontend
npx tsc --noEmit 2>&1 | head -20
```

---

### Task 5: 前端——Step 2/3 新增返回與取消按鈕

**Files:**
- Modify: `frontend/src/pages/subscriptions/CreateSubscriptionWizard.tsx`

**Step 1: 替換 Step 2 footer**

找到 Step 2 footer（約第 395-399 行）：
```tsx
{step === 2 && (
  <Button onClick={() => setStep(3)} disabled={!step2NextEnabled}>
    下一步
  </Button>
)}
```

替換為：
```tsx
{step === 2 && (
  <>
    <Button variant="outline" onClick={closeAndCleanup}>
      取消
    </Button>
    <Button variant="outline" onClick={goBackToStep1}>
      返回
    </Button>
    <Button onClick={() => setStep(3)} disabled={!step2NextEnabled}>
      下一步
    </Button>
  </>
)}
```

**Step 2: 替換 Step 3 footer**

找到 Step 3 footer（約第 401-413 行）：
```tsx
{step === 3 && (
  <Button
    onClick={() => {
      onCreated?.()
      onOpenChange(false)
      reset()
    }}
    disabled={!step3DoneEnabled}
  >
    完成
  </Button>
)}
```

替換為：
```tsx
{step === 3 && (
  <>
    <Button variant="outline" onClick={closeAndCleanup}>
      取消
    </Button>
    <Button
      variant="outline"
      onClick={() => {
        stopStep3Polling()
        setStep(2)
      }}
    >
      返回
    </Button>
    <Button
      onClick={() => {
        onCreated?.()
        onOpenChange(false)
        reset()
      }}
      disabled={!step3DoneEnabled}
    >
      完成
    </Button>
  </>
)}
```

**Step 3: 最終 TypeScript 驗證**

```bash
cd /workspace/frontend
npx tsc --noEmit 2>&1 | head -30
```

Expected: 無 error

**Step 4: Commit**

```bash
cd /workspace
git add frontend/src/pages/subscriptions/CreateSubscriptionWizard.tsx
git commit -m "feat(wizard): add back button, cancel cleanup, sync fetch on step1"
```

---

## 完成驗收清單

- [ ] `POST /subscriptions/:id/fetch` 可以回應 200
- [ ] Step 1 按「建立訂閱」後顯示「爬取中...」直到 fetch 完成再進 Step 2
- [ ] Step 2 有「返回」按鈕，點擊後：訂閱被刪除、回到 Step 1、表單保留原值
- [ ] Step 3 有「返回」按鈕，點擊後：回到 Step 2、重新開始 Step 2 輪詢
- [ ] Step 2/3 的「取消」按鈕：刪除訂閱後關閉
- [ ] X 按鈕關閉 wizard（在 Step 2/3 時）：fire-and-forget 刪除訂閱
- [ ] `cargo check -p core-service` 無 error
- [ ] `npx tsc --noEmit` 無 error
