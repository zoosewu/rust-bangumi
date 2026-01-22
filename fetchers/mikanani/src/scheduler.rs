use std::time::Duration;
use tokio::time::interval;
use crate::RssParser;
use std::sync::Arc;

pub struct FetchScheduler {
    parser: Arc<RssParser>,
    rss_url: String,
    interval: Duration,
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
