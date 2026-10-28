pub mod auto_trigger;
pub mod chain;
pub mod client;
pub mod factory;
pub mod filter_generator;
pub mod openai;
pub mod parser_generator;
pub mod prompt_test;
pub mod prompts;
pub mod queue;

pub use chain::{build_ai_chain, AiProviderChain, AttemptRecord};
pub use client::{AiClient, AiError};
pub use openai::OpenAiClient;

/// AI 生成失敗的通知：同一個訂閱、同一種結果 6 小時內只通知一次
pub(crate) fn alert_generation_failed(
    pool: &crate::db::DbPool,
    pending: &crate::models::PendingAiResult,
    error: &str,
) {
    let kind = if pending.result_type == "filter" {
        "過濾規則"
    } else {
        "解析器"
    };
    let subscription = pending
        .subscription_id
        .map(|id| format!("訂閱 {}", id))
        .unwrap_or_else(|| "手動".to_string());
    crate::services::alerts::raise(
        pool,
        &format!(
            "ai:{}:{}",
            pending.result_type,
            pending.subscription_id.unwrap_or(0)
        ),
        &format!("⚠️ AI 產生{}失敗", kind),
        &format!("{}（{}）：{}", subscription, pending.source_title, error),
    );
}

/// 記下實際成功的 provider 用的固定 prompt（三層 prompt 下可能是那家的模型 prompt），
/// 重新生成時編輯的就是這份。
pub(crate) fn record_used_fixed_prompt(pool: &crate::db::DbPool, pending_id: i32, used: &str) {
    use crate::schema::pending_ai_results;
    use diesel::prelude::*;
    if let Ok(mut conn) = pool.get() {
        diesel::update(pending_ai_results::table.find(pending_id))
            .set(pending_ai_results::used_fixed_prompt.eq(used))
            .execute(&mut conn)
            .map_err(|e| tracing::warn!("無法記錄 pending {} 用的 prompt: {}", pending_id, e))
            .ok();
    }
}

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
