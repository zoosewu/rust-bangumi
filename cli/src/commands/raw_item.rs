use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum RawItemAction {
    /// 列出 Raw Items
    #[command(about = "列出 RSS 抓取記錄（可依狀態篩選）")]
    List {
        /// 狀態篩選: pending|parsed|no_match|failed|skipped
        #[arg(long, short = 's')]
        status: Option<String>,
        /// 訂閱 ID 篩選
        #[arg(long)]
        sub: Option<i64>,
        /// 返回筆數（預設 50）
        #[arg(long, default_value = "50")]
        limit: i64,
        /// 偏移量（預設 0）
        #[arg(long, default_value = "0")]
        offset: i64,
    },

    /// 顯示 Raw Item 詳情
    #[command(about = "顯示單一 Raw Item 詳情")]
    Show {
        /// Item ID
        id: i64,
    },

    /// 重新解析
    #[command(about = "重新解析指定 Raw Item")]
    Reparse {
        /// Item ID
        id: i64,
    },

    /// 標記跳過
    #[command(about = "標記 Raw Item 為跳過")]
    Skip {
        /// Item ID
        id: i64,
    },
}

#[derive(Tabled)]
struct RawItemRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "標題")]
    title: String,
    #[tabled(rename = "狀態")]
    status: String,
    #[tabled(rename = "解析標題")]
    parsed_title: String,
    #[tabled(rename = "集數")]
    episode: String,
    #[tabled(rename = "過濾")]
    filtered: String,
    #[tabled(rename = "訂閱 ID")]
    sub_id: i64,
}

pub async fn run(client: &ApiClient, action: RawItemAction, json: bool) -> Result<()> {
    match action {
        RawItemAction::List { status, sub, limit, offset } => {
            let mut params = format!("?limit={}&offset={}", limit, offset);
            if let Some(s) = &status {
                params.push_str(&format!("&status={}", s));
            }
            if let Some(sid) = sub {
                params.push_str(&format!("&subscription_id={}", sid));
            }
            let resp: RawItemsResponse =
                client.get(&format!("/raw-items{}", params)).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            println!(
                "共 {} 筆，顯示第 {}-{} 筆",
                resp.total,
                offset + 1,
                offset + resp.items.len() as i64
            );
            if resp.items.is_empty() {
                println!("（無記錄）");
                return Ok(());
            }
            let rows: Vec<RawItemRow> = resp
                .items
                .iter()
                .map(|item| RawItemRow {
                    id: item.item_id,
                    title: if item.title.len() > 40 {
                        format!("{}...", &item.title[..40])
                    } else {
                        item.title.clone()
                    },
                    status: output::format_status(&item.status),
                    parsed_title: item
                        .parsed_title
                        .as_deref()
                        .map(|t| {
                            if t.len() > 30 {
                                format!("{}...", &t[..30])
                            } else {
                                t.to_string()
                            }
                        })
                        .unwrap_or_else(|| "-".to_string()),
                    episode: item
                        .parsed_episode_no
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    filtered: item
                        .filtered_flag
                        .map(output::format_bool)
                        .unwrap_or_else(|| "-".to_string()),
                    sub_id: item.subscription_id,
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        RawItemAction::Show { id } => {
            let item: RawItemResponse = client.get(&format!("/raw-items/{}", id)).await?;
            if json {
                output::print_json(&item);
                return Ok(());
            }
            output::print_kv(
                &format!("Raw Item #{}", id),
                &[
                    ("ID", item.item_id.to_string()),
                    ("標題", item.title.clone()),
                    ("下載 URL", item.download_url.clone()),
                    ("狀態", output::format_status(&item.status)),
                    ("解析標題", output::opt_str(&item.parsed_title)),
                    (
                        "集數",
                        item.parsed_episode_no
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "過濾",
                        item.filtered_flag
                            .map(|f| f.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    ("訂閱 ID", item.subscription_id.to_string()),
                    (
                        "Parser ID",
                        item.parser_id
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "建立時間",
                        item.created_at
                            .format("%Y-%m-%d %H:%M:%S UTC")
                            .to_string(),
                    ),
                ],
            );
        }

        RawItemAction::Reparse { id } => {
            let resp: serde_json::Value =
                client.post_no_body(&format!("/raw-items/{}/reparse", id)).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!("Raw Item #{} 已重新解析", id));
        }

        RawItemAction::Skip { id } => {
            let resp: serde_json::Value =
                client.post_no_body(&format!("/raw-items/{}/skip", id)).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!("Raw Item #{} 已標記跳過", id));
        }
    }
    Ok(())
}
