use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum SeriesAction {
    /// 列出所有動畫系列
    #[command(about = "列出所有動畫系列（含集數統計）")]
    List {
        /// 篩選特定動畫 ID
        #[arg(long)]
        anime: Option<i64>,
    },

    /// 顯示系列詳情
    #[command(about = "顯示系列詳情")]
    Show {
        /// 系列 ID
        id: i64,
    },

    /// 更新系列元資料
    #[command(about = "更新系列元資料")]
    Update {
        /// 系列 ID
        id: i64,
        #[arg(long)]
        description: Option<String>,
        #[arg(long, help = "開播日期，格式: YYYY-MM-DD")]
        aired_date: Option<String>,
        #[arg(long, help = "完結日期，格式: YYYY-MM-DD")]
        end_date: Option<String>,
        #[arg(long)]
        season_id: Option<i64>,
    },

    /// 列出系列的所有集數連結
    #[command(about = "列出系列集數與下載狀態")]
    Links {
        /// 系列 ID
        id: i64,
    },
}

#[derive(Tabled)]
struct SeriesRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "動畫")]
    anime: String,
    #[tabled(rename = "季")]
    series_no: i32,
    #[tabled(rename = "播出季")]
    season: String,
    #[tabled(rename = "已下載")]
    downloaded: i64,
    #[tabled(rename = "已找到")]
    found: i64,
}

#[derive(Tabled)]
struct LinkRow {
    #[tabled(rename = "Link ID")]
    link_id: i64,
    #[tabled(rename = "集")]
    episode: i32,
    #[tabled(rename = "字幕組")]
    group: String,
    #[tabled(rename = "過濾")]
    filtered: String,
    #[tabled(rename = "衝突")]
    conflict: String,
    #[tabled(rename = "下載狀態")]
    dl_status: String,
}

pub async fn run(client: &ApiClient, action: SeriesAction, json: bool) -> Result<()> {
    match action {
        SeriesAction::List { anime } => {
            let resp: SeriesListResponse = client.get("/series").await?;
            let mut series = resp.series;
            if let Some(anime_id) = anime {
                series.retain(|s| s.anime_id == anime_id);
            }
            if json {
                output::print_json(&series);
                return Ok(());
            }
            if series.is_empty() {
                println!("尚無系列");
                return Ok(());
            }
            let rows: Vec<SeriesRow> = series
                .iter()
                .map(|s| SeriesRow {
                    id: s.series_id,
                    anime: s.anime_title.clone(),
                    series_no: s.series_no,
                    season: s
                        .season
                        .as_ref()
                        .map(|se| format!("{} {}", se.year, se.season))
                        .unwrap_or_else(|| "-".to_string()),
                    downloaded: s.episode_downloaded,
                    found: s.episode_found,
                })
                .collect();
            println!("{}", Table::new(rows));
        }

        SeriesAction::Show { id } => {
            let resp: AnimeSeriesRichResponse =
                client.get(&format!("/anime/series/{}", id)).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            let season_str = resp
                .season
                .as_ref()
                .map(|s| format!("{} {}", s.year, s.season))
                .unwrap_or_else(|| "-".to_string());
            let subs: Vec<String> = resp
                .subscriptions
                .iter()
                .map(|s| {
                    s.name
                        .clone()
                        .unwrap_or_else(|| format!("#{}", s.subscription_id))
                })
                .collect();
            output::print_kv(
                &format!("系列 #{}", id),
                &[
                    ("ID", resp.series_id.to_string()),
                    ("動畫", resp.anime_title.clone()),
                    ("季號", format!("S{}", resp.series_no)),
                    ("播出季", season_str),
                    ("已下載", resp.episode_downloaded.to_string()),
                    ("已找到", resp.episode_found.to_string()),
                    ("說明", output::opt_str(&resp.description)),
                    ("開播", output::opt_str(&resp.aired_date)),
                    ("完結", output::opt_str(&resp.end_date)),
                    ("訂閱", subs.join(", ")),
                ],
            );
        }

        SeriesAction::Update { id, description, aired_date, end_date, season_id } => {
            let req = UpdateSeriesRequest {
                season_id,
                description,
                aired_date,
                end_date,
            };
            let resp: serde_json::Value =
                client.put(&format!("/anime/series/{}", id), &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!("系列 #{} 已更新", id));
        }

        SeriesAction::Links { id } => {
            let resp: LinksResponse = client.get(&format!("/links/{}", id)).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            if resp.links.is_empty() {
                println!("尚無集數連結");
                return Ok(());
            }
            let rows: Vec<LinkRow> = resp
                .links
                .iter()
                .map(|l| LinkRow {
                    link_id: l.link_id,
                    episode: l.episode_no,
                    group: l
                        .group_name
                        .clone()
                        .unwrap_or_else(|| format!("#{}", l.group_id.unwrap_or(0))),
                    filtered: output::format_bool(l.filtered_flag),
                    conflict: if l.conflict_flag {
                        output::format_bool(true)
                    } else {
                        "-".to_string()
                    },
                    dl_status: l
                        .download
                        .as_ref()
                        .map(|d| output::format_status(&d.status))
                        .unwrap_or_else(|| "-".to_string()),
                })
                .collect();
            println!("{}", Table::new(rows));
        }
    }
    Ok(())
}
