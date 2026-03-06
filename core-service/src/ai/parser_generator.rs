use chrono::Utc;
use diesel::prelude::*;
use serde_json::Value;
use std::sync::Arc;

use crate::db::DbPool;
use crate::models::{NewPendingAiResult, NewTitleParser, PendingAiResult, RawAnimeItem, TitleParser};
use crate::schema::{ai_prompt_settings, ai_settings, pending_ai_results, raw_anime_items, title_parsers};
use crate::services::title_parser::{ParseStatus, TitleParserService};
use super::client::AiClient;
use super::client::AiError;
use super::openai::OpenAiClient;
use super::prompts::*;

/// 從 DB 取得 AiClient，如果未設定則回傳 None
pub fn build_ai_client(conn: &mut PgConnection) -> Result<Option<OpenAiClient>, String> {
    let settings = ai_settings::table
        .first::<crate::models::AiSettings>(conn)
        .optional()
        .map_err(|e| e.to_string())?;

    match settings {
        Some(s) if !s.api_key.is_empty() && !s.base_url.is_empty() => {
            Ok(Some(OpenAiClient::new(&s.base_url, &s.api_key, &s.model_name)))
        }
        _ => Ok(None),
    }
}

/// 為單一動畫標題生成 parser（背景非同步觸發）
pub async fn generate_parser_for_title(
    pool: Arc<DbPool>,
    source_title: String,
    raw_item_id: Option<i32>,
    temp_custom_prompt: Option<String>,
    temp_fixed_prompt: Option<String>,
) -> Result<PendingAiResult, String> {
    let now = Utc::now().naive_utc();

    // 取得 prompt 設定
    let (fixed_prompt, custom_prompt) = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let prompt_settings = ai_prompt_settings::table
            .first::<crate::models::AiPromptSettings>(&mut conn)
            .optional()
            .map_err(|e| e.to_string())?;
        let fixed = prompt_settings
            .as_ref()
            .and_then(|p| p.fixed_parser_prompt.clone())
            .unwrap_or_else(|| DEFAULT_FIXED_PARSER_PROMPT.to_string());
        let custom = temp_custom_prompt.or_else(|| {
            prompt_settings.and_then(|p| p.custom_parser_prompt)
        });
        (fixed, custom)
    };

    // 若呼叫方提供臨時 fixed_prompt，以其覆蓋 DB 設定
    let fixed_prompt = temp_fixed_prompt.unwrap_or(fixed_prompt);

    // 從 raw_item_id 查詢所屬 subscription_id
    let subscription_id: Option<i32> = raw_item_id.and_then(|rid| {
        let mut conn = pool.get().ok()?;
        raw_anime_items::table
            .filter(raw_anime_items::item_id.eq(rid))
            .select(raw_anime_items::subscription_id)
            .first::<i32>(&mut conn)
            .ok()
    });

    // 建立 pending record（status=generating）
    let pending = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        diesel::insert_into(pending_ai_results::table)
            .values(NewPendingAiResult {
                result_type: "parser".to_string(),
                source_title: source_title.clone(),
                generated_data: None,
                status: "generating".to_string(),
                error_message: None,
                raw_item_id,
                used_fixed_prompt: fixed_prompt.clone(),
                used_custom_prompt: custom_prompt.clone(),
                expires_at: None,
                created_at: now,
                updated_at: now,
                subscription_id,
            })
            .get_result::<PendingAiResult>(&mut conn)
            .map_err(|e| e.to_string())?
    };

    let pending_id = pending.id;

    // 建立 AI client 並呼叫
    let client_result = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        build_ai_client(&mut conn)
    };

    let ai_result = match client_result {
        Ok(Some(client)) => {
            let system = build_system_prompt(Some(&fixed_prompt));
            let user = build_parser_user_prompt(&source_title, custom_prompt.as_deref());
            client.chat_completion_structured(&system, &user, &parser_schema()).await
        }
        Ok(None) => Err(AiError::NotConfigured),
        Err(e) => Err(AiError::ApiError(e)),
    };

    match ai_result {
        Ok(json_str) => {
            let extracted = super::extract_json(&json_str);
            tracing::debug!("AI 原始回應 pending_id={}: {:?}", pending_id, json_str);
            if extracted.is_empty() {
                return update_pending_failed(
                    &pool,
                    pending_id,
                    "AI 回應為空或無法提取 JSON",
                )
                .await;
            }
            match serde_json::from_str::<Value>(extracted) {
                Ok(mut data) => {
                    if data.get("condition_regex").is_none() || data.get("parse_regex").is_none() {
                        let err =
                            "AI 返回的 JSON 缺少必要欄位 condition_regex/parse_regex".to_string();
                        return update_pending_failed(&pool, pending_id, &err).await;
                    }
                    // 修正雙重轉義的 regex 欄位
                    for field in &["condition_regex", "parse_regex"] {
                        if let Some(fixed) = data.get(*field)
                            .and_then(|v| v.as_str())
                            .map(super::fix_regex_escaping)
                        {
                            data[*field] = Value::String(fixed);
                        }
                    }
                    let parser = create_unconfirmed_parser(&pool, &data, pending_id).await?;
                    tracing::info!("parser_id={} 已建立（未確認）", parser);
                    update_pending_success(&pool, pending_id, data).await
                }
                Err(e) => {
                    tracing::warn!(
                        "AI JSON 解析失敗 pending_id={}: {}\n原始回應: {:?}",
                        pending_id,
                        e,
                        json_str
                    );
                    update_pending_failed(
                        &pool,
                        pending_id,
                        &format!("JSON 解析失敗: {}（原始回應長度: {} 字元）", e, json_str.len()),
                    )
                    .await
                }
            }
        }
        Err(e) => update_pending_failed(&pool, pending_id, &e.to_string()).await,
    }
}

async fn create_unconfirmed_parser(
    pool: &Arc<DbPool>,
    data: &Value,
    pending_id: i32,
) -> Result<i32, String> {
    let now = Utc::now().naive_utc();
    let mut conn = pool.get().map_err(|e| e.to_string())?;

    let get_str = |key: &str| -> String {
        data.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let get_opt_str = |key: &str| -> Option<String> {
        data.get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };

    use crate::models::ParserSourceType;
    let parse_source = |key: &str| -> ParserSourceType {
        match data.get(key).and_then(|v| v.as_str()) {
            Some("static") => ParserSourceType::Static,
            _ => ParserSourceType::Regex,
        }
    };
    let parse_opt_source = |key: &str| -> Option<ParserSourceType> {
        match data.get(key).and_then(|v| v.as_str()) {
            Some("static") => Some(ParserSourceType::Static),
            Some("regex") => Some(ParserSourceType::Regex),
            _ => None,
        }
    };

    let new_parser = NewTitleParser {
        name: get_str("name"),
        description: None,
        priority: 50,
        is_enabled: true,
        condition_regex: get_str("condition_regex"),
        parse_regex: get_str("parse_regex"),
        anime_title_source: parse_source("anime_title_source"),
        anime_title_value: get_str("anime_title_value"),
        episode_no_source: parse_source("episode_no_source"),
        episode_no_value: get_str("episode_no_value"),
        episode_end_source: parse_opt_source("episode_end_source"),
        episode_end_value: get_opt_str("episode_end_value"),
        series_no_source: parse_opt_source("series_no_source"),
        series_no_value: get_opt_str("series_no_value"),
        subtitle_group_source: parse_opt_source("subtitle_group_source"),
        subtitle_group_value: get_opt_str("subtitle_group_value"),
        resolution_source: parse_opt_source("resolution_source"),
        resolution_value: get_opt_str("resolution_value"),
        season_source: parse_opt_source("season_source"),
        season_value: get_opt_str("season_value"),
        year_source: parse_opt_source("year_source"),
        year_value: get_opt_str("year_value"),
        created_at: now,
        updated_at: now,
        created_from_type: None,
        created_from_id: None,
        pending_result_id: Some(pending_id),
    };

    diesel::insert_into(title_parsers::table)
        .values(&new_parser)
        .returning(title_parsers::parser_id)
        .get_result::<i32>(&mut conn)
        .map_err(|e| e.to_string())
}

async fn update_pending_success(
    pool: &Arc<DbPool>,
    pending_id: i32,
    data: Value,
) -> Result<PendingAiResult, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    diesel::update(pending_ai_results::table.find(pending_id))
        .set((
            pending_ai_results::status.eq("pending"),
            pending_ai_results::generated_data.eq(Some(data)),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| e.to_string())
}

/// 為某訂閱中所有解析失敗的標題批次生成解析器
/// 每次 iteration：將所有 no_match 標題一次送給 AI → 生成一個 parser → 套用到匹配的項目 → 重試剩餘
pub async fn generate_parsers_for_subscription_batch(
    pool: Arc<DbPool>,
    subscription_id: i32,
) -> Result<(), String> {
    const MAX_ITERATIONS: usize = 5;

    // 載入 prompt 設定
    let (fixed_prompt, custom_prompt) = {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let ps = ai_prompt_settings::table
            .first::<crate::models::AiPromptSettings>(&mut conn)
            .optional()
            .map_err(|e| e.to_string())?;
        let fixed = ps.as_ref()
            .and_then(|p| p.fixed_parser_prompt.clone())
            .unwrap_or_else(|| DEFAULT_FIXED_PARSER_PROMPT.to_string());
        let custom = ps.and_then(|p| p.custom_parser_prompt);
        (fixed, custom)
    };

    for iteration in 0..MAX_ITERATIONS {
        // 查詢本次 no_match 項目
        let unmatched: Vec<RawAnimeItem> = {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            raw_anime_items::table
                .filter(raw_anime_items::subscription_id.eq(subscription_id))
                .filter(raw_anime_items::status.eq(ParseStatus::NoMatch.as_str()))
                .load::<RawAnimeItem>(&mut conn)
                .map_err(|e| e.to_string())?
        };

        if unmatched.is_empty() {
            tracing::info!("subscription={} 批次解析完成（iteration={}）", subscription_id, iteration);
            return Ok(());
        }

        // 去重標題（僅送唯一標題給 AI，降低 token 使用量）
        let unique_titles: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            unmatched.iter()
                .filter(|item| seen.insert(item.title.clone()))
                .map(|item| item.title.clone())
                .collect()
        };

        tracing::info!(
            "subscription={} 批次解析 iteration={}: {} 個唯一標題",
            subscription_id, iteration, unique_titles.len()
        );

        // 建立 pending record（status=generating）
        let now = Utc::now().naive_utc();
        let batch_source = format!("批次解析：{} 個標題（iteration {}）", unique_titles.len(), iteration + 1);
        let pending_id = {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            diesel::insert_into(pending_ai_results::table)
                .values(NewPendingAiResult {
                    result_type: "parser".to_string(),
                    source_title: batch_source,
                    generated_data: None,
                    status: "generating".to_string(),
                    error_message: None,
                    raw_item_id: None,
                    used_fixed_prompt: fixed_prompt.clone(),
                    used_custom_prompt: custom_prompt.clone(),
                    expires_at: None,
                    created_at: now,
                    updated_at: now,
                    subscription_id: Some(subscription_id),
                })
                .returning(pending_ai_results::id)
                .get_result::<i32>(&mut conn)
                .map_err(|e| e.to_string())?
        };

        // 建立 AI client
        let client_opt = {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            match build_ai_client(&mut conn) {
                Ok(c) => c,
                Err(e) => {
                    update_pending_failed(&pool, pending_id, &e).await.ok();
                    return Err(e);
                }
            }
        };
        let client = match client_opt {
            Some(c) => c,
            None => {
                update_pending_failed(&pool, pending_id, "AI 未設定").await.ok();
                return Ok(());
            }
        };

        // 呼叫 AI
        let system = build_system_prompt(Some(&fixed_prompt));
        let user = build_parser_batch_user_prompt(&unique_titles, custom_prompt.as_deref());
        let json_str = match client.chat_completion_structured(&system, &user, &parser_schema()).await {
            Ok(s) => s,
            Err(e) => {
                update_pending_failed(&pool, pending_id, &e.to_string()).await.ok();
                return Err(e.to_string());
            }
        };

        tracing::debug!("AI batch 回應 pending_id={}: {:?}", pending_id, json_str);

        // 解析 AI 回應
        let extracted = super::extract_json(&json_str);
        if extracted.is_empty() {
            update_pending_failed(&pool, pending_id, "AI 回應為空或無法提取 JSON").await.ok();
            break;
        }
        let sanitized = super::sanitize_ai_json(extracted);
        let mut data: Value = match serde_json::from_str(&sanitized) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Batch JSON 解析失敗 pending_id={}: {}\nraw: {:?}", pending_id, e, json_str);
                update_pending_failed(
                    &pool, pending_id,
                    &format!("JSON 解析失敗: {}（原始回應長度: {} 字元）", e, json_str.len()),
                ).await.ok();
                continue;
            }
        };

        if data.get("condition_regex").is_none() || data.get("parse_regex").is_none() {
            update_pending_failed(&pool, pending_id, "AI JSON 缺少 condition_regex/parse_regex").await.ok();
            continue;
        }

        // 修正雙重轉義的 regex 欄位（結構化輸出時部分模型會把 \[ 輸出成 \\[）
        for field in &["condition_regex", "parse_regex"] {
            if let Some(fixed) = data.get(*field)
                .and_then(|v| v.as_str())
                .map(super::fix_regex_escaping)
            {
                data[*field] = Value::String(fixed);
            }
        }

        // 驗證 regex 是否真的能匹配 AI 自己宣稱的 matched_titles
        // 避免 AI 宣稱匹配但 regex 實際無法運作（如 CJK 數字誤用 \d+）
        {
            let condition_str = data["condition_regex"].as_str().unwrap_or("");
            let parse_str = data["parse_regex"].as_str().unwrap_or("");
            let claimed: Vec<&str> = data["matched_titles"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();

            if !claimed.is_empty() {
                let actually_matched = claimed.iter().filter(|title| {
                    let ok_cond = regex::Regex::new(condition_str)
                        .map(|re| re.is_match(title))
                        .unwrap_or(false);
                    let ok_parse = regex::Regex::new(parse_str)
                        .map(|re| re.is_match(title))
                        .unwrap_or(false);
                    ok_cond && ok_parse
                }).count();

                let match_rate = actually_matched as f32 / claimed.len() as f32;
                tracing::info!(
                    "Regex 驗證 pending_id={}: AI 宣稱 {} 個匹配，實際 {} 個（{:.0}%）",
                    pending_id, claimed.len(), actually_matched, match_rate * 100.0
                );

                if actually_matched == 0 {
                    update_pending_failed(
                        &pool, pending_id,
                        &format!(
                            "Regex 驗證失敗：AI 宣稱匹配 {} 個標題，但 condition_regex/parse_regex 實際一個都不匹配（可能有 CJK 數字或大小寫問題）",
                            claimed.len()
                        ),
                    ).await.ok();
                    continue; // 讓 AI 重試，不放棄剩餘 iteration
                }
            }
        }

        // 建立解析器（is_enabled=true，待使用者確認）
        let parser_id = match create_unconfirmed_parser(&pool, &data, pending_id).await {
            Ok(id) => id,
            Err(e) => {
                update_pending_failed(&pool, pending_id, &e).await.ok();
                break;
            }
        };
        tracing::info!("批次 parser_id={} 已建立，subscription={}", parser_id, subscription_id);
        update_pending_success(&pool, pending_id, data).await.ok();

        // 載入新解析器並套用到所有 no_match 項目
        let new_parser: TitleParser = {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            title_parsers::table
                .filter(title_parsers::parser_id.eq(parser_id))
                .first::<TitleParser>(&mut conn)
                .map_err(|e| e.to_string())?
        };

        let mut parsed_count = 0;
        let mut new_link_ids: Vec<i32> = Vec::new();

        for item in &unmatched {
            match TitleParserService::try_parser(&new_parser, &item.title) {
                Ok(Some(parsed)) => {
                    let mut conn = match pool.get() {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::warn!("DB 連線失敗 item={}: {}", item.item_id, e);
                            continue;
                        }
                    };
                    match crate::handlers::fetcher_results::process_parsed_result(&mut conn, item, &parsed) {
                        Ok(link_ids) => {
                            new_link_ids.extend(link_ids);
                            TitleParserService::update_raw_item_status(
                                &mut conn, item.item_id, ParseStatus::Parsed,
                                Some(parsed.parser_id), None,
                            ).ok();
                            parsed_count += 1;
                        }
                        Err(e) => {
                            TitleParserService::update_raw_item_status(
                                &mut conn, item.item_id, ParseStatus::Failed,
                                Some(parsed.parser_id), Some(&e),
                            ).ok();
                        }
                    }
                }
                Ok(None) => {} // 維持 no_match，等下一輪
                Err(e) => {
                    tracing::warn!("try_parser 錯誤 item={}: {}", item.item_id, e);
                }
            }
        }

        tracing::info!(
            "Batch iteration={}: parser={} 匹配 {}/{} 項目",
            iteration, parser_id, parsed_count, unmatched.len()
        );

        // 派送新建立的下載連結，並觸發 conflict detection
        if !new_link_ids.is_empty() {
            let dispatch = crate::services::DownloadDispatchService::new(pool.as_ref().clone());
            match dispatch.dispatch_new_links(new_link_ids).await {
                Ok(result) => {
                    tracing::info!(
                        "批次解析後派送：dispatched={}, no_downloader={}, failed={}",
                        result.dispatched, result.no_downloader, result.failed
                    );
                }
                Err(e) => tracing::warn!("批次解析後派送失敗: {}", e),
            }

            // 建立新 links 後觸發 conflict detection，偵測新衝突並觸發批次過濾器生成
            let conflict_service = crate::services::ConflictDetectionService::new(
                std::sync::Arc::new(crate::db::repository::DieselAnimeLinkRepository::new(
                    pool.as_ref().clone(),
                )),
                std::sync::Arc::new(crate::db::repository::DieselAnimeLinkConflictRepository::new(
                    pool.as_ref().clone(),
                )),
                pool.clone(),
            );
            if let Err(e) = conflict_service.detect_and_mark_conflicts().await {
                tracing::warn!("批次解析後 conflict detection 失敗: {}", e);
            }
        }

        // 若此輪沒有新解析，停止避免無限重試
        if parsed_count == 0 {
            tracing::warn!("Batch iteration={}: 解析器未匹配任何項目，停止重試", iteration);
            break;
        }
    }

    Ok(())
}

async fn update_pending_failed(
    pool: &Arc<DbPool>,
    pending_id: i32,
    error: &str,
) -> Result<PendingAiResult, String> {
    tracing::warn!("AI parser 生成失敗 pending_id={}: {}", pending_id, error);
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    diesel::update(pending_ai_results::table.find(pending_id))
        .set((
            pending_ai_results::status.eq("failed"),
            pending_ai_results::error_message.eq(error),
            pending_ai_results::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<PendingAiResult>(&mut conn)
        .map_err(|e| e.to_string())
}
