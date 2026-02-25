# Fix Global Parsers Visibility Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 讓全域預設解析器正確顯示於 Dashboard 的 Global Parsers 列表，並確保 ParsersPage 新增的解析器也歸屬於 global。

**Architecture:** 初始 migration 的預設解析器插入時未設定 `created_from_type`，導致後端以 `WHERE created_from_type = 'global'` 查詢時找不到這些解析器。修正方式：①migration 只保留 Catch-All 並標記為 global；②ParsersPage 新增解析器時明確傳入 `created_from_type: "global"`。

**Tech Stack:** Rust (Diesel migration SQL), React/TypeScript (frontend)

---

### Task 1: 修正 migration 初始資料

**Files:**
- Modify: `core-service/migrations/2026-02-17-000000-initial-schema/up.sql`

**背景：**
目前 up.sql 末尾有兩段 `INSERT INTO title_parsers`，共插入 4 筆：
- `LoliHouse 標準格式`（priority 100）
- `六四位元 星號格式`（priority 90）
- `預設解析器`（priority 1）
- `Catch-All 全匹配`（priority 0）

只保留 `Catch-All 全匹配`，並加上 `created_from_type = 'global'`。

**Step 1: 確認目前 migration 內容（約第 293–356 行）**

閱讀 `core-service/migrations/2026-02-17-000000-initial-schema/up.sql` 第 293–357 行確認現有 INSERT 語句。

**Step 2: 替換兩段 INSERT 為單一段**

將檔案中從 `-- Default Data: Title Parsers` 開始到最後一個分號（`);`）的整個區塊，替換成：

```sql
-- ============================================================================
-- Default Data: Title Parsers
-- ============================================================================
INSERT INTO title_parsers (
    name, description, priority,
    condition_regex, parse_regex,
    anime_title_source, anime_title_value,
    episode_no_source, episode_no_value,
    series_no_source, series_no_value,
    subtitle_group_source, subtitle_group_value,
    created_from_type
) VALUES (
    'Catch-All 全匹配',
    '最低優先級，將整個標題作為動畫名稱，集數預設為 1。確保所有標題都能被解析。',
    0,
    '.+',
    '^(.+)$',
    'regex', '1',
    'static', '1',
    'static', '1',
    'static', '未知字幕組',
    'global'
);
```

**Step 3: 驗證 SQL 語法正確**

檢查替換後的 up.sql 末尾，確認只剩一段 INSERT，column 數量與 VALUES 數量一致，語句以 `;` 結尾。

**Step 4: Commit**

```bash
git add core-service/migrations/2026-02-17-000000-initial-schema/up.sql
git commit -m "fix(migration): keep only Catch-All parser with created_from_type=global"
```

---

### Task 2: 修正 ParsersPage 新增解析器時的 created_from_type

**Files:**
- Modify: `frontend/src/pages/parsers/ParsersPage.tsx`

**背景：**
ParsersPage 的 `createParser` mutation 目前呼叫 `createParser(buildParserRequest(form))`，而 `buildParserRequest` 只包含解析器欄位，不包含 `created_from_type`。後端收到後存入 NULL，導致新增的解析器在 global 列表中不顯示。

**Step 1: 找到 createParser mutation 的定義位置**

在 `ParsersPage.tsx` 第 44–50 行：
```typescript
const { mutate: createParser, isLoading: creating } = useEffectMutation(
  (req: Record<string, unknown>) =>
    Effect.gen(function* () {
      const api = yield* CoreApi
      return yield* api.createParser(req)
    }),
)
```

**Step 2: 找到 handleSave 呼叫 createParser 的位置**

在 `ParsersPage.tsx` 第 128–141 行 `handleSave` 中：
```typescript
result = await createParser(buildParserRequest(form))
```

**Step 3: 修改 createParser 呼叫，加入 created_from_type**

將：
```typescript
result = await createParser(buildParserRequest(form))
```

改為：
```typescript
result = await createParser({ ...buildParserRequest(form), created_from_type: "global" })
```

**Step 4: 確認 updateParser 不需要修改**

`updateParser` 使用 `buildParserRequest(form)` 更新現有解析器。現有解析器若已有 `created_from_type`，更新時後端會以 request body 中的值覆蓋。目前 `buildParserRequest` 不含 `created_from_type`，所以更新後會被清為 NULL——這是個潛在問題。

在 `handleSave` 的 editTarget 分支：
```typescript
result = await updateParser({ id: editTarget.parser_id as number, data: buildParserRequest(form) })
```

改為：
```typescript
result = await updateParser({
  id: editTarget.parser_id as number,
  data: { ...buildParserRequest(form), created_from_type: editTarget.created_from_type ?? "global" },
})
```

這樣可以保留原有的 `created_from_type`，global 的 parser 更新後依然是 global。

**Step 5: 確認前端 TitleParser schema 包含 created_from_type 欄位**

查看 `frontend/src/schemas/parser.ts`，確認 `TitleParser` 有 `created_from_type` 欄位（若無，型別推斷會使用 `Record<string, unknown>` 中的 unknown，已足夠）。

**Step 6: Commit**

```bash
git add frontend/src/pages/parsers/ParsersPage.tsx
git commit -m "fix(frontend): pass created_from_type=global when creating/updating parsers in ParsersPage"
```
