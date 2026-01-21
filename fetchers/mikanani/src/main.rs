use axum::{
    routing::post,
    Router,
};
use std::net::SocketAddr;
use tracing_subscriber;

mod handlers;
mod rss_parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日誌
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("fetcher_mikanani=debug".parse()?),
        )
        .init();

    tracing::info!("啟動 Mikanani 擷取服務");

    // 註冊到主服務
    register_to_core().await?;

    // 構建應用路由
    let app = Router::new()
        .route("/fetch", post(handlers::fetch))
        .route("/health", post(handlers::health_check));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8001));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Mikanani 擷取服務監聽於 {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn register_to_core() -> anyhow::Result<()> {
    let core_service_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());

    let registration = shared::ServiceRegistration {
        service_type: shared::ServiceType::Fetcher,
        service_name: "mikanani".to_string(),
        host: "fetcher-mikanani".to_string(),
        port: 8001,
        capabilities: shared::Capabilities {
            fetch_endpoint: Some("/fetch".to_string()),
            download_endpoint: None,
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
