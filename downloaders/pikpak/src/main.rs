// downloaders/pikpak/src/main.rs
use axum::{
    routing::{delete, get, post},
    Router,
};
use downloader_pikpak::{handlers, PikPakClient};
use shared::{DownloadType, DownloaderClient, ServiceRegistration, ServiceType};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("downloader_pikpak=debug".parse()?),
        )
        .init();

    let db_path =
        std::env::var("PIKPAK_DB_PATH").unwrap_or_else(|_| "/data/pikpak.db".to_string());

    let client = Arc::new(PikPakClient::new(&db_path)?);

    // Auto-login if credentials provided via env
    let email = std::env::var("PIKPAK_EMAIL").unwrap_or_default();
    let password = std::env::var("PIKPAK_PASSWORD").unwrap_or_default();
    if !email.is_empty() && !password.is_empty() {
        match client.login(&email, &password).await {
            Ok(()) => {
                tracing::info!("PikPak auto-login successful");
                client.start_polling();
            }
            Err(e) => {
                tracing::warn!(
                    "PikPak auto-login failed: {e}. Use POST /config/credentials to set credentials."
                );
            }
        }
    } else {
        tracing::info!("No PIKPAK_EMAIL/PIKPAK_PASSWORD set. Use POST /config/credentials.");
    }

    let app = Router::new()
        .route(
            "/downloads",
            post(handlers::batch_download::<PikPakClient>),
        )
        .route(
            "/downloads",
            get(handlers::query_download_status::<PikPakClient>),
        )
        .route(
            "/downloads/cancel",
            post(handlers::batch_cancel::<PikPakClient>),
        )
        .route(
            "/downloads/:hash/pause",
            post(handlers::pause::<PikPakClient>),
        )
        .route(
            "/downloads/:hash/resume",
            post(handlers::resume::<PikPakClient>),
        )
        .route(
            "/downloads/:hash",
            delete(handlers::delete_download::<PikPakClient>),
        )
        .route("/health", get(handlers::health_check))
        .route(
            "/config/credentials",
            post(handlers::set_credentials::<PikPakClient>),
        )
        .with_state(client);

    let service_port: u16 = std::env::var("SERVICE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8006);
    let addr = format!("0.0.0.0:{service_port}");
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("PikPak downloader listening on {addr}");

    // Register with Core after server is ready
    tokio::spawn(async move {
        let core_url = std::env::var("CORE_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());
        let service_host =
            std::env::var("SERVICE_HOST").unwrap_or_else(|_| "localhost".to_string());
        let port: u16 = std::env::var("SERVICE_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8006);

        let registration = ServiceRegistration {
            service_type: ServiceType::Downloader,
            service_name: std::env::var("SERVICE_NAME")
                .unwrap_or_else(|_| "pikpak-downloader".to_string()),
            host: service_host,
            port,
            capabilities: shared::Capabilities {
                fetch_endpoint: None,
                download_endpoint: Some("/downloads".to_string()),
                sync_endpoint: None,
                supported_download_types: vec![DownloadType::Magnet, DownloadType::Http],
            },
        };
        shared::register_with_core_backoff(&core_url, &registration).await;
    });

    axum::serve(listener, app).await?;
    Ok(())
}
