use crate::client::ApiClient;
use crate::models::*;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use tabled::{Table, Tabled};

#[derive(Subcommand)]
pub enum ConflictAction {
    /// 列出所有衝突
    #[command(about = "列出所有訂閱衝突與 Link 衝突")]
    List,

    /// 解決訂閱衝突
    #[command(about = "解決訂閱衝突，指定處理的 Fetcher")]
    Resolve {
        /// 衝突 ID
        id: i64,
        /// 指定 Fetcher ID
        #[arg(long)]
        fetcher: i64,
    },

    /// 解決 Link 衝突
    #[command(about = "解決 Link 衝突，選擇保留的 Link")]
    ResolveLink {
        /// 衝突 ID
        id: i64,
        /// 選擇保留的 Link ID
        #[arg(long)]
        link: i64,
    },
}

#[derive(Tabled)]
struct ConflictRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "類型")]
    kind: String,
    #[tabled(rename = "說明")]
    description: String,
    #[tabled(rename = "候選")]
    candidates: String,
    #[tabled(rename = "建立時間")]
    created_at: String,
}

pub async fn run(client: &ApiClient, action: ConflictAction, json: bool) -> Result<()> {
    match action {
        ConflictAction::List => {
            let conflicts: ConflictsResponse = client.get("/conflicts").await?;
            let link_conflicts: LinkConflictsResponse = client.get("/link-conflicts").await?;

            if json {
                let combined = serde_json::json!({
                    "subscription_conflicts": conflicts.conflicts,
                    "link_conflicts": link_conflicts.conflicts,
                });
                output::print_json(&combined);
                return Ok(());
            }

            let mut rows: Vec<ConflictRow> = Vec::new();

            for c in &conflicts.conflicts {
                let url = c
                    .rss_url
                    .as_deref()
                    .or(c.source_url.as_deref())
                    .unwrap_or("-");
                let fetchers: Vec<String> = c
                    .candidate_fetchers
                    .iter()
                    .map(|f| format!("{}({})", f.fetcher_name, f.fetcher_id))
                    .collect();
                rows.push(ConflictRow {
                    id: c.conflict_id,
                    kind: "訂閱衝突".to_string(),
                    description: if url.len() > 50 {
                        format!("{}...", &url[..50])
                    } else {
                        url.to_string()
                    },
                    candidates: fetchers.join(", "),
                    created_at: c.created_at.format("%Y-%m-%d %H:%M").to_string(),
                });
            }

            for lc in &link_conflicts.conflicts {
                let links: Vec<String> = lc
                    .conflicting_links
                    .iter()
                    .map(|l| format!("Link#{}", l.link_id))
                    .collect();
                rows.push(ConflictRow {
                    id: lc.conflict_id,
                    kind: "Link 衝突".to_string(),
                    description: format!("系列#{} 第{}集", lc.series_id, lc.episode_no),
                    candidates: links.join(", "),
                    created_at: lc.created_at.format("%Y-%m-%d %H:%M").to_string(),
                });
            }

            if rows.is_empty() {
                output::print_success("目前無衝突");
                return Ok(());
            }
            println!("{}", Table::new(rows));
        }

        ConflictAction::Resolve { id, fetcher } => {
            let req = ResolveConflictRequest { fetcher_id: fetcher };
            let resp: serde_json::Value =
                client.post(&format!("/conflicts/{}/resolve", id), &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!(
                "衝突 #{} 已解決（Fetcher: {}）",
                id, fetcher
            ));
        }

        ConflictAction::ResolveLink { id, link } => {
            let req = ResolveLinkConflictRequest { chosen_link_id: link };
            let resp: serde_json::Value =
                client
                    .post(&format!("/link-conflicts/{}/resolve", id), &req)
                    .await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success(&format!(
                "Link 衝突 #{} 已解決（保留 Link: {}）",
                id, link
            ));
        }
    }
    Ok(())
}
