use axum::{
    routing::{delete, get, post},
    Router,
};
use downloader_qbittorrent::{DownloaderClient, QBittorrentClient};
use shared::{DownloadType, ServiceRegistration, ServiceType};
use std::sync::Arc;
use tokio::net::TcpListener;

mod handlers;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("downloader_qbittorrent=debug".parse()?),
        )
        .init();

    let qb_url =
        std::env::var("QBITTORRENT_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let qb_user = std::env::var("QBITTORRENT_USER").unwrap_or_default();
    let qb_pass = std::env::var("QBITTORRENT_PASSWORD").unwrap_or_default();

    let client = Arc::new(QBittorrentClient::new(qb_url));
    if !qb_user.is_empty() && !qb_pass.is_empty() {
        if let Err(e) = client.login(&qb_user, &qb_pass).await {
            tracing::warn!("qBittorrent 登入失敗: {}。請使用 'bangumi qb-login' 指令設定帳密。", e);
        }
    } else {
        tracing::info!("未設定 qBittorrent 帳密，請在 qBittorrent 初始化後執行 'bangumi qb-login' 指令。");
    }

    let app = Router::new()
        .route(
            "/downloads",
            post(handlers::batch_download::<QBittorrentClient>),
        )
        .route(
            "/downloads",
            get(handlers::query_download_status::<QBittorrentClient>),
        )
        .route(
            "/downloads/cancel",
            post(handlers::batch_cancel::<QBittorrentClient>),
        )
        .route(
            "/downloads/:hash/pause",
            post(handlers::pause::<QBittorrentClient>),
        )
        .route(
            "/downloads/:hash/resume",
            post(handlers::resume::<QBittorrentClient>),
        )
        .route(
            "/downloads/:hash",
            delete(handlers::delete_download::<QBittorrentClient>),
        )
        .route("/health", get(handlers::health_check))
        .route("/config/credentials", post(handlers::set_credentials::<QBittorrentClient>))
        .with_state(client);

    let service_port: u16 = std::env::var("SERVICE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8002);
    let addr = format!("0.0.0.0:{}", service_port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("無法綁定 {} — {}", addr, e);
            std::process::exit(1);
        }
    };
    tracing::info!("Download service listening on {}", addr);

    // 服務就緒後才向 Core 註冊（指數退避重試直到成功）
    tokio::spawn(async {
        let core_url = std::env::var("CORE_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());
        let service_name = std::env::var("SERVICE_NAME")
            .unwrap_or_else(|_| "qbittorrent-downloader".to_string());
        let service_host =
            std::env::var("SERVICE_HOST").unwrap_or_else(|_| "localhost".to_string());
        let service_port: u16 = std::env::var("SERVICE_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8002);

        let registration = ServiceRegistration {
            service_type: ServiceType::Downloader,
            service_name,
            host: service_host,
            port: service_port,
            capabilities: shared::Capabilities {
                fetch_endpoint: None,
                search_endpoint: None,
                download_endpoint: Some("/downloads".to_string()),
                sync_endpoint: None,
                supported_download_types: vec![DownloadType::Magnet, DownloadType::Torrent],
            },
        };

        shared::register_with_core_backoff(&core_url, &registration).await;
    });

    axum::serve(listener, app).await?;
    Ok(())
}
