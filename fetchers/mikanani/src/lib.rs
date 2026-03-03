pub mod config;
pub mod fetch_task;
pub mod http_client;
mod rss_parser;
pub mod search_scraper;

pub use config::FetcherConfig;
pub use fetch_task::{FetchTask, FetchTaskError, FetcherResultsPayload};
pub use http_client::{HttpClient, HttpError, HttpResponse, RealHttpClient};
pub use rss_parser::RssParser;
pub use search_scraper::{RealSearchScraper, SearchScraper};
