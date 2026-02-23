use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum AnimeAction {
    /// 列出所有動畫
    #[command(about = "列出所有動畫條目")]
    List,

    /// 新增動畫
    #[command(about = "新增動畫條目")]
    Add {
        /// 動畫標題
        title: String,
    },

    /// 刪除動畫
    #[command(about = "刪除動畫條目")]
    Delete {
        /// 動畫 ID
        id: i64,
    },

    /// 列出某動畫的所有系列
    #[command(about = "列出動畫下的所有系列")]
    Series {
        /// 動畫 ID
        anime_id: i64,
    },
}

#[derive(Tabled)]
struct AnimeRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "標題")]
    title: String,
    #[tabled(rename = "建立時間")]
    created_at: String,
}

pub async fn run(client: &ApiClient, action: AnimeAction, json: bool) -> Result<()> {
    match action {
        AnimeAction::List => {
            let resp: AnimesResponse = client.get("/anime").await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            if resp.animes.is_empty() {
                println!("尚無動畫");
                return Ok(());
            }
            let rows: Vec<AnimeRow> = resp
                .animes
                .iter()
                .map(|a| AnimeRow {
                    id: a.anime_id,
                    title: a.title.clone(),
                    created_at: a.created_at[..10.min(a.created_at.len())].to_string(),
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        AnimeAction::Add { title } => {
            let req = CreateAnimeRequest { title: title.clone() };
            let resp: AnimeResponse = client.post("/anime", &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!("動畫已建立: {} (ID: {})", resp.title, resp.anime_id));
        }

        AnimeAction::Delete { id } => {
            client.delete(&format!("/anime/{}", id)).await?;
            if json {
                output::print_json(&serde_json::json!({"deleted": id}));
                return Ok(());
            }
            output::print_success(&format!("動畫 #{} 已刪除", id));
        }

        AnimeAction::Series { anime_id } => {
            let resp: serde_json::Value =
                client.get(&format!("/anime/{}/series", anime_id)).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }
    Ok(())
}
