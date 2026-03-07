# 三步驟訂閱建立 Wizard 實作計劃

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 將新增訂閱從 2 步驟改為 3 步驟 Wizard，包含 RSS 抓取預覽、AI Parser 確認、Conflict Filter 確認，並讓所有待確認項目的 AiResultPanel 同時顯示固定 Prompt 與自訂 Prompt 供編輯後重新生成。

**Architecture:**
- **Backend**：`pending_ai_results` 加 `subscription_id` 欄位，讓 parser/filter 結果都可以按訂閱查詢；`rerun_unmatched_raw_items` 完成後觸發 conflict detection，確保 Step 3 能拿到 filter pending results；`RegenerateRequest` 加 `fixed_prompt` 覆蓋支援。
- **Frontend**：`AiResultPanel` 新增固定 Prompt 可編輯欄位（全局通用）；Wizard 改為 3 步驟，Step 2 輪詢 raw items + parser pending，Step 3 輪詢 filter pending；新增共用 `WizardPendingList` 元件供 Step 2/3 使用。

**Tech Stack:** Rust/Axum/Diesel (backend), React/TypeScript/Effect.ts/shadcn-ui (frontend)

---

## Phase 1: Backend DB 與模型

### Task 1: Migration — 新增 subscription_id 至 pending_ai_results

**Files:**
- Create: `core-service/migrations/2026-03-05-000000-pending-ai-subscription-id/up.sql`
- Create: `core-service/migrations/2026-03-05-000000-pending-ai-subscription-id/down.sql`

**Step 1: 建立 migration 目錄**

```bash
cd core-service
diesel migration generate pending-ai-subscription-id
```

**Step 2: 編輯 up.sql**

```sql
ALTER TABLE pending_ai_results
    ADD COLUMN subscription_id INT REFERENCES subscriptions(subscription_id) ON DELETE SET NULL;

CREATE INDEX idx_pending_ai_results_subscription_id
    ON pending_ai_results(subscription_id);
```

**Step 3: 編輯 down.sql**

```sql
DROP INDEX IF EXISTS idx_pending_ai_results_subscription_id;
ALTER TABLE pending_ai_results DROP COLUMN IF EXISTS subscription_id;
```

**Step 4: 執行 migration**

```bash
cd core-service
diesel migration run
```
Expected: `Running migration 2026-03-05-000000-pending-ai-subscription-id`

**Step 5: Commit**

```bash
git add core-service/migrations/2026-03-05-000000-pending-ai-subscription-id/
git commit -m "feat(db): add subscription_id to pending_ai_results"
```

---

### Task 2: 更新 schema.rs 與 models/db.rs

**Files:**
- Modify: `core-service/src/schema.rs`
- Modify: `core-service/src/models/db.rs`

**Step 1: 在 schema.rs 的 pending_ai_results 表定義末尾加入新欄位**

在 `pending_ai_results` 表的 `expires_at` 行之後、`created_at` 行之前加入：
```rust
        subscription_id    -> Nullable<Int4>,
```

**Step 2: 在 models/db.rs 的 PendingAiResult struct 末尾加入**

```rust
pub subscription_id: Option<i32>,
```

在 `NewPendingAiResult` struct 末尾同樣加入：

```rust
pub subscription_id: Option<i32>,
```

**Step 3: 確認編譯**

```bash
cd core-service && cargo check 2>&1 | head -30
```
Expected: 0 errors

**Step 4: Commit**

```bash
git add core-service/src/schema.rs core-service/src/models/db.rs
git commit -m "feat(models): add subscription_id to PendingAiResult"
```

---

## Phase 2: Backend AI 生成器更新

### Task 3: 更新 generate_parser_for_title — 自動帶入 subscription_id

**Files:**
- Modify: `core-service/src/ai/parser_generator.rs`

**Step 1: 新增 import**

在 `parser_generator.rs` 頂部的 use 宣告區加入：
```rust
use crate::schema::raw_anime_items;
```

**Step 2: 在 `generate_parser_for_title` 函式中，建立 pending record 之前，從 raw_item_id 查詢 subscription_id**

在取得 `(fixed_prompt, custom_prompt)` 之後、`建立 pending record` 之前插入：

```rust
    // 從 raw_item_id 查詢所屬 subscription_id
    let subscription_id: Option<i32> = raw_item_id.and_then(|rid| {
        let mut conn = pool.get().ok()?;
        raw_anime_items::table
            .filter(raw_anime_items::item_id.eq(rid))
            .select(raw_anime_items::subscription_id)
            .first::<i32>(&mut conn)
            .ok()
    });
```

**Step 3: 在 `NewPendingAiResult { ... }` 的 fields 末尾加入**

```rust
                subscription_id,
```

**Step 4: 確認編譯**

```bash
cd core-service && cargo check 2>&1 | head -30
```

**Step 5: Commit**

```bash
git add core-service/src/ai/parser_generator.rs
git commit -m "feat(ai): populate subscription_id in parser pending results"
```

---

### Task 4: 更新 generate_filter_for_conflict — 接受並儲存 subscription_id

**Files:**
- Modify: `core-service/src/ai/filter_generator.rs`
- Modify: `core-service/src/services/conflict_detection.rs`

**Step 1: 在 `generate_filter_for_conflict` 函式簽章加入 subscription_id 參數**

```rust
pub async fn generate_filter_for_conflict(
    pool: Arc<DbPool>,
    conflict_titles: Vec<String>,
    source_title: String,
    temp_custom_prompt: Option<String>,
    subscription_id: Option<i32>,   // 新增
) -> Result<PendingAiResult, String> {
```

**Step 2: 在 `NewPendingAiResult { ... }` 的 fields 末尾加入**

```rust
                subscription_id,
```

**Step 3: 更新 `conflict_detection.rs` 的呼叫處，從 links 查詢 subscription_id**

在 `conflict_detection.rs` 的 `generate_filter_for_conflict` 呼叫前加入查詢邏輯：

```rust
                // 從 links 的 raw_item_id 查詢 subscription_id
                let filter_sub_id: Option<i32> = {
                    use crate::schema::raw_anime_items;
                    let raw_id = links.iter().find_map(|l| l.raw_item_id);
                    raw_id.and_then(|rid| {
                        let mut conn = self.pool.get().ok()?;
                        raw_anime_items::table
                            .filter(raw_anime_items::item_id.eq(rid))
                            .select(raw_anime_items::subscription_id)
                            .first::<i32>(&mut conn)
                            .ok()
                    })
                };
```

然後修改 tokio::spawn 內的呼叫：

```rust
                    tokio::spawn(async move {
                        if let Err(e) = crate::ai::filter_generator::generate_filter_for_conflict(
                            pool_clone,
                            conflict_titles,
                            source,
                            None,
                            filter_sub_id,    // 新增
                        )
                        .await
                        {
                            tracing::warn!("AI filter 觸發失敗: {}", e);
                        }
                    });
```

**Step 4: 更新 `pending_ai_results.rs` 的 regenerate handler 中 filter 的呼叫**

在 `pending_ai_results.rs` 的 `regenerate_pending` 函式中，`"filter"` 分支的 `generate_filter_for_conflict` 呼叫加上最後一個 `None`（subscription_id，regenerate 時不變）：

```rust
        "filter" => {
            crate::ai::filter_generator::generate_filter_for_conflict(
                pool,
                vec![source_title.clone()],
                source_title,
                req.custom_prompt,
                None,   // subscription_id 在 regenerate 時從舊 record 讀取，暫時保留 None
            )
            .await
        }
```

**Step 5: 確認編譯**

```bash
cd core-service && cargo check 2>&1 | head -30
```

**Step 6: Commit**

```bash
git add core-service/src/ai/filter_generator.rs core-service/src/services/conflict_detection.rs core-service/src/handlers/pending_ai_results.rs
git commit -m "feat(ai): populate subscription_id in filter pending results"
```

---

## Phase 3: Backend API 更新

### Task 5: list_pending handler — 加入 subscription_id 過濾

**Files:**
- Modify: `core-service/src/handlers/pending_ai_results.rs`

**Step 1: 在 `ListPendingQuery` struct 加入 subscription_id**

```rust
#[derive(Debug, Deserialize)]
pub struct ListPendingQuery {
    pub result_type: Option<String>,
    pub status: Option<String>,
    pub subscription_id: Option<i32>,   // 新增
}
```

**Step 2: 在 `list_pending` 函式的 query builder 加入 subscription_id 過濾**

在現有的 `if let Some(s) = q.status { ... }` 之後加入：

```rust
    if let Some(sub_id) = q.subscription_id {
        query = query.filter(pending_ai_results::subscription_id.eq(sub_id));
    }
```

**Step 3: 確認編譯**

```bash
cd core-service && cargo check 2>&1 | head -30
```

**Step 4: Commit**

```bash
git add core-service/src/handlers/pending_ai_results.rs
git commit -m "feat(api): add subscription_id filter to list_pending endpoint"
```

---

### Task 6: 支援 fixed_prompt 臨時覆蓋（重新生成用）

**Files:**
- Modify: `core-service/src/ai/parser_generator.rs`
- Modify: `core-service/src/ai/filter_generator.rs`
- Modify: `core-service/src/handlers/pending_ai_results.rs`

**Step 1: 在 `generate_parser_for_title` 加入 `temp_fixed_prompt` 參數**

修改函式簽章：
```rust
pub async fn generate_parser_for_title(
    pool: Arc<DbPool>,
    source_title: String,
    raw_item_id: Option<i32>,
    temp_custom_prompt: Option<String>,
    temp_fixed_prompt: Option<String>,   // 新增
) -> Result<PendingAiResult, String> {
```

在取得 `fixed_prompt` 的邏輯之後加入覆蓋邏輯：

```rust
    // 若呼叫方提供臨時 fixed_prompt，以其覆蓋 DB 設定
    let fixed_prompt = temp_fixed_prompt.unwrap_or(fixed_prompt);
```

**Step 2: 在 `generate_filter_for_conflict` 加入 `temp_fixed_prompt` 參數**

修改函式簽章（在 `temp_custom_prompt` 之後）：
```rust
pub async fn generate_filter_for_conflict(
    pool: Arc<DbPool>,
    conflict_titles: Vec<String>,
    source_title: String,
    temp_custom_prompt: Option<String>,
    subscription_id: Option<i32>,
    temp_fixed_prompt: Option<String>,   // 新增
) -> Result<PendingAiResult, String> {
```

同樣在取得 `fixed_prompt` 後加入：
```rust
    let fixed_prompt = temp_fixed_prompt.unwrap_or(fixed_prompt);
```

**Step 3: 更新 `RegenerateRequest` struct 加入 `fixed_prompt`**

```rust
#[derive(Debug, Deserialize)]
pub struct RegenerateRequest {
    pub custom_prompt: Option<String>,
    pub fixed_prompt: Option<String>,   // 新增
}
```

**Step 4: 更新 `regenerate_pending` 中的兩個 generator 呼叫，傳入 `req.fixed_prompt`**

`"parser"` 分支：
```rust
        "parser" => {
            crate::ai::parser_generator::generate_parser_for_title(
                pool,
                source_title,
                None,
                req.custom_prompt,
                req.fixed_prompt,   // 新增
            )
            .await
        }
```

`"filter"` 分支：
```rust
        "filter" => {
            crate::ai::filter_generator::generate_filter_for_conflict(
                pool,
                vec![source_title.clone()],
                source_title,
                req.custom_prompt,
                None,
                req.fixed_prompt,   // 新增
            )
            .await
        }
```

**Step 5: 修正其他呼叫 `generate_parser_for_title` / `generate_filter_for_conflict` 的地方，補上新參數 `None`**

搜尋並修正：

```bash
cd core-service && grep -rn "generate_parser_for_title\|generate_filter_for_conflict" src/ | grep -v "\.rs:#\|//\|pub async fn"
```

所有呼叫處補上最後一個 `None`：
- `src/services/title_parser.rs`: `generate_parser_for_title(..., None, None)` — 最後兩個 None
- `src/handlers/pending_ai_results.rs` 的 `rerun_unmatched_raw_items` 中：補上 `None`
- `src/services/conflict_detection.rs`：補上 `None`（filter_generator 已有 subscription_id，再補 temp_fixed_prompt=None）

**Step 6: 確認編譯**

```bash
cd core-service && cargo check 2>&1 | head -30
```

**Step 7: Commit**

```bash
git add core-service/src/ai/parser_generator.rs core-service/src/ai/filter_generator.rs core-service/src/handlers/pending_ai_results.rs
git commit -m "feat(ai): support fixed_prompt override in regenerate endpoint"
```

---

### Task 7: rerun_unmatched_raw_items 後觸發 conflict detection

**Background:** 當 pending parser 被確認後，`rerun_unmatched_raw_items` 重新解析成功並建立新 anime_links，但目前沒有觸發 conflict detection。Wizard Step 3 需要 conflict detection 先跑完才能看到 filter pending results。

**Files:**
- Modify: `core-service/src/handlers/pending_ai_results.rs`
- Modify: `core-service/src/state.rs` (確認 conflict_detection 已在 AppState 中)

**Step 1: 查看 AppState 確認 conflict_detection 欄位**

```bash
grep -n "conflict_detection" core-service/src/state.rs | head -5
```

Expected: `pub conflict_detection: Arc<ConflictDetectionService>`

**Step 2: 更新 `confirm_pending` 函式的 `"parser"` 分支，在 tokio::spawn 中同時觸發 conflict detection**

找到目前的 tokio::spawn 區塊（`rerun_unmatched_raw_items` 的呼叫），修改為：

```rust
            // 觸發 re-run + conflict detection（背景非同步）
            let pool_arc = Arc::new(pool.clone());
            let conflict_detection = state.conflict_detection.clone();
            tokio::spawn(async move {
                if let Err(e) = rerun_unmatched_raw_items(pool_arc).await {
                    tracing::warn!("rerun_unmatched_raw_items 失敗: {}", e);
                }
                // 解析完成後重跑 conflict detection，觸發 AI filter 生成
                if let Err(e) = conflict_detection.detect_and_mark_conflicts().await {
                    tracing::warn!("conflict detection after parser confirm 失敗: {}", e);
                }
            });
```

**Step 3: 確認 `confirm_pending` 函式可以存取 `state`**

函式簽章應包含 `State(state): State<AppState>`，確認 `state` 在 match block 內仍然可用。如有 ownership 問題，先 clone 需要的欄位。

**Step 4: 確認編譯**

```bash
cd core-service && cargo check 2>&1 | head -30
```

**Step 5: Commit**

```bash
git add core-service/src/handlers/pending_ai_results.rs
git commit -m "fix(ai): trigger conflict detection after parser confirm to enable wizard step 3"
```

---

## Phase 4: Frontend 核心元件更新

### Task 8: 更新 schemas/ai.ts 與 CoreApi.ts / ApiLayer.ts

**Files:**
- Modify: `frontend/src/schemas/ai.ts`
- Modify: `frontend/src/services/CoreApi.ts`
- Modify: `frontend/src/layers/ApiLayer.ts`

**Step 1: 更新 `schemas/ai.ts`**

```typescript
// PendingAiResult 加入 subscription_id
export interface PendingAiResult {
  id: number
  result_type: "parser" | "filter"
  source_title: string
  generated_data: Record<string, unknown> | null
  status: "generating" | "pending" | "confirmed" | "failed"
  error_message: string | null
  raw_item_id: number | null
  subscription_id: number | null   // 新增
  used_fixed_prompt: string
  used_custom_prompt: string | null
  expires_at: string | null
  created_at: string
  updated_at: string
}

// RegenerateRequest 加入 fixed_prompt
export interface RegenerateRequest {
  custom_prompt?: string
  fixed_prompt?: string   // 新增
}
```

**Step 2: 更新 `CoreApi.ts` 的 `getPendingAiResults` 簽章**

```typescript
readonly getPendingAiResults: (params?: {
  result_type?: string
  status?: string
  subscription_id?: number   // 新增
}) => Effect.Effect<readonly PendingAiResult[]>
```

**同時新增 `getFetcherModules`：**

```typescript
readonly getFetcherModules: Effect.Effect<readonly ServiceModule[]>
```

**Step 3: 更新 `ApiLayer.ts`**

找到 `getPendingAiResults` 的實作（搜尋 `pending-ai-results`），修改 URL 建立邏輯：

```typescript
    getPendingAiResults: (params) => {
      const qs = new URLSearchParams()
      if (params?.result_type) qs.set("result_type", params.result_type)
      if (params?.status) qs.set("status", params.status)
      if (params?.subscription_id != null) qs.set("subscription_id", String(params.subscription_id))
      const url = `/api/core/pending-ai-results${qs.toString() ? `?${qs}` : ""}`
      return fetchJson(HttpClientRequest.get(url), Schema.Array(PendingAiResultSchema))
    },
```

> 注意：查看 ApiLayer.ts 現有的 `getPendingAiResults` 實作方式，若使用不同的 query param 構建方式則保持一致。

**新增 `getFetcherModules`：**

在 `getDownloaderModules` 實作之後加入：

```typescript
    getFetcherModules: fetchJson(
      HttpClientRequest.get("/api/core/fetcher-modules"),
      Schema.Struct({ modules: Schema.Array(ServiceModule) }),
    ).pipe(Effect.map((r) => r.modules)),
```

**Step 4: 確認前端可編譯（no TypeScript errors）**

```bash
cd frontend && npx tsc --noEmit 2>&1 | head -30
```

**Step 5: Commit**

```bash
git add frontend/src/schemas/ai.ts frontend/src/services/CoreApi.ts frontend/src/layers/ApiLayer.ts
git commit -m "feat(frontend): add subscription_id filter and fixed_prompt to pending API"
```

---

### Task 9: 更新 AiResultPanel — 加入固定 Prompt 可編輯欄位

**Files:**
- Modify: `frontend/src/components/shared/AiResultPanel.tsx`

**Background:** 目前 `AiResultPanel` 只有自訂 prompt 的 textarea。需要新增固定 prompt 的 textarea（初始值為 `result.used_fixed_prompt`），讓使用者可以臨時覆蓋後重新生成。兩個 textarea 並排或依序顯示。

**Step 1: 在 component state 加入 `tempFixedPrompt`**

在現有的 `const [tempPrompt, setTempPrompt] = useState("")` 之後加入：

```typescript
const [tempFixedPrompt, setTempFixedPrompt] = useState("")
```

**Step 2: 修改 `regenerate` mutation，傳入 `fixed_prompt`**

```typescript
  const { mutate: regenerate, isLoading: regenerating } = useEffectMutation(
    () =>
      Effect.flatMap(CoreApi, (api) =>
        api.regeneratePendingAiResult(result.id, {
          custom_prompt: tempPrompt || undefined,
          fixed_prompt: tempFixedPrompt || undefined,   // 新增
        }),
      ),
  )
```

**Step 3: 在「臨時自訂 Prompt」區塊之前，加入「固定 Prompt」區塊**

在 `{/* 臨時自訂 Prompt */}` 的 div 之前插入：

```tsx
      {/* 固定 Prompt（可臨時覆蓋） */}
      <div className="space-y-2">
        <Label className="text-sm">固定 Prompt（臨時覆蓋，不影響全局設定）</Label>
        <Textarea
          value={tempFixedPrompt || result.used_fixed_prompt}
          onChange={(e) => setTempFixedPrompt(e.target.value)}
          rows={4}
          className="text-sm font-mono text-xs"
        />
      </div>
```

**Step 4: 重置 state — 在 regenerate 成功後清除 tempFixedPrompt**

```typescript
          regenerate().then((updated) => {
            if (updated) {
              setTempPrompt("")
              setTempFixedPrompt("")   // 新增
              onRegenerated?.(updated)
            }
          })
```

**Step 5: TypeScript 確認**

```bash
cd frontend && npx tsc --noEmit 2>&1 | head -20
```

**Step 6: Commit**

```bash
git add frontend/src/components/shared/AiResultPanel.tsx
git commit -m "feat(frontend): add fixed_prompt editable field in AiResultPanel"
```

---

## Phase 5: Wizard 前端重寫

### Task 10: 建立 WizardPendingList 共用元件

**Files:**
- Create: `frontend/src/components/shared/WizardPendingList.tsx`

**Background:** Step 2 和 Step 3 都需要顯示一個「待確認列表」，每項可展開顯示 `AiResultPanel`，並附帶 JSON 編輯器（與 `/pending` 頁面相同）。把這個共用邏輯抽成元件。

**Step 1: 先查看 PendingPage 的 PendingResultRow 實作方式**

讀取 `frontend/src/pages/pending/PendingPage.tsx`，了解 `PendingResultRow` 如何組合 `AiResultPanel` + JSON textarea + confirm/reject/regenerate 操作。

**Step 2: 建立 `WizardPendingList.tsx`**

```tsx
import { useState } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { AiResultPanel } from "@/components/shared/AiResultPanel"
import { Textarea } from "@/components/ui/textarea"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { ChevronDown, ChevronRight, Loader2 } from "lucide-react"
import type { PendingAiResult } from "@/schemas/ai"

interface WizardPendingListProps {
  results: readonly PendingAiResult[]
  onAnyChange: () => void  // callback to refetch results after any action
}

function WizardPendingRow({
  result,
  onAnyChange,
}: {
  result: PendingAiResult
  onAnyChange: () => void
}) {
  const [expanded, setExpanded] = useState(false)
  const [jsonValue, setJsonValue] = useState(
    result.generated_data ? JSON.stringify(result.generated_data, null, 2) : ""
  )

  const { mutate: updateData } = useEffectMutation(
    (data: Record<string, unknown>) =>
      Effect.flatMap(CoreApi, (api) => api.updatePendingAiResult(result.id, data)),
  )

  const handleJsonChange = (v: string) => {
    setJsonValue(v)
    try {
      const parsed = JSON.parse(v)
      updateData(parsed)
    } catch {
      // invalid JSON — don't update
    }
  }

  const statusColor: Record<string, string> = {
    generating: "bg-yellow-500",
    pending: "bg-blue-500",
    confirmed: "bg-green-500",
    failed: "bg-red-500",
  }

  return (
    <div className="border rounded-lg">
      <button
        className="w-full flex items-center gap-2 p-3 text-left hover:bg-muted/50 transition-colors"
        onClick={() => setExpanded((v) => !v)}
      >
        {expanded ? <ChevronDown className="size-4 shrink-0" /> : <ChevronRight className="size-4 shrink-0" />}
        <span
          className={`size-2 rounded-full shrink-0 ${statusColor[result.status] ?? "bg-gray-400"}`}
        />
        <span className="text-sm flex-1 truncate">{result.source_title}</span>
        {result.status === "generating" && (
          <Loader2 className="size-3 animate-spin text-muted-foreground" />
        )}
        <Badge variant="outline" className="text-xs">
          {result.result_type}
        </Badge>
      </button>

      {expanded && (
        <div className="border-t p-4 space-y-4">
          <AiResultPanel
            result={result}
            onConfirmed={onAnyChange}
            onRejected={onAnyChange}
            onRegenerated={onAnyChange}
          >
            {/* JSON 編輯器 */}
            <Textarea
              value={jsonValue}
              onChange={(e) => handleJsonChange(e.target.value)}
              rows={8}
              className="font-mono text-xs"
              placeholder="生成的 JSON 資料"
            />
          </AiResultPanel>
        </div>
      )}
    </div>
  )
}

export function WizardPendingList({ results, onAnyChange }: WizardPendingListProps) {
  if (results.length === 0) {
    return (
      <p className="text-sm text-muted-foreground text-center py-4">
        沒有待確認項目
      </p>
    )
  }

  return (
    <div className="space-y-2">
      {results.map((r) => (
        <WizardPendingRow key={r.id} result={r} onAnyChange={onAnyChange} />
      ))}
    </div>
  )
}
```

**Step 3: TypeScript 確認**

```bash
cd frontend && npx tsc --noEmit 2>&1 | head -20
```

**Step 4: Commit**

```bash
git add frontend/src/components/shared/WizardPendingList.tsx
git commit -m "feat(frontend): add WizardPendingList shared component"
```

---

### Task 11: 重寫 CreateSubscriptionWizard — 三步驟

**Files:**
- Modify: `frontend/src/pages/subscriptions/CreateSubscriptionWizard.tsx`

**Background:**
- **Step 1**：填寫基本資料（URL、名稱、抓取間隔、可選 Fetcher）→ 點「建立訂閱」送出
- **Step 2**：輪詢（每 1 秒）`getRawItems({ subscription_id })`；所有 raw item 不再是 `pending` 後停止輪詢；同時顯示 `getPendingAiResults({ subscription_id, result_type: "parser" })`；所有 pending_ai_results 都是 `confirmed` 或 `rejected` 才可點「下一步」
- **Step 3**：輪詢 `getPendingAiResults({ subscription_id, result_type: "filter" })`，直到沒有 `generating`；顯示 filter 待確認列表；全部確認/拒絕後點「完成」

**Step 1: 完整重寫 `CreateSubscriptionWizard.tsx`**

```tsx
import { useState, useEffect, useRef } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { CheckCircle2, Loader2 } from "lucide-react"
import { WizardPendingList } from "@/components/shared/WizardPendingList"
import type { PendingAiResult } from "@/schemas/ai"
import type { RawAnimeItem } from "@/schemas/download"
import type { ServiceModule } from "@/schemas/service-module"

interface CreateSubscriptionWizardProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onCreated?: () => void
}

type WizardStep = 1 | 2 | 3

export function CreateSubscriptionWizard({
  open,
  onOpenChange,
  onCreated,
}: CreateSubscriptionWizardProps) {
  const [step, setStep] = useState<WizardStep>(1)
  const [url, setUrl] = useState("")
  const [name, setName] = useState("")
  const [interval, setInterval] = useState("30")
  const [fetcherId, setFetcherId] = useState<string>("auto")
  const [subscriptionId, setSubscriptionId] = useState<number | null>(null)

  // Step 2/3 輪詢狀態
  const [rawItems, setRawItems] = useState<readonly RawAnimeItem[]>([])
  const [parserPending, setParserPending] = useState<readonly PendingAiResult[]>([])
  const [filterPending, setFilterPending] = useState<readonly PendingAiResult[]>([])
  const [polling, setPolling] = useState(false)
  const pollingRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // 取得 Fetcher 列表（供 Step 1 選擇）
  const { data: fetcherModules } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getFetcherModules),
    [],
  )

  const reset = () => {
    setStep(1)
    setUrl("")
    setName("")
    setInterval("30")
    setFetcherId("auto")
    setSubscriptionId(null)
    setRawItems([])
    setParserPending([])
    setFilterPending([])
    setPolling(false)
    if (pollingRef.current) clearTimeout(pollingRef.current)
  }

  // 清除輪詢 timer on unmount
  useEffect(() => {
    return () => {
      if (pollingRef.current) clearTimeout(pollingRef.current)
    }
  }, [])

  const { mutate: createSub, isLoading: creating } = useEffectMutation(
    () =>
      Effect.flatMap(CoreApi, (api) =>
        api.createSubscription({
          source_url: url.trim(),
          name: name.trim() || undefined,
          fetch_interval_minutes: interval === "" ? 30 : parseInt(interval),
          fetcher_id: fetcherId !== "auto" ? parseInt(fetcherId) : undefined,
        }),
      ),
  )

  const handleCreate = () => {
    createSub().then((sub) => {
      if (sub) {
        setSubscriptionId(sub.subscription_id)
        setStep(2)
        setPolling(true)
      }
    })
  }

  // Step 2: 輪詢 raw items + parser pending
  const pollStep2 = async (subId: number) => {
    try {
      const api = await Effect.runPromise(
        Effect.flatMap(CoreApi, (api) => Effect.succeed(api))
      )
      const items = await Effect.runPromise(
        api.getRawItems({ subscription_id: subId, limit: 200 })
      )
      setRawItems(items)

      const pending = await Effect.runPromise(
        api.getPendingAiResults({ subscription_id: subId, result_type: "parser" })
      )
      setParserPending(pending)

      const stillPending = items.some((i) => i.status === "pending")
      const hasGenerating = pending.some((p) => p.status === "generating")

      if (stillPending || hasGenerating) {
        pollingRef.current = setTimeout(() => pollStep2(subId), 1000)
      } else {
        setPolling(false)
      }
    } catch {
      setPolling(false)
    }
  }

  // Step 3: 輪詢 filter pending
  const pollStep3 = async (subId: number) => {
    try {
      const api = await Effect.runPromise(
        Effect.flatMap(CoreApi, (api) => Effect.succeed(api))
      )
      const pending = await Effect.runPromise(
        api.getPendingAiResults({ subscription_id: subId, result_type: "filter" })
      )
      setFilterPending(pending)

      const hasGenerating = pending.some((p) => p.status === "generating")
      if (hasGenerating) {
        pollingRef.current = setTimeout(() => pollStep3(subId), 1000)
      } else {
        setPolling(false)
      }
    } catch {
      setPolling(false)
    }
  }

  // 啟動輪詢
  useEffect(() => {
    if (step === 2 && subscriptionId && polling) {
      pollStep2(subscriptionId)
    }
    if (step === 3 && subscriptionId && polling) {
      pollStep3(subscriptionId)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, subscriptionId, polling])

  const handleGoToStep3 = () => {
    if (pollingRef.current) clearTimeout(pollingRef.current)
    setPolling(true)
    setStep(3)
  }

  const handleFinish = () => {
    onCreated?.()
    onOpenChange(false)
    reset()
  }

  // Step 2 「下一步」啟用條件：無輪詢中、無 generating/pending 的 parser pending
  const canProceedStep2 =
    !polling &&
    parserPending.every((p) => p.status === "confirmed" || p.status === "rejected" || p.status === "failed")

  // Step 3 「完成」啟用條件：無輪詢中、無 generating/pending 的 filter pending
  const canFinishStep3 =
    !polling &&
    filterPending.every((p) => p.status === "confirmed" || p.status === "rejected" || p.status === "failed")

  const stepTitles: Record<WizardStep, string> = {
    1: "基本設定",
    2: "解析確認",
    3: "Conflict Filter",
  }

  const refreshParserPending = () => {
    if (subscriptionId) {
      Effect.runPromise(
        Effect.flatMap(CoreApi, (api) =>
          api.getPendingAiResults({ subscription_id: subscriptionId, result_type: "parser" })
        )
      ).then(setParserPending).catch(() => {})
    }
  }

  const refreshFilterPending = () => {
    if (subscriptionId) {
      Effect.runPromise(
        Effect.flatMap(CoreApi, (api) =>
          api.getPendingAiResults({ subscription_id: subscriptionId, result_type: "filter" })
        )
      ).then(setFilterPending).catch(() => {})
    }
  }

  const parsedCount = rawItems.filter((i) => i.status === "parsed").length
  const failedCount = rawItems.filter(
    (i) => i.status === "no_match" || i.status === "failed"
  ).length

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v) reset()
        onOpenChange(v)
      }}
    >
      <DialogContent className="sm:max-w-xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>新增訂閱 — {stepTitles[step]}</DialogTitle>
        </DialogHeader>

        {/* Step 指示器 */}
        <div className="flex gap-2">
          {([1, 2, 3] as WizardStep[]).map((s) => (
            <div
              key={s}
              className={`flex-1 h-1 rounded-full ${s <= step ? "bg-primary" : "bg-muted"}`}
            />
          ))}
        </div>

        {/* ── Step 1 ── */}
        {step === 1 && (
          <div className="space-y-4 overflow-y-auto">
            <div className="space-y-2">
              <Label>RSS URL *</Label>
              <Input
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="https://mikanani.me/RSS/..."
              />
            </div>
            <div className="space-y-2">
              <Label>名稱</Label>
              <Input value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label>抓取間隔（分鐘，0 = 單次）</Label>
              <Input
                type="number"
                min="0"
                value={interval}
                onChange={(e) => setInterval(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label>Fetcher（留空自動選擇）</Label>
              <Select value={fetcherId} onValueChange={setFetcherId}>
                <SelectTrigger>
                  <SelectValue placeholder="自動選擇" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="auto">自動選擇</SelectItem>
                  {fetcherModules?.map((m: ServiceModule) => (
                    <SelectItem key={m.module_id} value={String(m.module_id)}>
                      {m.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                取消
              </Button>
              <Button onClick={handleCreate} disabled={!url.trim() || creating}>
                {creating && <Loader2 className="mr-1 size-4 animate-spin" />}
                建立訂閱
              </Button>
            </DialogFooter>
          </div>
        )}

        {/* ── Step 2 ── */}
        {step === 2 && (
          <div className="flex flex-col gap-4 overflow-y-auto min-h-0 flex-1">
            {/* 統計列 */}
            <div className="flex gap-4 text-sm text-muted-foreground">
              {polling ? (
                <span className="flex items-center gap-1">
                  <Loader2 className="size-3 animate-spin" /> 正在抓取 RSS…
                </span>
              ) : (
                <>
                  <span>共 {rawItems.length} 項</span>
                  <span className="text-green-600">{parsedCount} 已解析</span>
                  {failedCount > 0 && (
                    <span className="text-red-500">{failedCount} 解析失敗</span>
                  )}
                </>
              )}
            </div>

            {/* Parser 待確認列表 */}
            <div className="overflow-y-auto flex-1">
              <WizardPendingList
                results={parserPending}
                onAnyChange={refreshParserPending}
              />
              {!polling && parserPending.length === 0 && rawItems.length > 0 && (
                <div className="flex items-center gap-2 text-green-600 text-sm py-2">
                  <CheckCircle2 className="size-4" />
                  所有項目解析成功
                </div>
              )}
            </div>

            <DialogFooter>
              <Button
                onClick={handleGoToStep3}
                disabled={!canProceedStep2}
              >
                {polling && <Loader2 className="mr-1 size-4 animate-spin" />}
                下一步
              </Button>
            </DialogFooter>
          </div>
        )}

        {/* ── Step 3 ── */}
        {step === 3 && (
          <div className="flex flex-col gap-4 overflow-y-auto min-h-0 flex-1">
            <div className="text-sm text-muted-foreground">
              {polling ? (
                <span className="flex items-center gap-1">
                  <Loader2 className="size-3 animate-spin" /> 正在檢查 Conflict…
                </span>
              ) : filterPending.length === 0 ? (
                <span className="flex items-center gap-2 text-green-600">
                  <CheckCircle2 className="size-4" /> 無衝突
                </span>
              ) : (
                <span>發現 {filterPending.length} 個 Conflict，請確認 Filter 規則</span>
              )}
            </div>

            <div className="overflow-y-auto flex-1">
              <WizardPendingList
                results={filterPending}
                onAnyChange={refreshFilterPending}
              />
            </div>

            <DialogFooter>
              <Button onClick={handleFinish} disabled={!canFinishStep3}>
                {polling && <Loader2 className="mr-1 size-4 animate-spin" />}
                完成
              </Button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
```

> **重要**：`createSubscription` 的 `req` 型別需要確認是否已支援 `fetcher_id`。查看 `CoreApi.ts` 的 `createSubscription` 定義，如尚未有 `fetcher_id?: number` 則需補上。

**Step 2: 確認 createSubscription 的 req 型別**

查看 `CoreApi.ts` 第 80-85 行的 `createSubscription` 定義：

```typescript
readonly createSubscription: (req: {
  source_url: string
  name?: string
  fetch_interval_minutes?: number
  preferred_downloader_id?: number | null
  fetcher_id?: number   // 若沒有則需補上
}) => Effect.Effect<Subscription>
```

若 `ApiLayer.ts` 中 `createSubscription` 的實作也需要傳 `fetcher_id`，確認 request body 包含它。

**Step 3: TypeScript 確認**

```bash
cd frontend && npx tsc --noEmit 2>&1 | head -30
```

修正所有 type error。

**Step 4: 視覺測試**

啟動前端（`cd frontend && npm run dev`），建立一個新訂閱，確認：
- Step 1：Fetcher 下拉出現
- Step 2：建立後顯示 spinner，完成後顯示 raw items 統計 + parser pending list
- Step 3：顯示 conflict 狀態 + filter pending list
- AiResultPanel 有固定 Prompt + 自訂 Prompt 兩個 textarea

**Step 5: Commit**

```bash
git add frontend/src/pages/subscriptions/CreateSubscriptionWizard.tsx
git commit -m "feat(frontend): rewrite subscription wizard as 3-step with polling and pending confirmation"
```

---

## 驗收清單

- [ ] `cargo test -p core-service` 通過
- [ ] `cd frontend && npx tsc --noEmit` 無 type error
- [ ] `cargo clippy -p core-service` 無 error
- [ ] Step 1 Fetcher 下拉正確顯示（或顯示「自動選擇」）
- [ ] Step 2 輪詢在所有 raw items 不再 `pending` 後停止
- [ ] Step 2 `pending_ai_results` 按 subscription_id 正確過濾
- [ ] Step 2 parser pending items 展開可見 AiResultPanel（含固定 Prompt + 自訂 Prompt）
- [ ] Step 3 在確認 parser 後觸發 conflict detection 並生成 filter pending
- [ ] Step 3 filter pending items 展開可見 AiResultPanel
- [ ] `/pending` 頁面的 AiResultPanel 也同樣顯示固定 Prompt（Task 9 通用）
- [ ] `cargo fmt` + `cargo clippy` 通過
