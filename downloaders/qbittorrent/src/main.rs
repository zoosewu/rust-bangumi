use axum::{
    routing::{get, post},
    Router,
};
use downloader_qbittorrent::QBittorrentClient;
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
        .route("/download", post(handlers::download::<QBittorrentClient>))
        .route("/health", get(handlers::health_check))
        .with_state(client);

    let listener = TcpListener::bind("0.0.0.0:8002").await?;
    tracing::info!("Download service listening on 0.0.0.0:8002");

    axum::serve(listener, app).await?;
    Ok(())
}
