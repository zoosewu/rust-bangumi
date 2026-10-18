//! 把 mikan 的訂閱解析成作品的外部身分（`mikan/{id}`、`bgm/{subject}`）。

use async_trait::async_trait;
use shared::ExternalId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::detail_scraper::parse_bgm_subject_id;

/// 從 mikan 的單作品 RSS URL 抽出 bangumiId。
///
/// 只有 `RSS/Bangumi?bangumiId=` 這種單作品 feed 帶得出身分；
/// `RSS/MyBangumi` 這類混合 feed 回傳 None（本期不支援逐 item 反查）。
pub fn mikan_id_from_rss_url(rss_url: &str) -> Option<String> {
    reqwest::Url::parse(rss_url)
        .ok()?
        .query_pairs()
        .find(|(key, _)| key.eq_ignore_ascii_case("bangumiId"))
        .map(|(_, value)| value.into_owned())
        .filter(|id| !id.is_empty())
}

/// 取得 mikan 作品頁 HTML。抽成 trait 讓測試不打網路。
#[async_trait]
pub trait DetailPageFetcher: Send + Sync {
    async fn fetch_detail_html(&self, mikan_id: &str) -> Result<String, String>;
}

pub struct RealDetailPageFetcher {
    client: reqwest::Client,
}

impl RealDetailPageFetcher {
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

impl Default for RealDetailPageFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DetailPageFetcher for RealDetailPageFetcher {
    async fn fetch_detail_html(&self, mikan_id: &str) -> Result<String, String> {
        let response = self
            .client
            .get(format!("https://mikanani.me/Home/Bangumi/{}", mikan_id))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        response
            .text()
            .await
            .map_err(|e| format!("Read body failed: {}", e))
    }
}

/// 把 mikan id 解析成一組 external id，以 mikan id 為鍵快取：
/// 一個 feed 的每個 item 都是同一部作品，不必每次都抓作品頁。
pub struct IdentityResolver {
    fetcher: Arc<dyn DetailPageFetcher>,
    cache: Mutex<HashMap<String, Vec<ExternalId>>>,
}

impl IdentityResolver {
    pub fn new(fetcher: Arc<dyn DetailPageFetcher>) -> Self {
        Self {
            fetcher,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// mikan 身分一定有；bgm 身分要看作品頁有沒有登載。
    /// 作品頁抓取失敗時只回 mikan 身分且不快取，下次再試。
    pub async fn resolve(&self, mikan_id: &str) -> Vec<ExternalId> {
        if let Some(hit) = self.cache.lock().unwrap().get(mikan_id) {
            return hit.clone();
        }
        let mikan = ExternalId::new("mikan", mikan_id);
        match self.fetcher.fetch_detail_html(mikan_id).await {
            Ok(html) => {
                let ids: Vec<ExternalId> = std::iter::once(mikan)
                    .chain(parse_bgm_subject_id(&html).map(|id| ExternalId::new("bgm", id)))
                    .collect();
                if ids.len() == 1 {
                    tracing::warn!("mikan {} detail page has no bgm.tv link", mikan_id);
                }
                self.cache
                    .lock()
                    .unwrap()
                    .insert(mikan_id.to_string(), ids.clone());
                ids
            }
            Err(e) => {
                tracing::warn!("Failed to fetch mikan {} detail page: {}", mikan_id, e);
                vec![mikan]
            }
        }
    }

    /// 訂閱 RSS URL 的身分；不是單作品 feed 時回傳空集合。
    pub async fn resolve_rss_url(&self, rss_url: &str) -> Vec<ExternalId> {
        match mikan_id_from_rss_url(rss_url) {
            Some(mikan_id) => self.resolve(&mikan_id).await,
            None => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeFetcher {
        result: Result<String, String>,
        calls: AtomicUsize,
    }

    impl FakeFetcher {
        fn page(html: &str) -> Arc<Self> {
            Arc::new(Self {
                result: Ok(html.to_string()),
                calls: AtomicUsize::new(0),
            })
        }

        fn failing() -> Arc<Self> {
            Arc::new(Self {
                result: Err("timeout".to_string()),
                calls: AtomicUsize::new(0),
            })
        }
    }

    #[async_trait]
    impl DetailPageFetcher for FakeFetcher {
        async fn fetch_detail_html(&self, _mikan_id: &str) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.result.clone()
        }
    }

    const BGM_PAGE: &str = r#"<a href="https://bgm.tv/subject/548818">x</a>"#;

    #[test]
    fn extracts_mikan_id_from_a_bangumi_rss_url() {
        assert_eq!(
            mikan_id_from_rss_url("https://mikanani.me/RSS/Bangumi?bangumiId=3822&subgroupid=1236"),
            Some("3822".to_string())
        );
    }

    #[test]
    fn a_feed_mixing_many_anime_has_no_mikan_id() {
        assert_eq!(
            mikan_id_from_rss_url("https://mikanani.me/RSS/MyBangumi?token=abc"),
            None
        );
        assert_eq!(mikan_id_from_rss_url("not a url"), None);
    }

    #[tokio::test]
    async fn reports_both_the_mikan_and_bgm_identities() {
        let resolver = IdentityResolver::new(FakeFetcher::page(BGM_PAGE));
        assert_eq!(
            resolver.resolve("3822").await,
            vec![
                ExternalId::new("mikan", "3822"),
                ExternalId::new("bgm", "548818")
            ]
        );
    }

    #[tokio::test]
    async fn still_reports_the_mikan_identity_without_a_bgm_link() {
        let resolver = IdentityResolver::new(FakeFetcher::page("<p>no link</p>"));
        assert_eq!(
            resolver.resolve("3822").await,
            vec![ExternalId::new("mikan", "3822")]
        );
    }

    #[tokio::test]
    async fn a_feed_costs_one_page_fetch() {
        let fetcher = FakeFetcher::page(BGM_PAGE);
        let resolver = IdentityResolver::new(fetcher.clone());
        for _ in 0..3 {
            resolver.resolve("3822").await;
        }
        assert_eq!(fetcher.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_page_fetch_is_retried_next_time() {
        let fetcher = FakeFetcher::failing();
        let resolver = IdentityResolver::new(fetcher.clone());
        assert_eq!(
            resolver.resolve("3822").await,
            vec![ExternalId::new("mikan", "3822")]
        );
        resolver.resolve("3822").await;
        assert_eq!(fetcher.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn rss_url_without_mikan_id_resolves_to_nothing() {
        let fetcher = FakeFetcher::page(BGM_PAGE);
        let resolver = IdentityResolver::new(fetcher.clone());
        assert!(resolver
            .resolve_rss_url("https://mikanani.me/RSS/MyBangumi?token=abc")
            .await
            .is_empty());
        assert_eq!(fetcher.calls.load(Ordering::SeqCst), 0);
    }
}
