use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum DownloadAction {
    /// 列出下載記錄
    #[command(about = "列出下載記錄（可依狀態篩選）")]
    List {
        /// 狀態篩選: downloading|completed|failed|paused
        #[arg(long, short = 's')]
        status: Option<String>,
        /// 返回筆數（預設 50）
        #[arg(long, default_value = "50")]
        limit: i64,
        /// 偏移量（預設 0）
        #[arg(long, default_value = "0")]
        offset: i64,
    },
}

#[derive(Tabled)]
struct DownloadRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Link ID")]
    link_id: String,
    #[tabled(rename = "狀態")]
    status: String,
    #[tabled(rename = "進度")]
    progress: String,
    #[tabled(rename = "路徑")]
    path: String,
    #[tabled(rename = "建立時間")]
    created_at: String,
}

pub async fn run(client: &ApiClient, action: DownloadAction, json: bool) -> Result<()> {
    match action {
        DownloadAction::List { status, limit, offset } => {
            let mut params = format!("?limit={}&offset={}", limit, offset);
            if let Some(s) = &status {
                params.push_str(&format!("&status={}", s));
            }
            let resp: DownloadsResponse =
                client.get(&format!("/downloads{}", params)).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            if resp.downloads.is_empty() {
                println!("尚無下載記錄");
                return Ok(());
            }
            let rows: Vec<DownloadRow> = resp
                .downloads
                .iter()
                .map(|d| DownloadRow {
                    id: d.download_id,
                    link_id: d
                        .link_id
                        .map(|l| l.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    status: output::format_status(&d.status),
                    progress: d
                        .progress
                        .map(|p| format!("{:.1}%", p * 100.0))
                        .unwrap_or_else(|| "-".to_string()),
                    path: d
                        .file_path
                        .as_deref()
                        .map(|p| {
                            if p.len() > 40 {
                                format!("...{}", &p[p.len() - 40..])
                            } else {
                                p.to_string()
                            }
                        })
                        .unwrap_or_else(|| "-".to_string()),
                    created_at: d.created_at.format("%Y-%m-%d %H:%M").to_string(),
                })
                .collect();
            println!("{}", Table::new(rows));
        }
    }
    Ok(())
}
