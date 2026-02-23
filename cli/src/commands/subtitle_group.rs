use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum SubtitleGroupAction {
    /// 列出所有字幕組
    #[command(about = "列出所有字幕組")]
    List,

    /// 新增字幕組
    #[command(about = "新增字幕組")]
    Add {
        /// 字幕組名稱
        name: String,
    },

    /// 刪除字幕組
    #[command(about = "刪除字幕組")]
    Delete {
        /// 字幕組 ID
        id: i64,
    },
}

#[derive(Tabled)]
struct GroupRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "名稱")]
    name: String,
    #[tabled(rename = "建立時間")]
    created_at: String,
}

pub async fn run(client: &ApiClient, action: SubtitleGroupAction, json: bool) -> Result<()> {
    match action {
        SubtitleGroupAction::List => {
            let resp: SubtitleGroupsResponse = client.get("/subtitle-groups").await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            if resp.groups.is_empty() {
                println!("尚無字幕組");
                return Ok(());
            }
            let rows: Vec<GroupRow> = resp
                .groups
                .iter()
                .map(|g| GroupRow {
                    id: g.group_id,
                    name: g.group_name.clone(),
                    created_at: g.created_at[..10.min(g.created_at.len())].to_string(),
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        SubtitleGroupAction::Add { name } => {
            let req = CreateSubtitleGroupRequest {
                group_name: name.clone(),
            };
            let resp: SubtitleGroupResponse = client.post("/subtitle-groups", &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!(
                "字幕組已建立: {} (ID: {})",
                resp.group_name, resp.group_id
            ));
        }

        SubtitleGroupAction::Delete { id } => {
            client.delete(&format!("/subtitle-groups/{}", id)).await?;
            if json {
                output::print_json(&serde_json::json!({"deleted": id}));
                return Ok(());
            }
            output::print_success(&format!("字幕組 #{} 已刪除", id));
        }
    }
    Ok(())
}
