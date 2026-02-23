use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum FilterAction {
    /// 列出過濾規則
    #[command(about = "列出過濾規則（可依類型/目標篩選）")]
    List {
        /// 目標類型: global|anime|series|group|fetcher
        #[arg(long, short = 't')]
        r#type: Option<String>,
        /// 目標 ID（非 global 時使用）
        #[arg(long)]
        target: Option<i64>,
    },

    /// 新增過濾規則
    #[command(about = "新增過濾規則")]
    Add {
        /// 目標類型: global|anime|series|group|fetcher
        #[arg(long, short = 't')]
        r#type: String,
        /// 目標 ID
        #[arg(long)]
        target: Option<i64>,
        /// 正規式
        #[arg(long, short = 'r')]
        regex: String,
        /// 排序（預設 1）
        #[arg(long, default_value = "1")]
        order: i32,
        /// 設為負向規則（過濾掉）
        #[arg(long)]
        negative: bool,
    },

    /// 刪除過濾規則
    #[command(about = "刪除過濾規則")]
    Delete {
        /// 規則 ID
        id: i64,
    },

    /// 預覽過濾效果
    #[command(about = "預覽規則對現有資料的篩選效果")]
    Preview {
        /// 目標類型: global|anime|series|group|fetcher
        #[arg(long, short = 't')]
        r#type: Option<String>,
        /// 目標 ID
        #[arg(long)]
        target: Option<i64>,
        /// 正規式
        #[arg(long, short = 'r')]
        regex: String,
        /// 設為負向規則
        #[arg(long)]
        negative: bool,
        /// 排序（預設 1）
        #[arg(long, default_value = "1")]
        order: i32,
    },
}

#[derive(Tabled)]
struct FilterRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "目標類型")]
    target_type: String,
    #[tabled(rename = "目標 ID")]
    target_id: String,
    #[tabled(rename = "排序")]
    order: i32,
    #[tabled(rename = "方向")]
    direction: String,
    #[tabled(rename = "正規式")]
    regex: String,
}

pub async fn run(client: &ApiClient, action: FilterAction, json: bool) -> Result<()> {
    match action {
        FilterAction::List { r#type, target } => {
            let mut params = String::new();
            let mut sep = "?";
            if let Some(t) = &r#type {
                params.push_str(&format!("{}target_type={}", sep, t));
                sep = "&";
            }
            if let Some(id) = target {
                params.push_str(&format!("{}target_id={}", sep, id));
            }
            let resp: FiltersResponse =
                client.get(&format!("/filters{}", params)).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            if resp.rules.is_empty() {
                println!("尚無過濾規則");
                return Ok(());
            }
            let rows: Vec<FilterRow> = resp
                .rules
                .iter()
                .map(|r| FilterRow {
                    id: r.rule_id,
                    target_type: r.target_type.clone(),
                    target_id: r
                        .target_id
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    order: r.rule_order,
                    direction: if r.is_positive {
                        "正向".to_string()
                    } else {
                        "負向".to_string()
                    },
                    regex: r.regex_pattern.clone(),
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        FilterAction::Add { r#type, target, regex, order, negative } => {
            let req = CreateFilterRuleRequest {
                target_type: r#type,
                target_id: target,
                rule_order: order,
                is_positive: !negative,
                regex_pattern: regex,
            };
            let resp: FilterRuleResponse = client.post("/filters", &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!("過濾規則已建立 (ID: {})", resp.rule_id));
        }

        FilterAction::Delete { id } => {
            client.delete(&format!("/filters/{}", id)).await?;
            if json {
                output::print_json(&serde_json::json!({"deleted": id}));
                return Ok(());
            }
            output::print_success(&format!("過濾規則 #{} 已刪除", id));
        }

        FilterAction::Preview { r#type, target, regex, negative, order } => {
            let req = CreateFilterRuleRequest {
                target_type: r#type.unwrap_or_else(|| "global".to_string()),
                target_id: target,
                rule_order: order,
                is_positive: !negative,
                regex_pattern: regex,
            };
            let resp: serde_json::Value = client.post("/filters/preview", &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }
    Ok(())
}
