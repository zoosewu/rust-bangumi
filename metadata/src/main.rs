mod bangumi_client;
mod handlers;
mod models;

use axum::{routing::{get, post}, Router};
use handlers::AppState;
use std::sync::Arc;
use tracing::info;

async fn register_with_core(core_url: &str, host: &str, port: u16) {
    let registration = shared::ServiceRegistration {
        service_type: shared::ServiceType::Metadata,
        service_name: "bangumi".to_string(),
        host: host.to_string(),
        port,
        capabilities: shared::Capabilities {
            fetch_endpoint: None,
            download_endpoint: None,
            sync_endpoint: None,
            supported_download_types: vec![],
        },
    };
    let client = reqwest::Client::new();
    for attempt in 1..=5u32 {
        match client
            .post(format!("{}/services/register", core_url))
            .json(&registration)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                info!("Registered with Core successfully");
                return;
            }
            Ok(r) => tracing::warn!("Registration attempt {}: HTTP {}", attempt, r.status()),
            Err(e) => tracing::warn!("Registration attempt {} failed: {}", attempt, e),
        }
        if attempt < 5 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
    tracing::error!("Failed to register with Core after 5 attempts");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let port: u16 = std::env::var("PORT")
        .or_else(|_| std::env::var("SERVICE_PORT"))
        .unwrap_or_else(|_| "8005".to_string())
        .parse()
        .unwrap_or(8005);
    let service_host =
        std::env::var("SERVICE_HOST").unwrap_or_else(|_| "metadata".to_string());
    let core_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());

    let bangumi = Arc::new(bangumi_client::BangumiClient::new());
    let state = AppState { bangumi };

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/enrich/anime", post(handlers::enrich_anime))
        .route("/enrich/episodes", post(handlers::enrich_episodes))
        .with_state(state);

    let core_url_clone = core_url.clone();
    let host_clone = service_host.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        register_with_core(&core_url_clone, &host_clone, port).await;
    });

    let addr = format!("0.0.0.0:{}", port);
    info!("Metadata service listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
