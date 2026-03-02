use axum::{
    routing::{get, post},
    Router,
};
use fetcher_mikanani::FetcherConfig;
use std::net::SocketAddr;

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

    // Create app state
    let app_state = AppState::new();

    // Build router with state
    let mut app = Router::new()
        .route("/fetch", post(handlers::fetch))
        .route("/search", post(handlers::search))
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
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("無法綁定 {} — {}", addr, e);
            std::process::exit(1);
        }
    };

    tracing::info!("Mikanani fetcher service listening on {}", addr);

    // 服務就緒後才向 Core 註冊（指數退避重試直到成功）
    tokio::spawn(async move {
        let registration = shared::ServiceRegistration {
            service_type: shared::ServiceType::Fetcher,
            service_name: config.service_name.clone(),
            host: config.service_host.clone(),
            port: config.service_port,
            capabilities: shared::Capabilities {
                fetch_endpoint: Some("/fetch".to_string()),
                search_endpoint: Some("/search".to_string()),
                download_endpoint: None,
                sync_endpoint: None,
                supported_download_types: vec![],
            },
        };
        let core_url = config.register_url().replace("/services/register", "");
        shared::register_with_core_backoff(&core_url, &registration).await;
    });

    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
async fn register_to_core<C: fetcher_mikanani::HttpClient>(
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
            search_endpoint: None,
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
