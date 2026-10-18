use crate::client::ApiClient;
use crate::output;
use anyhow::Result;
use colored::Colorize;
use serde::Deserialize;

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct BackfillReport {
    pub applied: bool,
    pub subscriptions: Vec<SubscriptionBackfill>,
    pub merged_duplicates: Vec<MergedDuplicate>,
    pub soft_deleted_works: Vec<WorkRef>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct SubscriptionBackfill {
    pub subscription_id: i32,
    pub name: Option<String>,
    pub external_ids: Vec<String>,
    pub outcome: String,
    pub detail: Option<String>,
    pub season: Option<SeasonRef>,
    pub previous_works: Vec<String>,
    pub links_moved: usize,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct SeasonRef {
    pub anime_id: i32,
    pub series_no: i32,
    pub series_title: String,
    pub season_title: Option<String>,
    pub aired_date: Option<String>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct MergedDuplicate {
    pub anime_id: i32,
    pub episode_no: i32,
    pub kept_download_id: i32,
    pub resolved_link_id: i32,
    pub cancelled_download_id: Option<i32>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct WorkRef {
    pub work_id: i32,
    pub title: String,
}

/// `backfill-identity`：依 fetcher 回報的身分回填既有作品與季。預設只產出報表。
pub async fn run(client: &ApiClient, apply: bool, json: bool) -> Result<()> {
    let report: BackfillReport = client
        .post(
            "/admin/identity-backfill",
            &serde_json::json!({ "apply": apply }),
        )
        .await?;

    if json {
        output::print_json(&report);
        return Ok(());
    }

    let mode = if report.applied {
        "已套用".green().bold()
    } else {
        "DRY-RUN（資料庫未變更）".yellow().bold()
    };
    println!("{} {}", "=== 身分回填 ===".bold(), mode);
    println!();

    for sub in &report.subscriptions {
        let name = sub.name.as_deref().unwrap_or("-");
        match (&sub.season, sub.outcome.as_str()) {
            (Some(season), "bound") => {
                println!(
                    "訂閱 {:>3} {}  →  {} / {}（S{}，{}）",
                    sub.subscription_id,
                    name,
                    season.series_title.bold(),
                    season.season_title.as_deref().unwrap_or("-"),
                    season.series_no,
                    season.aired_date.as_deref().unwrap_or("播出日不明"),
                );
                println!(
                    "          身分 {}；原作品 {}；搬移 {} 個 link",
                    sub.external_ids.join(", "),
                    sub.previous_works.join(" | "),
                    sub.links_moved
                );
            }
            (_, outcome) => println!(
                "訂閱 {:>3} {}  {} {}",
                sub.subscription_id,
                name,
                outcome.red(),
                sub.detail.as_deref().unwrap_or("")
            ),
        }
    }

    println!();
    println!(
        "{} {} 筆（保留合集版）",
        "合併重複下載".bold(),
        report.merged_duplicates.len()
    );
    for m in &report.merged_duplicates {
        let dropped = m
            .cancelled_download_id
            .map(|d| format!("取消下載 {}", d))
            .unwrap_or_else(|| format!("略過未下載的 link {}", m.resolved_link_id));
        println!(
            "  季 {} 第 {} 集：保留下載 {}，{}",
            m.anime_id, m.episode_no, m.kept_download_id, dropped
        );
    }

    println!();
    println!(
        "{} {} 個（沒有任何 link）",
        "Soft delete 作品".bold(),
        report.soft_deleted_works.len()
    );
    for w in &report.soft_deleted_works {
        println!("  {:>4} {}", w.work_id, w.title);
    }

    if !report.applied {
        println!();
        println!("確認無誤後加上 --apply 執行。");
    }
    Ok(())
}
