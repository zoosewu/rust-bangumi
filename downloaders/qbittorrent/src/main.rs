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
    let qb_user = std::env::var("QBITTORRENT_USER").unwrap_or_else(|_| "admin".to_string());
    let qb_pass =
        std::env::var("QBITTORRENT_PASSWORD").unwrap_or_else(|_| "adminadmin".to_string());

    let client = Arc::new(QBittorrentClient::new(qb_url));
    client.login(&qb_user, &qb_pass).await?;

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
        .with_state(client);

    let addr = "0.0.0.0:8002";
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("無法綁定 {} — {}", addr, e);
            std::process::exit(1);
        }
    };
    tracing::info!("Download service listening on {}", addr);

    // 服務就緒後才向 Core 註冊
    tokio::spawn(async { register_with_core().await });

    axum::serve(listener, app).await?;
    Ok(())
}

async fn register_with_core() {
    let core_url =
        std::env::var("CORE_SERVICE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());
    let service_name =
        std::env::var("SERVICE_NAME").unwrap_or_else(|_| "qbittorrent-downloader".to_string());
    let service_host = std::env::var("SERVICE_HOST").unwrap_or_else(|_| "localhost".to_string());
    let service_port: u16 = std::env::var("SERVICE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8002);

    let registration = ServiceRegistration {
        service_type: ServiceType::Downloader,
        service_name: service_name.clone(),
        host: service_host,
        port: service_port,
        capabilities: shared::Capabilities {
            fetch_endpoint: None,
            download_endpoint: Some("/downloads".to_string()),
            sync_endpoint: None,
            supported_download_types: vec![DownloadType::Magnet, DownloadType::Torrent],
        },
    };

    let url = format!("{}/services/register", core_url);
    let http_client = reqwest::Client::new();

    match http_client.post(&url).json(&registration).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("已向核心服務註冊: {} ({})", service_name, url);
        }
        Ok(resp) => {
            tracing::warn!("核心服務註冊回應非成功: {} ({})", resp.status(), url);
        }
        Err(e) => {
            tracing::warn!("無法連接核心服務進行註冊: {} ({})", e, url);
        }
    }
}
