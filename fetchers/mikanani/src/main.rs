use axum::{
    routing::{get, post},
    Router,
};
use fetcher_mikanani::{FetcherConfig, HttpClient, RealHttpClient};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber;

mod cors;
mod handlers;

use handlers::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    dotenv::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("fetcher_mikanani=debug".parse()?),
        )
        .init();

    tracing::info!("Starting Mikanani fetcher service");

    // Load configuration
    let config = FetcherConfig::from_env();

    // Create HTTP client for registration
    let http_client = Arc::new(RealHttpClient::new());

    // Register to core service
    register_to_core(&config, &*http_client).await?;

    // Create app state
    let app_state = AppState::new();

    // Build router with state
    let mut app = Router::new()
        .route("/fetch", post(handlers::fetch))
        .route("/health", get(handlers::health_check))
        .route(
            "/can-handle-subscription",
            post(handlers::can_handle_subscription),
        )
        .with_state(app_state);

    // 有條件地應用 CORS 中間件
    if let Some(cors) = cors::create_cors_layer() {
        app = app.layer(cors);
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], config.service_port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Mikanani fetcher service listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn register_to_core<C: HttpClient>(
    config: &FetcherConfig,
    http_client: &C,
) -> anyhow::Result<()> {
    let registration = shared::ServiceRegistration {
        service_type: shared::ServiceType::Fetcher,
        service_name: config.service_name.clone(),
        host: config.service_host.clone(),
        port: config.service_port,
        capabilities: shared::Capabilities {
            fetch_endpoint: Some("/fetch".to_string()),
            download_endpoint: None,
            sync_endpoint: None,
            supported_download_types: vec![],
        },
    };

    let url = config.register_url();
    http_client.post_json(&url, &registration).await?;

    tracing::info!("已向核心服務註冊: {}", url);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fetcher_mikanani::http_client::mock::MockHttpClient;
    use reqwest::StatusCode;

    #[tokio::test]
    async fn test_register_to_core_sends_correct_request() {
        let config = FetcherConfig::for_test();
        let mock_client = MockHttpClient::with_response(StatusCode::OK, "{}");

        let result = register_to_core(&config, &mock_client).await;

        assert!(result.is_ok());

        let requests = mock_client.get_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].0.contains("/services/register"));
        assert!(requests[0].1.contains("mikanani"));
        assert!(requests[0].1.contains("fetcher"));
    }

    #[tokio::test]
    async fn test_register_to_core_handles_error() {
        let config = FetcherConfig::for_test();
        let mock_client = MockHttpClient::with_error(fetcher_mikanani::HttpError::RequestFailed(
            "connection refused".to_string(),
        ));

        let result = register_to_core(&config, &mock_client).await;

        assert!(result.is_err());
    }
}
