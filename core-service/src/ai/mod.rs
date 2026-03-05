pub mod client;
pub mod filter_generator;
pub mod openai;
pub mod parser_generator;
pub mod prompts;

pub use client::{AiClient, AiError};
pub use openai::OpenAiClient;

/// AI 回傳的字串可能包在 markdown code fence 中（e.g. ```json ... ```）
/// 嘗試提取其中的 JSON 內容，否則原樣回傳
pub fn extract_json(s: &str) -> &str {
    let trimmed = s.trim();
    // 處理 ```json ... ``` 或 ``` ... ```
    let inner = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .map(|s| s.trim());
    inner.unwrap_or(trimmed)
}
