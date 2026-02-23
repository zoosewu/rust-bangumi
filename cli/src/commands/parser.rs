use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum ParserAction {
    /// 列出解析器
    #[command(about = "列出所有解析器")]
    List {
        #[arg(long)]
        r#type: Option<String>,
        #[arg(long)]
        target: Option<i64>,
    },

    /// 顯示解析器詳情
    #[command(about = "顯示解析器詳情")]
    Show {
        id: i64,
    },

    /// 新增解析器
    #[command(about = "新增標題解析器")]
    Add {
        /// 解析器名稱
        #[arg(long, short = 'n')]
        name: String,
        /// 優先度（數字越小越優先）
        #[arg(long, default_value = "10")]
        priority: i32,
        /// 條件正規式（符合才套用此解析器）
        #[arg(long)]
        condition: Option<String>,
        /// 解析正規式
        #[arg(long)]
        parse_regex: Option<String>,
        /// 停用
        #[arg(long)]
        disabled: bool,
        /// 建立來源類型（global|anime|series|group）
        #[arg(long, default_value = "global")]
        from_type: String,
        /// 建立來源 ID
        #[arg(long)]
        from_id: Option<i64>,
    },

    /// 更新解析器
    #[command(about = "更新解析器設定")]
    Update {
        id: i64,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        priority: Option<i32>,
        #[arg(long)]
        condition: Option<String>,
        #[arg(long)]
        parse_regex: Option<String>,
        #[arg(long, conflicts_with = "disable")]
        enable: bool,
        #[arg(long, conflicts_with = "enable")]
        disable: bool,
    },

    /// 刪除解析器
    #[command(about = "刪除解析器")]
    Delete {
        id: i64,
    },

    /// 預覽解析效果
    #[command(about = "預覽解析器對現有 Raw Items 的效果")]
    Preview {
        /// 使用現有解析器 ID
        #[arg(long)]
        id: Option<i64>,
        /// 條件正規式
        #[arg(long)]
        condition: Option<String>,
        /// 解析正規式
        #[arg(long)]
        parse_regex: Option<String>,
    },
}

#[derive(Tabled)]
struct ParserRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "名稱")]
    name: String,
    #[tabled(rename = "優先度")]
    priority: i32,
    #[tabled(rename = "條件正規式")]
    condition: String,
    #[tabled(rename = "啟用")]
    enabled: String,
    #[tabled(rename = "來源")]
    from: String,
}

pub async fn run(client: &ApiClient, action: ParserAction, json: bool) -> Result<()> {
    match action {
        ParserAction::List { r#type, target } => {
            let mut params = String::new();
            let mut sep = "?";
            if let Some(t) = &r#type {
                params.push_str(&format!("{}created_from_type={}", sep, t));
                sep = "&";
            }
            if let Some(id) = target {
                params.push_str(&format!("{}created_from_id={}", sep, id));
            }
            let resp: ParsersResponse =
                client.get(&format!("/parsers{}", params)).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            if resp.parsers.is_empty() {
                println!("尚無解析器");
                return Ok(());
            }
            let rows: Vec<ParserRow> = resp
                .parsers
                .iter()
                .map(|p| ParserRow {
                    id: p.parser_id,
                    name: p.name.clone(),
                    priority: p.priority,
                    condition: p
                        .condition_regex
                        .clone()
                        .unwrap_or_else(|| "-".to_string()),
                    enabled: output::format_bool(p.enabled),
                    from: match &p.created_from_type {
                        Some(t) => {
                            if let Some(fid) = p.created_from_id {
                                format!("{}#{}", t, fid)
                            } else {
                                t.clone()
                            }
                        }
                        None => "global".to_string(),
                    },
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        ParserAction::Show { id } => {
            let parser: ParserResponse = client.get(&format!("/parsers/{}", id)).await?;
            if json {
                output::print_json(&parser);
                return Ok(());
            }
            output::print_kv(
                &format!("解析器 #{}", id),
                &[
                    ("ID", parser.parser_id.to_string()),
                    ("名稱", parser.name.clone()),
                    ("優先度", parser.priority.to_string()),
                    ("條件正規式", output::opt_str(&parser.condition_regex)),
                    ("啟用", parser.enabled.to_string()),
                    ("來源類型", output::opt_str(&parser.created_from_type)),
                    ("來源 ID", output::opt_i64(parser.created_from_id)),
                    (
                        "建立時間",
                        parser
                            .created_at
                            .format("%Y-%m-%d %H:%M:%S UTC")
                            .to_string(),
                    ),
                ],
            );
        }

        ParserAction::Add {
            name,
            priority,
            condition,
            parse_regex,
            disabled,
            from_type,
            from_id,
        } => {
            let req = CreateParserRequest {
                name,
                priority: Some(priority),
                condition_regex: condition,
                parse_regex,
                enabled: Some(!disabled),
                created_from_type: Some(from_type),
                created_from_id: from_id,
            };
            let resp: ParserResponse = client.post("/parsers", &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!(
                "解析器已建立: {} (ID: {})",
                resp.name, resp.parser_id
            ));
        }

        ParserAction::Update {
            id,
            name,
            priority,
            condition,
            parse_regex,
            enable,
            disable,
        } => {
            let enabled = if enable {
                Some(true)
            } else if disable {
                Some(false)
            } else {
                None
            };
            let req = UpdateParserRequest {
                name,
                priority,
                condition_regex: condition,
                parse_regex,
                enabled,
            };
            let resp: ParserResponse =
                client.put(&format!("/parsers/{}", id), &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!("解析器 #{} 已更新", id));
        }

        ParserAction::Delete { id } => {
            client.delete(&format!("/parsers/{}", id)).await?;
            if json {
                output::print_json(&serde_json::json!({"deleted": id}));
                return Ok(());
            }
            output::print_success(&format!("解析器 #{} 已刪除", id));
        }

        ParserAction::Preview { id, condition, parse_regex } => {
            let body = serde_json::json!({
                "parser_id": id,
                "condition_regex": condition,
                "parse_regex": parse_regex,
            });
            let resp: serde_json::Value = client.post("/parsers/preview", &body).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }
    Ok(())
}
