use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum SubscriptionAction {
    /// 列出所有訂閱
    #[command(about = "列出所有 RSS 訂閱")]
    List,

    /// 新增訂閱
    #[command(about = "新增 RSS 訂閱")]
    Add {
        /// RSS URL
        url: String,
        /// 訂閱名稱（選填）
        #[arg(long, short = 'n')]
        name: Option<String>,
        /// 抓取間隔（分鐘，預設 60）
        #[arg(long, short = 'i')]
        interval: Option<i32>,
    },

    /// 顯示訂閱詳情
    #[command(about = "顯示訂閱詳情")]
    Show {
        /// 訂閱 ID
        id: i64,
    },

    /// 更新訂閱設定
    #[command(about = "更新訂閱設定")]
    Update {
        /// 訂閱 ID
        id: i64,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        interval: Option<i32>,
        /// 啟用訂閱
        #[arg(long, conflicts_with = "inactive")]
        active: bool,
        /// 停用訂閱
        #[arg(long, conflicts_with = "active")]
        inactive: bool,
    },

    /// 刪除訂閱
    #[command(about = "刪除訂閱（--purge 完整清除含下載記錄）")]
    Delete {
        /// 訂閱 ID
        id: i64,
        /// 硬刪除（含清理下載記錄與媒體）
        #[arg(long)]
        purge: bool,
    },
}

#[derive(Tabled)]
struct SubRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "名稱")]
    name: String,
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "間隔(分)")]
    interval: i32,
    #[tabled(rename = "狀態")]
    status: String,
    #[tabled(rename = "上次抓取")]
    last_fetched: String,
}

pub async fn run(client: &ApiClient, action: SubscriptionAction, json: bool) -> Result<()> {
    match action {
        SubscriptionAction::List => {
            let resp: SubscriptionsResponse = client.get("/subscriptions").await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            if resp.subscriptions.is_empty() {
                println!("尚無訂閱");
                return Ok(());
            }
            let rows: Vec<SubRow> = resp.subscriptions.iter().map(|s| SubRow {
                id: s.subscription_id,
                name: output::opt_str(&s.name),
                url: output::truncate_str(&s.source_url, 60),
                interval: s.fetch_interval_minutes,
                status: output::format_status(if s.is_active { "active" } else { "inactive" }),
                last_fetched: s
                    .last_fetched_at
                    .as_deref()
                    .map(|t| t[..16.min(t.len())].to_string())
                    .unwrap_or_else(|| "-".to_string()),
            }).collect();
            println!("{}", Table::new(rows));
        }

        SubscriptionAction::Add { url, name, interval } => {
            let req = CreateSubscriptionRequest {
                source_url: url,
                name,
                fetch_interval_minutes: interval,
            };
            let resp: SubscriptionResponse = client.post("/subscriptions", &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!("訂閱已建立 (ID: {})", resp.subscription_id));
            println!("  URL: {}", resp.source_url);
            println!("  間隔: {} 分鐘", resp.fetch_interval_minutes);
        }

        SubscriptionAction::Show { id } => {
            let resp: SubscriptionsResponse = client.get("/subscriptions").await?;
            let sub = resp
                .subscriptions
                .iter()
                .find(|s| s.subscription_id == id)
                .ok_or_else(|| anyhow::anyhow!("找不到訂閱 ID: {}", id))?;
            if json {
                output::print_json(sub);
                return Ok(());
            }
            output::print_kv(
                &format!("訂閱 #{}", id),
                &[
                    ("ID", sub.subscription_id.to_string()),
                    ("名稱", output::opt_str(&sub.name)),
                    ("URL", sub.source_url.clone()),
                    ("間隔", format!("{} 分鐘", sub.fetch_interval_minutes)),
                    ("狀態", output::format_status(if sub.is_active { "active" } else { "inactive" })),
                    (
                        "上次抓取",
                        sub.last_fetched_at
                            .as_deref()
                            .map(|t| t[..19.min(t.len())].to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "下次抓取",
                        sub.next_fetch_at
                            .as_deref()
                            .map(|t| t[..19.min(t.len())].to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                ],
            );
        }

        SubscriptionAction::Update { id, name, interval, active, inactive } => {
            let is_active = if active {
                Some(true)
            } else if inactive {
                Some(false)
            } else {
                None
            };
            let req = UpdateSubscriptionRequest {
                name,
                fetch_interval_minutes: interval,
                is_active,
            };
            let resp: SubscriptionResponse =
                client.patch(&format!("/subscriptions/{}", id), &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!("訂閱 #{} 已更新", id));
        }

        SubscriptionAction::Delete { id, purge } => {
            let path = if purge {
                format!("/subscriptions/{}?purge=true", id)
            } else {
                format!("/subscriptions/{}", id)
            };
            client.delete(&path).await?;
            if json {
                output::print_json(&serde_json::json!({"deleted": id, "purge": purge}));
                return Ok(());
            }
            output::print_success(&format!(
                "訂閱 #{} 已刪除{}",
                id,
                if purge { "（含完整清除）" } else { "" }
            ));
        }
    }
    Ok(())
}
