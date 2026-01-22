use std::time::Duration;
use tokio::time::interval;
use crate::RssParser;
use std::sync::Arc;

pub struct FetchScheduler {
    parser: Arc<RssParser>,
    rss_url: String,
    interval: Duration,
}

#[derive(serde::Deserialize)]
pub struct UrlResponse {
    pub urls: Vec<String>,
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

                    for url in urls {
                        match self.parser.parse_feed(&url).await {
                            Ok(animes) => {
                                let count: usize = animes.iter().map(|a| a.links.len()).sum();
                                tracing::info!("Fetch successful for {}: {} links from {} anime", url, count, animes.len());
                            }
                            Err(e) => {
                                tracing::error!("Fetch failed for {}: {}", url, e);
                            }
                        }
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
