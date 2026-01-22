use std::time::Duration;
use tokio::time::interval;
use crate::RssParser;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use shared::FetchedAnime;

pub struct FetchScheduler {
    parser: Arc<RssParser>,
    rss_url: String,
    interval: Duration,
}

#[derive(serde::Deserialize)]
pub struct UrlResponse {
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherResultsPayload {
    pub animes: Vec<FetchedAnime>,
    pub fetcher_source: String,
}

impl FetchScheduler {
    pub fn new(parser: Arc<RssParser>, rss_url: String, interval_secs: u64) -> Self {
        Self {
            parser,
            rss_url,
            interval: Duration::from_secs(interval_secs),
        }
    }

    pub async fn start(self) {
        let mut ticker = interval(self.interval);

        loop {
            ticker.tick().await;
            tracing::info!("Scheduled fetch triggered for: {}", self.rss_url);

            match self.parser.parse_feed(&self.rss_url).await {
                Ok(animes) => {
                    let count: usize = animes.iter().map(|a| a.links.len()).sum();
                    tracing::info!("Scheduled fetch successful: {} links from {} anime", count, animes.len());
                }
                Err(e) => {
                    tracing::error!("Scheduled fetch failed: {}", e);
                }
            }
        }
    }

    /// Start scheduler with fetching URLs from core service
    pub async fn run_with_core(self) {
        let mut ticker = interval(self.interval);

        loop {
            ticker.tick().await;
            tracing::info!("Scheduled fetch triggered - fetching URLs from core service");

            // Fetch URLs from core service
            match self.fetch_urls_from_core().await {
                Ok(urls) => {
                    tracing::info!("Fetched {} URLs from core service", urls.len());

                    // Collect all parsed animes
                    let mut all_animes = Vec::new();

                    for url in urls {
                        match self.parser.parse_feed(&url).await {
                            Ok(animes) => {
                                let count: usize = animes.iter().map(|a| a.links.len()).sum();
                                tracing::info!("Fetch successful for {}: {} links from {} anime", url, count, animes.len());
                                all_animes.extend(animes);
                            }
                            Err(e) => {
                                tracing::error!("Fetch failed for {}: {}", url, e);
                            }
                        }
                    }

                    // Send results to core service if we have any animes
                    if !all_animes.is_empty() {
                        match self.send_results_to_core(all_animes).await {
                            Ok(_) => {
                                tracing::info!("Successfully sent fetched results to core service");
                            }
                            Err(e) => {
                                tracing::error!("Failed to send results to core service: {}", e);
                            }
                        }
                    } else {
                        tracing::info!("No animes fetched in this cycle");
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to fetch URLs from core service: {}", e);
                }
            }
        }
    }

    /// Fetch RSS URLs from core service
    async fn fetch_urls_from_core(&self) -> anyhow::Result<Vec<String>> {
        let core_service_url = std::env::var("CORE_SERVICE_URL")
            .unwrap_or_else(|_| "http://core-service:8000".to_string());

        let client = reqwest::Client::new();
        let response = client
            .get(&format!("{}/subscriptions/urls?source=mikanani", core_service_url))
            .send()
            .await?;

        let url_response: UrlResponse = response.json().await?;

        tracing::info!("Received {} URLs from core service", url_response.urls.len());

        Ok(url_response.urls)
    }

    /// Send fetched anime results to core service
    async fn send_results_to_core(&self, animes: Vec<FetchedAnime>) -> anyhow::Result<()> {
        let core_service_url = std::env::var("CORE_SERVICE_URL")
            .unwrap_or_else(|_| "http://core-service:8000".to_string());

        let payload = FetcherResultsPayload {
            animes,
            fetcher_source: "mikanani".to_string(),
        };

        let client = reqwest::Client::new();
        let response = client
            .post(&format!("{}/fetcher-results", core_service_url))
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            tracing::info!("Fetcher results successfully sent to core service");
            Ok(())
        } else {
            anyhow::bail!(
                "Failed to send fetcher results: HTTP {}",
                response.status()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let parser = Arc::new(RssParser::new());
        let scheduler = FetchScheduler::new(
            parser,
            "https://example.com/rss".to_string(),
            60,
        );

        assert_eq!(scheduler.interval, Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_scheduler_with_mock_parser() {
        // Create scheduler with safe test parameters
        let parser = Arc::new(RssParser::new());
        let scheduler = FetchScheduler::new(
            parser,
            "mock://test".to_string(),
            1,  // 1 second interval for testing
        );

        assert_eq!(scheduler.rss_url, "mock://test");
    }
}
