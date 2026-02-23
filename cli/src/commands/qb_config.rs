use crate::client::ApiClient;
use crate::output;
use anyhow::Result;
use clap::Subcommand;
use serde::Serialize;

#[derive(Subcommand)]
pub enum QbConfigAction {
    /// 設定 qBittorrent 帳密
    #[command(about = "設定 qBittorrent WebUI 帳號與密碼")]
    SetCredentials {
        /// qBittorrent WebUI 帳號
        #[arg(long, short = 'u')]
        user: String,
        /// qBittorrent WebUI 密碼
        #[arg(long, short = 'p')]
        password: String,
        /// Downloader Service URL（或設定環境變數 BANGUMI_DOWNLOADER_URL）
        #[arg(
            long,
            env = "BANGUMI_DOWNLOADER_URL",
            default_value = "http://localhost:8002"
        )]
        downloader_url: String,
    },
}

#[derive(Serialize)]
struct CredentialsRequest<'a> {
    username: &'a str,
    password: &'a str,
}

pub async fn run(_client: &ApiClient, action: QbConfigAction, json: bool) -> Result<()> {
    match action {
        QbConfigAction::SetCredentials {
            user,
            password,
            downloader_url,
        } => {
            // 使用 downloader_url 建立獨立的 client，不使用主 api_url
            let dl_client = ApiClient::new(downloader_url.clone());
            let req = CredentialsRequest {
                username: &user,
                password: &password,
            };
            let resp: serde_json::Value =
                dl_client.post("/config/credentials", &req).await?;
            if json {
                output::print_json(&resp);
                return Ok(());
            }
            output::print_success("qBittorrent 帳密已設定");
            println!("  帳號: {}", user);
            println!("  Downloader URL: {}", downloader_url);
        }
    }
    Ok(())
}
