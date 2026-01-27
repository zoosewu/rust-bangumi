# 訂閱系統實施說明

**日期：** 2026-01-26
**基於：** 2026-01-26-subscription-system-redesign.md
**確認版本：** 1.0

---

## 🔧 實施確認

### 超時設置
```
廣播超時：60 秒（1 分鐘）

含義：
- CORE 向每個 Fetcher 發送 /can-handle-subscription 請求
- 如果 60 秒內沒有收到回應，視為該 Fetcher 不能處理
- 同時廣播給多個 Fetcher，不是順序等待

實現：
tokio::time::timeout(
    Duration::from_secs(60),
    broadcast_can_handle(fetcher_id, url)
)
```

---

## 📋 追蹤表詳解：`subscription_selections`

### 表結構
```sql
CREATE TABLE subscription_selections (
  selection_id SERIAL PRIMARY KEY,

  -- 關聯
  subscription_id INTEGER NOT NULL REFERENCES subscriptions,
  fetcher_id INTEGER NOT NULL REFERENCES fetcher_modules,

  -- 廣播結果
  can_handle BOOLEAN NOT NULL,
  candidate_priority INTEGER,

  -- 決策結果
  selected BOOLEAN NOT NULL,
  selection_reason VARCHAR(100),
  selected_at TIMESTAMP,

  created_at TIMESTAMP NOT NULL,

  UNIQUE(subscription_id, fetcher_id)
);
```

### 表的作用

#### 1. **審計日誌（Audit Trail）**
記錄每個訂閱的決策過程，便於事後查證。

**例子：**
```
訂閱 ID: 1, URL: https://mikanani.me/RSS/Bangumi?bangumiId=3215

subscription_selections 記錄：
┌──────────────────────────────────────────────────┐
│ selection_id │ fetcher_id │ can_handle │ selected │
├──────────────────────────────────────────────────┤
│      1       │     1      │    true    │   true   │ ← 選中
│      2       │     2      │    false   │   false  │
│      3       │     3      │    false   │   false  │
└──────────────────────────────────────────────────┘

selection_reason: "highest_priority"
selected_at: 2026-01-26 10:30:45
```

#### 2. **問題診斷（Debugging）**

**場景：** 為什麼 Fetcher A 從未被選中？
```sql
SELECT COUNT(*) as can_handle_count,
       COUNT(CASE WHEN selected THEN 1 END) as selected_count
FROM subscription_selections
WHERE fetcher_id = 2  -- Fetcher A
GROUP BY fetcher_id;

結果：
can_handle_count: 0
selected_count: 0
→ 說明 Fetcher A 從不回答 can_handle=true
→ 需要檢查 Fetcher A 的判斷邏輯或網絡連接
```

#### 3. **性能分析（Analytics）**

```sql
-- 哪些 Fetcher 最常被選中？
SELECT f.name, COUNT(*) as selection_count
FROM subscription_selections s
JOIN fetcher_modules f ON s.fetcher_id = f.fetcher_id
WHERE s.selected = true
GROUP BY f.name
ORDER BY selection_count DESC;

結果：
┌──────────┬──────────────┐
│   name   │ selection_count
├──────────┼──────────────┤
│ Mikanani │     150
│ HTTP     │      45
│ Twitter  │      12
└──────────┴──────────────┘

→ Mikanani 的使用最多，設計合理
→ Twitter 使用少，可能需要改進或推廣
```

#### 4. **監控告警（Monitoring）**

```sql
-- 某個 Fetcher 突然停止工作？
SELECT DATE(created_at) as date,
       COUNT(*) as total_broadcasts,
       SUM(CASE WHEN can_handle THEN 1 ELSE 0 END) as can_handle_count
FROM subscription_selections
WHERE fetcher_id = 1  -- Mikanani
GROUP BY DATE(created_at)
ORDER BY date DESC;

結果：
┌────────────┬─────────────┬──────────────┐
│    date    │ total_broadcasts
│ can_handle_count│
├────────────┼─────────────┼──────────────┤
│ 2026-01-26 │     10      │      0       │ ← 警報！
│ 2026-01-25 │     12      │      11      │
│ 2026-01-24 │     15      │      14      │
└────────────┴─────────────┴──────────────┘

→ Mikanani 在最近24小時內沒有回應任何請求
→ 觸發告警
```

---

## 📊 追蹤表使用場景

### 場景 1：用戶查詢

**用戶問：** "為什麼我的訂閱分配給了 Mikanani 而不是 Twitter？"

**查詢：**
```sql
SELECT ss.*, f.name, f.priority
FROM subscription_selections ss
JOIN fetcher_modules f ON ss.fetcher_id = f.fetcher_id
WHERE ss.subscription_id = 5
ORDER BY ss.fetcher_id;

結果：
┌─────────────────────────────────────────────────────────┐
│ fetcher_id │ name    │ can_handle │ priority │ selected │
├─────────────────────────────────────────────────────────┤
│     1      │Mikanani│    true    │   80     │   true   │ ← 選中
│     2      │ Twitter│    false   │   60     │   false  │
│     3      │ HTTP   │    true    │   30     │   false  │
└─────────────────────────────────────────────────────────┘

能夠回答：
- Mikanani 和 HTTP 都能處理
- Mikanani 優先級更高（80 > 30）
- 所以選擇了 Mikanani
```

### 場景 2：系統維護

**系統管理員調查：** "某 Fetcher 的優先級被誤改了？"

**查詢歷史：**
```sql
-- 查看該訂閱的所有決策
SELECT ss.*, f.priority as fetcher_priority_at_time,
       ss.candidate_priority as recorded_priority
FROM subscription_selections ss
JOIN fetcher_modules f ON ss.fetcher_id = f.fetcher_id
WHERE ss.subscription_id = 5
ORDER BY ss.created_at;

對比：
- candidate_priority: 80（當時記錄）
- f.priority: 50（現在的優先級）
→ 優先級被改了！
```

### 場景 3：性能優化

**決策：** "要不要移除某個 Fetcher？"

**分析：**
```sql
SELECT f.name,
       COUNT(*) as total_evaluations,
       SUM(CASE WHEN can_handle THEN 1 ELSE 0 END) as can_handle_count,
       SUM(CASE WHEN selected THEN 1 ELSE 0 END) as selected_count,
       ROUND(
         SUM(CASE WHEN selected THEN 1 ELSE 0 END)::numeric /
         COUNT(*) * 100, 2
       ) as selection_rate
FROM subscription_selections ss
JOIN fetcher_modules f ON ss.fetcher_id = f.fetcher_id
WHERE ss.created_at > NOW() - INTERVAL '30 days'
GROUP BY f.name
ORDER BY selection_rate DESC;

結果：
┌──────────┬──────────┬───────────┬────────────┐
│   name   │ can_handle│selected   │ rate       │
├──────────┼──────────┼───────────┼────────────┤
│ Mikanani │   150    │    150    │   100.00%  │
│ HTTP     │    50    │     40    │    80.00%  │
│ Twitter  │     2    │     0     │     0.00%  │ ← 考慮移除
└──────────┴──────────┴───────────┴────────────┘

決策：
- Twitter 選中率 0%，可能配置有問題或不需要
- HTTP 回應率 80%，可能有間歇性故障
- Mikanani 完美
```

---

## 🎯 是否需要追蹤表？

### 短期（MVP）：**可選**

```
如果系統剛啟動，可以先不用追蹤表。
優先完成核心功能：
- 廣播機制 ✓
- 自動選擇 ✓
- 通知 Fetcher ✓

追蹤表是 nice-to-have，不是必需。
```

### 長期：**建議添加**

```
一旦系統穩定並且有多個 Fetcher 運行時：
- 需要監控和審計
- 需要性能分析
- 需要故障排查

建議在 Phase 2 添加。
```

---

## 📋 實施清單（更新版）

### Phase 1：數據庫（必須）
- [ ] 添加 `priority` 到 `fetcher_modules`
- [ ] 修改 `subscriptions` 表字段
- [ ] 數據遷移和驗證

### Phase 2：CORE Service（必須）
- [ ] 實現 `auto_select_fetcher()` 邏輯
- [ ] 實現廣播機制（60秒超時）
- [ ] 修改 `POST /subscriptions` 端點
- [ ] 實現顯式指定 Fetcher 的邏輯

### Phase 3：Fetcher 適配（必須）
- [ ] Mikanani 實現 `POST /can-handle-subscription`
- [ ] Mikanani 改進 `POST /subscribe`
- [ ] 其他 Fetcher 適配

### Phase 4：追蹤表（可選，Phase 2+）
- [ ] 創建 `subscription_selections` 表
- [ ] CORE 在廣播時記錄結果
- [ ] 添加查詢工具/儀表板

### Phase 5：監控和告警（可選，Phase 3+）
- [ ] 設置異常告警
- [ ] 創建管理儀表板
- [ ] 性能報告

---

## 💡 建議

**我的建議是：**

1. **先實施 Phase 1-3**
   - 完成核心功能
   - 得到可工作的系統

2. **等系統穩定後**
   - 在 Phase 2 中添加追蹤表
   - 幫助監控和調試

3. **不要一開始就全做**
   - 追蹤表增加複雜度
   - 可以後期隨時添加

---

## 🚀 下一步

準備開始實施 Phase 1（數據庫遷移）？

我可以：
1. 編寫詳細的 SQL 遷移腳本
2. 開始修改 CORE Service 代碼
3. 創建需要的 Rust 結構體和邏輯
4. 更新 API 規格文檔

你想先從哪部分開始？
