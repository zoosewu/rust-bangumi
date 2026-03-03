use async_trait::async_trait;
use scraper::{Html, Selector};
use shared::SearchResult;

#[async_trait]
pub trait SearchScraper: Send + Sync {
    async fn scrape(&self, query: &str) -> Result<Vec<SearchResult>, String>;
}

pub struct RealSearchScraper {
    client: reqwest::Client,
}

impl RealSearchScraper {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .user_agent("Mozilla/5.0 (compatible; bangumi-bot/1.0)")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

impl Default for RealSearchScraper {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchScraper for RealSearchScraper {
    async fn scrape(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let response = self
            .client
            .get("https://mikanani.me/Home/Search")
            .query(&[("searchstr", query)])
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Mikanani search: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Mikanani search returned status {}",
                response.status()
            ));
        }

        let html = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        parse_search_results(&html)
    }
}

/// Parse HTML from Mikanani search results page.
pub fn parse_search_results(html: &str) -> Result<Vec<SearchResult>, String> {
    let document = Html::parse_document(html);

    let item_sel = Selector::parse("a.an-info-group")
        .map_err(|e| format!("Invalid CSS selector: {:?}", e))?;
    let title_sel = Selector::parse("p.an-text")
        .map_err(|e| format!("Invalid CSS selector: {:?}", e))?;
    let img_sel = Selector::parse("img")
        .map_err(|e| format!("Invalid CSS selector: {:?}", e))?;

    let mut results = Vec::new();

    for element in document.select(&item_sel) {
        let href = match element.value().attr("href") {
            Some(h) => h,
            None => continue,
        };

        if !href.contains("/Home/Bangumi/") {
            continue;
        }

        let bangumi_id: u32 = match href.rsplit('/').next().and_then(|s| s.parse().ok()) {
            Some(id) => id,
            None => {
                tracing::warn!("Could not parse bangumi ID from href: {}", href);
                continue;
            }
        };

        let title = element
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        let thumbnail_url = element
            .select(&img_sel)
            .next()
            .and_then(|el| el.value().attr("src"))
            .map(|src| {
                if src.starts_with("http") {
                    src.to_string()
                } else {
                    format!("https://mikanani.me{}", src)
                }
            });

        let subscription_url = format!(
            "https://mikanani.me/RSS/Bangumi?bangumiId={}",
            bangumi_id
        );

        results.push(SearchResult {
            title,
            description: None,
            thumbnail_url,
            subscription_url,
        });
    }

    tracing::info!(
        "Mikanani search parsed {} results from {} HTML bytes",
        results.len(),
        html.len()
    );

    Ok(results)
}

pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockSearchScraper {
        result: Mutex<Result<Vec<SearchResult>, String>>,
    }

    impl MockSearchScraper {
        pub fn with_results(results: Vec<SearchResult>) -> Self {
            Self {
                result: Mutex::new(Ok(results)),
            }
        }

        pub fn with_error(message: impl Into<String>) -> Self {
            Self {
                result: Mutex::new(Err(message.into())),
            }
        }
    }

    #[async_trait]
    impl SearchScraper for MockSearchScraper {
        async fn scrape(&self, _query: &str) -> Result<Vec<SearchResult>, String> {
            self.result.lock().unwrap().clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_html() {
        let result = parse_search_results("<html><body></body></html>").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_bangumi_card() {
        let html = r#"
            <html><body>
              <div class="an-ul">
                <a class="an-info-group" href="/Home/Bangumi/3310">
                  <div class="an-img-cell">
                    <img src="/images/Bangumi/3310/cover.jpg" />
                  </div>
                  <div class="an-info">
                    <p class="an-text">葬送的芙莉蓮</p>
                  </div>
                </a>
              </div>
            </body></html>
        "#;

        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "葬送的芙莉蓮");
        assert_eq!(
            results[0].subscription_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3310"
        );
        assert_eq!(
            results[0].thumbnail_url,
            Some("https://mikanani.me/images/Bangumi/3310/cover.jpg".to_string())
        );
    }

    #[test]
    fn test_parse_skips_non_bangumi_links() {
        let html = r#"
            <html><body>
              <a class="an-info-group" href="/Home/Episode/abc123">
                <p class="an-text">Some Episode</p>
              </a>
            </body></html>
        "#;
        let results = parse_search_results(html).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_absolute_thumbnail_url_unchanged() {
        let html = r#"
            <html><body>
              <a class="an-info-group" href="/Home/Bangumi/9999">
                <img src="https://cdn.example.com/cover.jpg" />
                <p class="an-text">Test Anime</p>
              </a>
            </body></html>
        "#;
        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].thumbnail_url,
            Some("https://cdn.example.com/cover.jpg".to_string())
        );
    }
}
