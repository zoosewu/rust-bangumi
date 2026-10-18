pub mod config;
pub mod detail_scraper;
pub mod fetch_task;
pub mod http_client;
pub mod identity_resolver;
mod rss_parser;
pub mod search_scraper;

pub use config::FetcherConfig;
pub use detail_scraper::{DetailScraper, RealDetailScraper};
pub use fetch_task::{FetchTask, FetchTaskError, FetcherResultsPayload};
pub use http_client::{HttpClient, HttpError, HttpResponse, RealHttpClient};
pub use identity_resolver::{IdentityResolver, RealDetailPageFetcher};
pub use rss_parser::RssParser;
pub use search_scraper::{RealSearchScraper, SearchScraper};
