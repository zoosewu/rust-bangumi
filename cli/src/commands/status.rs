use crate::client::ApiClient;
use crate::models::DashboardStats;
use crate::output;
use anyhow::Result;
use colored::Colorize;

pub async fn run(client: &ApiClient, json: bool) -> Result<()> {
    let stats: DashboardStats = client.get("/dashboard/stats").await?;

    if json {
        output::print_json(&stats);
        return Ok(());
    }

    println!("{}", "=== 系統統計 ===".bold());
    println!("  動畫總數:     {}", stats.total_anime);
    println!("  系列總數:     {}", stats.total_series);
    println!("  活躍訂閱:     {}", stats.active_subscriptions);
    println!();

    println!("{}", "=== 下載狀態 ===".bold());
    println!("  下載中:       {}", stats.downloading.to_string().yellow());
    println!("  已完成:       {}", stats.completed.to_string().green());
    println!("  失敗:         {}", stats.failed.to_string().red());
    println!("  總計:         {}", stats.total_downloads);
    println!();

    println!("{}", "=== 待處理 ===".bold());
    if stats.pending_raw_items > 0 {
        println!("  待解析 Raw Items: {}", stats.pending_raw_items.to_string().yellow());
    } else {
        println!("  待解析 Raw Items: {}", "0".green());
    }
    if stats.pending_conflicts > 0 {
        println!("  待解決衝突:       {}", stats.pending_conflicts.to_string().red());
    } else {
        println!("  待解決衝突:       {}", "0".green());
    }
    println!();

    println!("{}", "=== 服務狀態 ===".bold());
    for svc in &stats.services {
        let status = if svc.is_healthy {
            "✓ 健康".green().to_string()
        } else {
            "✗ 不健康".red().to_string()
        };
        println!("  [{:10}] {}: {}", svc.module_type, svc.name, status);
    }

    Ok(())
}
