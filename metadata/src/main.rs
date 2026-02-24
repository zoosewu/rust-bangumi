mod bangumi_client;
mod handlers;
mod models;

use axum::{routing::{get, post}, Router};
use handlers::AppState;
use shared::ServiceRegistration;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("metadata_service=debug".parse()?),
        )
        .init();

    let bangumi = Arc::new(bangumi_client::BangumiClient::new());
    let state = AppState { bangumi };

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/enrich/anime", post(handlers::enrich_anime))
        .route("/enrich/episodes", post(handlers::enrich_episodes))
        .with_state(state);

    let service_port: u16 = std::env::var("SERVICE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8005);
    let addr = format!("0.0.0.0:{}", service_port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("無法綁定 {} — {}", addr, e);
            std::process::exit(1);
        }
    };
    tracing::info!("Metadata service listening on {}", addr);

    tokio::spawn(async { register_with_core().await });

    axum::serve(listener, app).await?;
    Ok(())
}

async fn register_with_core() {
    let core_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());
    let service_host =
        std::env::var("SERVICE_HOST").unwrap_or_else(|_| "metadata".to_string());
    let service_port: u16 = std::env::var("SERVICE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8005);

    let registration = ServiceRegistration {
        service_type: shared::ServiceType::Metadata,
        service_name: "bangumi-metadata".to_string(),
        host: service_host,
        port: service_port,
        capabilities: shared::Capabilities {
            fetch_endpoint: None,
            download_endpoint: None,
            sync_endpoint: None,
            supported_download_types: vec![],
        },
    };

    let url = format!("{}/services/register", core_url);
    let http_client = reqwest::Client::new();

    match http_client.post(&url).json(&registration).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("已向核心服務註冊: bangumi-metadata ({})", url);
        }
        Ok(resp) => {
            tracing::warn!("核心服務註冊回應非成功: {} ({})", resp.status(), url);
        }
        Err(e) => {
            tracing::warn!("無法連接核心服務進行註冊: {} ({})", e, url);
        }
    }
}
