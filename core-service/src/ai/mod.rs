pub mod client;
pub mod filter_generator;
pub mod openai;
pub mod parser_generator;
pub mod prompts;

pub use client::{AiClient, AiError};
pub use openai::OpenAiClient;

/// 修正結構化輸出模式下模型雙重轉義 regex 的問題。
///
/// 部分模型在 JSON Schema 強制輸出時，會對 regex 字串中的反斜線雙重 escape：
/// 正確應為 `\[`（JSON 中寫 `\\[`），卻輸出 `\\[`（JSON 中寫 `\\\\[`）。
/// 本函數將解析後字串中的連續兩個反斜線縮減為一個：`\\` → `\`
pub fn fix_regex_escaping(s: &str) -> String {
    s.replace("\\\\", "\\")
}

/// 修復 AI 輸出中常見的非法 JSON escape 序列。
///
/// JSON 字串內只允許 `\"` `\\` `\/` `\b` `\f` `\n` `\r` `\t` `\uXXXX`。
/// 模型有時會輸出 `\$`、`\[`、`\]` 等非法 escape，導致解析失敗。
/// 本函數將這類非法 `\X` 還原為單純的 `X`（移除多餘的反斜線）。
pub fn sanitize_ai_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_string = false;
    let mut escaped = false;

    for ch in s.chars() {
        if escaped {
            match ch {
                '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' | 'u' => {
                    // 合法 escape，原樣保留反斜線
                    result.push('\\');
                    result.push(ch);
                }
                _ => {
                    // 非法 escape：移除反斜線，只保留字元
                    result.push(ch);
                }
            }
            escaped = false;
        } else if in_string && ch == '\\' {
            escaped = true;
        } else {
            if ch == '"' {
                in_string = !in_string;
            }
            result.push(ch);
        }
    }

    result
}

/// AI 回傳的字串可能包含 thinking model 的 `<think>...</think>` 推理區塊，
/// 或是 markdown code fence（` ```json ... ``` `）。
/// 嘗試提取其中的 JSON 內容；無法提取時回傳空字串。
pub fn extract_json(s: &str) -> &str {
    let base = if let Some(end) = s.find("</think>") {
        // thinking 完成，取閉合標籤後的內容
        s[end + "</think>".len()..].trim()
    } else if s.contains("<think>") {
        // thinking 區塊未閉合（回應被截斷），沒有有效 JSON
        return "";
    } else {
        s.trim()
    };

    // 處理 ```json ... ``` 或 ``` ... ```
    base.strip_prefix("```json")
        .or_else(|| base.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .map(|s| s.trim())
        .unwrap_or(base)
}
