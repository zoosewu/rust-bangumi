use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use futures::future::join_all;
use serde::Deserialize;
use shared::{
    AggregatedSearchResponse, AggregatedSearchResult, SearchRequest, SearchResponse, ServiceType,
};
use std::time::Duration;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SearchQueryParams {
    pub q: Option<String>,
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchQueryParams>,
) -> (StatusCode, Json<AggregatedSearchResponse>) {
    let query = params.q.unwrap_or_default();
    let query = query.trim().to_string();

    if query.is_empty() {
        return (
            StatusCode::OK,
            Json(AggregatedSearchResponse { results: vec![] }),
        );
    }

    // Collect fetchers that support search
    let fetchers = match state.registry.get_services_by_type(&ServiceType::Fetcher) {
        Ok(services) => services
            .into_iter()
            .filter(|s| s.capabilities.search_endpoint.is_some())
            .collect::<Vec<_>>(),
        Err(e) => {
            tracing::error!("Failed to get fetchers from registry: {}", e);
            return (
                StatusCode::OK,
                Json(AggregatedSearchResponse { results: vec![] }),
            );
        }
    };

    if fetchers.is_empty() {
        tracing::warn!("No fetchers with search_endpoint registered");
        return (
            StatusCode::OK,
            Json(AggregatedSearchResponse { results: vec![] }),
        );
    }

    let client = reqwest::Client::new();
    let search_request = SearchRequest {
        query: query.clone(),
    };

    // Fan out in parallel — each fetcher gets a 10s timeout
    let tasks = fetchers.into_iter().map(|fetcher| {
        let client = client.clone();
        let req = search_request.clone();
        let base_url = format!("http://{}:{}", fetcher.host, fetcher.port);
        let endpoint = fetcher.capabilities.search_endpoint.clone().unwrap();
        let url = format!("{}{}", base_url, endpoint);
        let source = fetcher.service_name.clone();

        async move {
            let result = tokio::time::timeout(
                Duration::from_secs(20),
                client.post(&url).json(&req).send(),
            )
            .await;

            match result {
                Ok(Ok(resp)) => match resp.json::<SearchResponse>().await {
                    Ok(sr) => sr
                        .results
                        .into_iter()
                        .map(|r| AggregatedSearchResult {
                            title: r.title,
                            thumbnail_url: r.thumbnail_url,
                            detail_key: r.detail_key,
                            source: source.clone(),
                        })
                        .collect::<Vec<_>>(),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse search response from {}: {}",
                            source,
                            e
                        );
                        vec![]
                    }
                },
                Ok(Err(e)) => {
                    tracing::warn!("Search request to {} failed: {}", source, e);
                    vec![]
                }
                Err(_) => {
                    tracing::warn!("Search request to {} timed out after 20s", source);
                    vec![]
                }
            }
        }
    });

    let results: Vec<AggregatedSearchResult> = join_all(tasks)
        .await
        .into_iter()
        .flatten()
        .collect();

    tracing::info!("Search '{}' returned {} results", query, results.len());
    (StatusCode::OK, Json(AggregatedSearchResponse { results }))
}
