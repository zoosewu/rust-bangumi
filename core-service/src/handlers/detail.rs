use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use shared::{DetailRequest, DetailResponse, ServiceType};
use std::time::Duration;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CoreDetailRequest {
    pub detail_key: String,
    pub source: String,
}

pub async fn detail(
    State(state): State<AppState>,
    Json(payload): Json<CoreDetailRequest>,
) -> (StatusCode, Json<DetailResponse>) {
    // Find the fetcher matching the given source name
    let fetcher = match state.registry.get_services_by_type(&ServiceType::Fetcher) {
        Ok(services) => services
            .into_iter()
            .find(|s| s.service_name == payload.source && s.capabilities.detail_endpoint.is_some()),
        Err(e) => {
            tracing::error!("Failed to get fetchers: {}", e);
            return (StatusCode::OK, Json(DetailResponse { items: vec![] }));
        }
    };

    let fetcher = match fetcher {
        Some(f) => f,
        None => {
            tracing::warn!("No fetcher with detail_endpoint found for source={}", payload.source);
            return (StatusCode::OK, Json(DetailResponse { items: vec![] }));
        }
    };

    let endpoint = fetcher.capabilities.detail_endpoint.as_deref().unwrap_or("/detail");
    let url = format!("http://{}:{}{}", fetcher.host, fetcher.port, endpoint);
    let req_body = DetailRequest { detail_key: payload.detail_key.clone() };

    let client = reqwest::Client::new();
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        client.post(&url).json(&req_body).send(),
    )
    .await;

    match result {
        Ok(Ok(resp)) => match resp.json::<DetailResponse>().await {
            Ok(dr) => {
                tracing::info!(
                    "Detail for key={} returned {} items",
                    payload.detail_key,
                    dr.items.len()
                );
                (StatusCode::OK, Json(dr))
            }
            Err(e) => {
                tracing::warn!("Failed to parse detail response: {}", e);
                (StatusCode::OK, Json(DetailResponse { items: vec![] }))
            }
        },
        Ok(Err(e)) => {
            tracing::warn!("Detail request to {} failed: {}", url, e);
            (StatusCode::OK, Json(DetailResponse { items: vec![] }))
        }
        Err(_) => {
            tracing::warn!("Detail request to {} timed out", url);
            (StatusCode::OK, Json(DetailResponse { items: vec![] }))
        }
    }
}
