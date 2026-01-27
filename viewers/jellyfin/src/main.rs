use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;

mod handlers;
mod file_organizer;

use file_organizer::FileOrganizer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    dotenv::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("viewer_jellyfin=debug".parse()?),
        )
        .init();

    tracing::info!("Starting Jellyfin Viewer Service");

    // Register to core service
    register_to_core().await?;

    // Initialize file organizer with paths from environment or defaults
    let source_dir = std::env::var("DOWNLOADS_DIR")
        .unwrap_or_else(|_| "/downloads".to_string());
    let library_dir = std::env::var("JELLYFIN_LIBRARY_DIR")
        .unwrap_or_else(|_| "/media/jellyfin".to_string());

    let organizer = Arc::new(FileOrganizer::new(
        std::path::PathBuf::from(source_dir),
        std::path::PathBuf::from(library_dir),
    ));

    tracing::info!("File organizer initialized");

    // Build application routes
    let app = Router::new()
        .route("/sync", post(handlers::sync))
        .route("/health", get(handlers::health_check))
        .with_state(organizer);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8003));
    let listener = TcpListener::bind(addr).await?;

    tracing::info!("Jellyfin Viewer Service listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn register_to_core() -> anyhow::Result<()> {
    let core_service_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());

    // 本地開發時設為 localhost，Docker 環境使用容器名稱
    let service_host = std::env::var("SERVICE_HOST")
        .unwrap_or_else(|_| "viewer-jellyfin".to_string());

    let registration = shared::ServiceRegistration {
        service_type: shared::ServiceType::Viewer,
        service_name: "jellyfin".to_string(),
        host: service_host,
        port: 8003,
        capabilities: shared::Capabilities {
            fetch_endpoint: None,
            download_endpoint: None,
            sync_endpoint: Some("/sync".to_string()),
        },
    };

    let client = reqwest::Client::new();
    match client
        .post(&format!("{}/services/register", core_service_url))
        .json(&registration)
        .send()
        .await
    {
        Ok(_) => {
            tracing::info!("Successfully registered with core service");
            Ok(())
        }
        Err(e) => {
            tracing::warn!(
                "Failed to register with core service: {}. Continuing anyway.",
                e
            );
            Ok(())
        }
    }
}
