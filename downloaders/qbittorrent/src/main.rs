use axum::{
    response::Json,
    routing::post,
    Router,
};
use std::net::SocketAddr;
use tracing_subscriber;

mod handlers;
mod qbittorrent_client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日誌
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("downloader_qbittorrent=debug".parse()?),
        )
        .init();

    tracing::info!("啟動 qBittorrent 下載服務");

    // 註冊到主服務
    register_to_core().await?;

    // 構建應用路由
    let app = Router::new()
        .route("/download", post(handlers::download))
        .route("/health", post(handlers::health_check));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8002));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("qBittorrent 下載服務監聽於 {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn register_to_core() -> anyhow::Result<()> {
    let core_service_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());

    let registration = shared::ServiceRegistration {
        service_type: shared::ServiceType::Downloader,
        service_name: "qbittorrent".to_string(),
        host: "downloader-qbittorrent".to_string(),
        port: 8002,
        capabilities: shared::Capabilities {
            fetch_endpoint: None,
            download_endpoint: Some("/download".to_string()),
            sync_endpoint: None,
        },
    };

    let client = reqwest::Client::new();
    client
        .post(&format!("{}/services/register", core_service_url))
        .json(&registration)
        .send()
        .await?;

    tracing::info!("已向核心服務註冊");

    Ok(())
}
