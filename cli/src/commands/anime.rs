use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum AnimeAction {
    /// 列出所有動畫作品
    #[command(about = "列出所有動畫作品條目")]
    List,

    /// 新增動畫作品
    #[command(about = "新增動畫作品條目")]
    Add {
        /// 動畫作品標題
        title: String,
    },

    /// 刪除動畫作品
    #[command(about = "刪除動畫作品條目")]
    Delete {
        /// 動畫作品 ID
        id: i64,
    },

    /// 列出某動畫作品的所有動畫
    #[command(about = "列出動畫作品下的所有動畫")]
    Animes {
        /// 動畫作品 ID
        work_id: i64,
    },
}

#[derive(Tabled)]
struct AnimeWorkRow {
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
            let resp: AnimeWorksResponse = client.get("/anime-works").await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            if resp.animes.is_empty() {
                println!("尚無動畫作品");
                return Ok(());
            }
            let rows: Vec<AnimeWorkRow> = resp
                .animes
                .iter()
                .map(|a| AnimeWorkRow {
                    id: a.anime_id,
                    title: a.title.clone(),
                    created_at: a.created_at[..10.min(a.created_at.len())].to_string(),
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        AnimeAction::Add { title } => {
            let req = CreateAnimeWorkRequest { title: title.clone() };
            let resp: AnimeWorkResponse = client.post("/anime-works", &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!("動畫作品已建立: {} (ID: {})", resp.title, resp.anime_id));
        }

        AnimeAction::Delete { id } => {
            client.delete(&format!("/anime-works/{}", id)).await?;
            if json {
                output::print_json(&serde_json::json!({"deleted": id}));
                return Ok(());
            }
            output::print_success(&format!("動畫作品 #{} 已刪除", id));
        }

        AnimeAction::Animes { work_id } => {
            let resp: serde_json::Value =
                client.get(&format!("/anime-works/{}/anime", work_id)).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }
    Ok(())
}
