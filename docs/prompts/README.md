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

## 2026-10-25 補強版（目前內建預設與 `parser-short.txt`）

依失誤補上規則：過濾規則照標題順序寫、中間用 `.*` 跳過、集數一律 `\d+`、送出前逐組驗證；
解析器加「送出前最後檢查」（空格用 `\s*`、順序、不是欄位的括號用 `(?:...)`、只跳脫 `[` `]`、condition_regex 與 parse_regex 結構一致）。

| 模型 | 解析器 | 過濾 |
|---|---|---|
| groq（`parser-short.txt`） | 23/23 | 9/9 |
| mistral（內建長版，跑兩輪） | 46/46 | 18/18 |
| openRouter（複本）（內建長版） | 9/9（成功的 2 次）| —（其餘 5 次上游 502，未評分） |

補強前 mistral 在同一組測試是解析 22/23、過濾 6/9。

## 2026-10-28 NVIDIA build（`https://integrate.api.nvidia.com/v1`，免費 key）

目錄（`/v1/models`）上有的模型，免費帳號不一定能用：

| 模型 | 結果 |
|---|---|
| `minimaxai/minimax-m3.1-flash`（inject_schema） | **解析 23/23、過濾 9/9**，每次 20–122 秒 → 採用，逾時 180 秒、排在最後 |
| `z-ai/glm-5.3-flash`（strict） | 解析 23/23、過濾 6/9（1 次回應不完整）；inject_schema 時回的不是 JSON |
| `openai/gpt-oss-20b`（strict） | 解析 23/23、過濾 6/9 |
| `moonshotai/kimi-k2.6`、`nvidia/nemotron-3-super-120b-a12b`、`mistralai/mistral-large-2-instruct` | 404 `Function … Not found for account`（免費帳號不能用） |
| `deepseek-ai/deepseek-v4.1-flash`、`google/gemma-4-31b-it` | 180 秒逾時 |

`openai/gpt-oss-120b` 已不在目錄上。
