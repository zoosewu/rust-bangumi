use clap::{Parser, Subcommand};
use std::process;

mod client;
mod commands;
mod models;
mod output;

use client::ApiClient;

#[derive(Parser)]
#[command(
    name = "bangumi",
    about = "動畫 RSS 聚合、下載與媒體庫管理系統",
    version
)]
struct Cli {
    /// Core Service URL（或設定環境變數 BANGUMI_API_URL）
    #[arg(
        global = true,
        long,
        env = "BANGUMI_API_URL",
        default_value = "http://localhost:8000"
    )]
    api_url: String,

    /// 以 JSON 格式輸出（適合腳本）
    #[arg(global = true, long)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 查看系統狀態與統計資訊
    #[command(name = "status", alias = "st")]
    Status,

    /// RSS 訂閱管理
    #[command(name = "subscription", aliases = &["sub"])]
    Subscription {
        #[command(subcommand)]
        action: commands::subscription::SubscriptionAction,
    },

    /// 動畫作品管理
    #[command(name = "anime")]
    Anime {
        #[command(subcommand)]
        action: commands::anime::AnimeAction,
    },

    /// 動畫查詢與管理
    #[command(name = "series")]
    Series {
        #[command(subcommand)]
        action: commands::series::SeriesAction,
    },

    /// Raw RSS 項目瀏覽與操作
    #[command(name = "raw-item", aliases = &["raw"])]
    RawItem {
        #[command(subcommand)]
        action: commands::raw_item::RawItemAction,
    },

    /// 衝突列表與解決
    #[command(name = "conflict")]
    Conflict {
        #[command(subcommand)]
        action: commands::conflict::ConflictAction,
    },

    /// 下載記錄查詢
    #[command(name = "download", aliases = &["dl"])]
    Download {
        #[command(subcommand)]
        action: commands::download::DownloadAction,
    },

    /// 過濾規則管理
    #[command(name = "filter")]
    Filter {
        #[command(subcommand)]
        action: commands::filter::FilterAction,
    },

    /// 標題解析器管理
    #[command(name = "parser")]
    Parser {
        #[command(subcommand)]
        action: commands::parser::ParserAction,
    },

    /// 字幕組管理
    #[command(name = "subtitle-group", aliases = &["sg"])]
    SubtitleGroup {
        #[command(subcommand)]
        action: commands::subtitle_group::SubtitleGroupAction,
    },

    /// qBittorrent 連線設定
    #[command(name = "qb-config")]
    QbConfig {
        #[command(subcommand)]
        action: commands::qb_config::QbConfigAction,
    },
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let cli = Cli::parse();
    let client = ApiClient::new(cli.api_url.clone());
    let json = cli.json;

    let result = match cli.command {
        Commands::Status => commands::status::run(&client, json).await,
        Commands::Subscription { action } => {
            commands::subscription::run(&client, action, json).await
        }
        Commands::Anime { action } => commands::anime::run(&client, action, json).await,
        Commands::Series { action } => commands::series::run(&client, action, json).await,
        Commands::RawItem { action } => commands::raw_item::run(&client, action, json).await,
        Commands::Conflict { action } => commands::conflict::run(&client, action, json).await,
        Commands::Download { action } => commands::download::run(&client, action, json).await,
        Commands::Filter { action } => commands::filter::run(&client, action, json).await,
        Commands::Parser { action } => commands::parser::run(&client, action, json).await,
        Commands::SubtitleGroup { action } => {
            commands::subtitle_group::run(&client, action, json).await
        }
        Commands::QbConfig { action } => commands::qb_config::run(&client, action, json).await,
    };

    if let Err(e) = result {
        output::print_error(&e.to_string());
        process::exit(2);
    }
}
