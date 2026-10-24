# 模型專屬 prompt

AI 的固定 prompt 分三層：**全域**（設定頁的固定 prompt，空白時用內建預設）→ **模型**（AI provider 編輯視窗的「模型專屬 prompt」，有填就取代全域）→ **客製化**（設定頁的自訂 prompt，附加在所有模型後面）。

## 2026-10-24 測試結果

用 `/ai-providers/test-prompt` 對 4 種標題格式（解析器）與 3 個衝突訂閱（過濾）測試：

| 模型 | 解析器：內建長版 | 解析器：`parser-short.txt` | 過濾：舊版內建 | 過濾：目前內建 |
|---|---|---|---|---|
| groq `openai/gpt-oss-120b` | 23/23 | 23/23 | 9/9 | 9/9 |
| mistral `ministral-14b-latest` | 46/46（兩輪） | 26/46（兩輪） | 0/9 | 9/9 |

- 過濾：目前內建（明講「只寫一條正向規則」）兩個模型都全對 → 全域用它即可
- 解析器：mistral 只有長版穩定；groq 用短版也全對，而且 token 只有約三分之一（減少 Groq 每分鐘 8K token 的限流）
  → groq 的「解析器 prompt」建議貼上 [`parser-short.txt`](parser-short.txt)，其他模型留空
