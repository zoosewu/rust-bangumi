use axum::{
    routing::{get, post},
    Router, Json, http::StatusCode,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber;
use fetcher_mikanani::{RssParser, FetchScheduler};
use serde::{Deserialize, Serialize};

mod handlers;
mod subscription_handler;
mod cors;

use subscription_handler::SubscriptionBroadcastPayload;

/// Response for subscription broadcast
#[derive(Debug, Serialize, Deserialize)]
pub struct SubscriptionBroadcastResponse {
    pub status: String,
    pub message: String,
}

/// Handle subscription broadcast from core service
async fn handle_subscription_broadcast(
    Json(payload): Json<SubscriptionBroadcastPayload>,
) -> (StatusCode, Json<SubscriptionBroadcastResponse>) {
    tracing::info!("Received subscription broadcast: {:?}", payload);

    let response = SubscriptionBroadcastResponse {
        status: "received".to_string(),
        message: format!("Subscription received for {}", payload.rss_url),
    };

    (StatusCode::OK, Json(response))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("fetcher_mikanani=debug".parse()?),
        )
        .init();

    tracing::info!("Starting Mikanani fetcher service");

    // Create RSS parser
    let parser = Arc::new(RssParser::new());

    // Register to core service
    register_to_core().await?;

    // Optional: Start scheduler in background if RSS_URL is configured
    if let Ok(rss_url) = std::env::var("FETCH_RSS_URL") {
        let scheduler = FetchScheduler::new(
            parser.clone(),
            rss_url.clone(),
            std::env::var("FETCH_INTERVAL_SECS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600),
        );

        tokio::spawn(async move {
            scheduler.start().await;
        });

        tracing::info!("RSS fetch scheduler started");
    }

    // Build router with state
    let mut app = Router::new()
        .route("/fetch", post(handlers::fetch))
        .route("/health", get(handlers::health_check))
        .route("/subscribe", post(handle_subscription_broadcast))
        .with_state(parser);

    // 有條件地應用 CORS 中間件
    if let Some(cors) = cors::create_cors_layer() {
        app = app.layer(cors);
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], 8001));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Mikanani fetcher service listening on {}", addr);

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
