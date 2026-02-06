use clap::{Parser, Subcommand};

mod client;
mod commands;
mod models;

#[cfg(test)]
mod tests;

#[derive(Parser)]
#[command(name = "bangumi")]
#[command(about = "動畫 RSS 聚合、下載與媒體庫管理系統", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(global = true, long, default_value = "http://localhost:8000")]
    api_url: String,
}

#[derive(Subcommand)]
enum Commands {
    /// 訂閱管理
    Subscribe {
        /// RSS 地址
        rss_url: String,
        /// 擷取區塊名稱
        #[arg(long)]
        fetcher: String,
    },

    /// 列出動畫
    List {
        /// 動畫 ID（可選）
        #[arg(long)]
        anime_id: Option<i64>,
        /// 季度，格式: 2025/冬（可選）
        #[arg(long)]
        season: Option<String>,
    },

    /// 列出連結
    Links {
        /// 動畫 ID
        anime_id: i64,
        /// 季數（可選）
        #[arg(long)]
        series: Option<i32>,
        /// 字幕組（可選）
        #[arg(long)]
        group: Option<String>,
    },

    /// 過濾規則
    Filter {
        #[command(subcommand)]
        action: FilterAction,
    },

    /// 手動下載
    Download {
        /// 連結 ID
        link_id: i64,
        /// 下載器名稱（可選）
        #[arg(long)]
        downloader: Option<String>,
    },

    /// 查看狀態
    Status,

    /// 列出服務
    Services,

    /// 查看日誌
    Logs {
        /// 日誌類型: cron | download
        #[arg(long)]
        r#type: String,
    },
}

#[derive(Subcommand)]
enum FilterAction {
    /// 添加過濾規則
    Add {
        series_id: i64,
        group_name: String,
        rule_type: String, // positive | negative
        regex: String,
    },
    /// 列出過濾規則
    List { series_id: i64, group_name: String },
    /// 刪除過濾規則
    Remove { rule_id: i64 },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("bangumi_cli=debug".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Subscribe { rss_url, fetcher } => {
            commands::subscribe(&cli.api_url, &rss_url, &fetcher).await?
        }
        Commands::List { anime_id, season } => {
            commands::list(&cli.api_url, anime_id, season).await?
        }
        Commands::Links {
            anime_id,
            series,
            group,
        } => commands::links(&cli.api_url, anime_id, series, group).await?,
        Commands::Filter { action } => match action {
            FilterAction::Add {
                series_id,
                group_name,
                rule_type,
                regex,
            } => {
                commands::filter_add(&cli.api_url, series_id, &group_name, &rule_type, &regex)
                    .await?
            }
            FilterAction::List {
                series_id,
                group_name,
            } => commands::filter_list(&cli.api_url, series_id, &group_name).await?,
            FilterAction::Remove { rule_id } => {
                commands::filter_remove(&cli.api_url, rule_id).await?
            }
        },
        Commands::Download {
            link_id,
            downloader,
        } => commands::download(&cli.api_url, link_id, downloader).await?,
        Commands::Status => commands::status(&cli.api_url).await?,
        Commands::Services => commands::services(&cli.api_url).await?,
        Commands::Logs { r#type } => commands::logs(&cli.api_url, &r#type).await?,
    }

    Ok(())
}
