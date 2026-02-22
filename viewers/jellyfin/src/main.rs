use axum::{
    routing::{get, post},
    Router,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::sync::Arc;
use tokio::net::TcpListener;

mod bangumi_client;
mod db;
mod file_organizer;
mod handlers;
mod models;
mod nfo_generator;
mod schema;

use file_organizer::FileOrganizer;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Clone)]
pub struct AppState {
    pub organizer: Arc<FileOrganizer>,
    pub db: db::DbPool,
    pub bangumi: Arc<bangumi_client::BangumiClient>,
}

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

    // Initialize file organizer with paths from environment or defaults
    let source_dir = std::env::var("DOWNLOADS_DIR").unwrap_or_else(|_| "/downloads".to_string());
    let library_dir =
        std::env::var("JELLYFIN_LIBRARY_DIR").unwrap_or_else(|_| "/media/jellyfin".to_string());

    let organizer = Arc::new(FileOrganizer::new(
        std::path::PathBuf::from(source_dir),
        std::path::PathBuf::from(library_dir),
    ));

    // Initialize database
    let database_url = std::env::var("VIEWER_DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://bangumi:bangumi_dev_password@localhost:5432/viewer_jellyfin".to_string()
    });
    let db_pool = db::create_pool(&database_url);

    // Run embedded migrations
    match db_pool.get() {
        Ok(mut conn) => match conn.run_pending_migrations(MIGRATIONS) {
            Ok(applied) => {
                if !applied.is_empty() {
                    tracing::info!("Applied {} database migrations", applied.len());
                }
            }
            Err(e) => tracing::warn!("Database migration failed: {}", e),
        },
        Err(e) => tracing::warn!("Could not get DB connection for migrations: {}", e),
    }

    let bangumi = Arc::new(bangumi_client::BangumiClient::new());

    let state = AppState {
        organizer,
        db: db_pool,
        bangumi,
    };

    tracing::info!("AppState initialized with DB pool and BangumiClient");

    // Build application routes
    let app = Router::new()
        .route("/sync", post(handlers::sync))
        .route("/resync", post(handlers::resync))
        .route("/delete", post(handlers::delete_synced))
        .route("/health", get(handlers::health_check))
        .with_state(state);

    let port: u16 = std::env::var("SERVICE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8003);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("無法綁定 {} — {}", addr, e);
            std::process::exit(1);
        }
    };

    tracing::info!("Jellyfin Viewer Service listening on {}", addr);

    // 服務就緒後才向 Core 註冊
    tokio::spawn(async { register_to_core().await });

    axum::serve(listener, app).await?;

    Ok(())
}

async fn register_to_core() -> anyhow::Result<()> {
    let core_service_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());

    let service_host =
        std::env::var("SERVICE_HOST").unwrap_or_else(|_| "viewer-jellyfin".to_string());

    let registration = shared::ServiceRegistration {
        service_type: shared::ServiceType::Viewer,
        service_name: "jellyfin".to_string(),
        host: service_host,
        port: std::env::var("SERVICE_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8003),
        capabilities: shared::Capabilities {
            fetch_endpoint: None,
            download_endpoint: None,
            sync_endpoint: Some("/sync".to_string()),
            supported_download_types: vec![],
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
