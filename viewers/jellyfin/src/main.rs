use axum::{
    response::Json,
    routing::post,
    Router,
};
use std::net::SocketAddr;
use tracing_subscriber;

mod handlers;
mod file_organizer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日誌
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("viewer_jellyfin=debug".parse()?),
        )
        .init();

    tracing::info!("啟動 Jellyfin 顯示服務");

    // 註冊到主服務
    register_to_core().await?;

    // 構建應用路由
    let app = Router::new()
        .route("/sync", post(handlers::sync))
        .route("/health", post(handlers::health_check));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8003));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Jellyfin 顯示服務監聽於 {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn register_to_core() -> anyhow::Result<()> {
    let core_service_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());

    let registration = shared::ServiceRegistration {
        service_type: shared::ServiceType::Viewer,
        service_name: "jellyfin".to_string(),
        host: "viewer-jellyfin".to_string(),
        port: 8003,
        capabilities: shared::Capabilities {
            fetch_endpoint: None,
            download_endpoint: None,
            sync_endpoint: Some("/sync".to_string()),
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
