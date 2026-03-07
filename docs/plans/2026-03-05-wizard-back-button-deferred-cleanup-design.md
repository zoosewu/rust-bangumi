# 訂閱精靈：返回按鈕、取消清理、同步爬取設計

**日期：** 2026-03-05
**狀態：** 已核准

---

## 需求摘要

1. **取消不留垃圾資料**：使用者在 Step 2/3 取消時，刪除已建立的訂閱
2. **允許返回上一步**：Step 2 可返回 Step 1 修改表單；Step 3 可返回 Step 2
3. **Step 2 進入時同步爬取**：確保進入 Step 2 時已有 RSS 資料可顯示

---

## 方案選擇

使用**方案 A（建立後可刪除重建）**：
- Step 1 仍立即建立訂閱（後端流程最小變動）
- 返回 Step 1 時刪除訂閱並重建
- 取消時刪除訂閱

---

## 前端設計（CreateSubscriptionWizard.tsx）

### Step 1 完成流程（修改）

```
按下「建立訂閱」
  → POST /subscriptions（建立，取得 subscription_id）
  → 顯示 loading「爬取中...」
  → POST /subscriptions/{id}/fetch（同步等待爬取完成）
  → setStep(2)，開始 Step 2 輪詢
```

### Step 2 底部按鈕（新增返回）

| 按鈕 | 行為 |
|------|------|
| 返回 | 停止輪詢 → DELETE /subscriptions/{id} → 清除 subscriptionId → 回 Step 1（預填表單） |
| 取消 | 停止輪詢 → DELETE /subscriptions/{id} → 關閉 wizard |
| 下一步 | 現有行為不變 |

### Step 3 底部按鈕（新增返回）

| 按鈕 | 行為 |
|------|------|
| 返回 | 停止 Step 3 輪詢 → 回 Step 2 → 重啟 Step 2 輪詢 |
| 取消 | 停止輪詢 → DELETE /subscriptions/{id} → 關閉 wizard |
| 完成 | 現有行為不變 |

### onOpenChange 修改

wizard 被關閉（X 按鈕或外部觸發）時，若 `subscriptionId` 存在（表示在 Step 2/3），則刪除訂閱再關閉。

---

## 後端設計（core-service）

### 新增端點

```
POST /subscriptions/:id/fetch
```

**功能：** 同步觸發一次 RSS 爬取，等待完成後回應。

**Handler：** `trigger_fetch_now`（新增於 `subscriptions.rs`）

**實作：** 呼叫既有 `trigger_immediate_fetch()` 但使用 `.await` 而非 `tokio::spawn`。

**回應：**
```json
{ "subscription_id": 1, "message": "Fetch completed" }
```

**錯誤：**
```json
{ "error": "fetch_failed", "message": "..." }
```

**路由（main.rs）：**
```rust
.route("/subscriptions/:id/fetch", post(trigger_fetch_now))
```

---

## API 客戶端設計（CoreApi.ts）

新增方法：
```typescript
triggerFetch(subscriptionId: number): Promise<void>
// POST /subscriptions/{subscriptionId}/fetch
```

新增方法：
```typescript
deleteSubscription(subscriptionId: number): Promise<void>
// DELETE /subscriptions/{subscriptionId}
```
（若尚未存在）

---

## 影響範圍

| 檔案 | 變更類型 |
|------|---------|
| `frontend/src/pages/subscriptions/CreateSubscriptionWizard.tsx` | 主要修改（按鈕、流程、cleanup） |
| `frontend/src/services/CoreApi.ts` | 新增 triggerFetch、確認 deleteSubscription |
| `core-service/src/handlers/subscriptions.rs` | 新增 trigger_fetch_now handler |
| `core-service/src/main.rs` | 新增一條路由 |

---

## 不在範圍內

- Step 2/3 的 AI 處理邏輯不變
- WizardPendingList 元件不變
- 訂閱建立的參數不變
