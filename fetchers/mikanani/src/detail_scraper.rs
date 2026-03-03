use async_trait::async_trait;
use scraper::{Html, Selector};
use shared::{DetailItem, DetailResponse};

#[async_trait]
pub trait DetailScraper: Send + Sync {
    async fn scrape(&self, detail_key: &str) -> Result<DetailResponse, String>;
}

pub struct RealDetailScraper {
    client: reqwest::Client,
}

impl RealDetailScraper {
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

impl Default for RealDetailScraper {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DetailScraper for RealDetailScraper {
    async fn scrape(&self, detail_key: &str) -> Result<DetailResponse, String> {
        if let Some(bangumi_id) = detail_key.strip_prefix("bangumi:") {
            scrape_bangumi(&self.client, bangumi_id).await
        } else if let Some(searchstr) = detail_key.strip_prefix("source:") {
            scrape_source(&self.client, searchstr).await
        } else {
            Err(format!("Unknown detail_key prefix: {}", detail_key))
        }
    }
}

async fn scrape_bangumi(client: &reqwest::Client, bangumi_id: &str) -> Result<DetailResponse, String> {
    let url = format!("https://mikanani.me/Home/Bangumi/{}", bangumi_id);
    let html = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Read body failed: {}", e))?;

    parse_bangumi_detail(&html, bangumi_id)
}

async fn scrape_source(client: &reqwest::Client, searchstr: &str) -> Result<DetailResponse, String> {
    let html = client
        .get("https://mikanani.me/Home/Search")
        .query(&[("searchstr", searchstr)])
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Read body failed: {}", e))?;

    parse_source_detail(&html, searchstr)
}

/// Parse the bangumi detail page for subgroup RSS subscriptions.
/// Looks for anchor tags linking to RSS feeds with subgroupid parameter.
/// Also returns the root RSS (no subgroupid) as "全部".
pub fn parse_bangumi_detail(html: &str, bangumi_id: &str) -> Result<DetailResponse, String> {
    let document = Html::parse_document(html);

    // Look for anchor tags whose href contains both bangumiId and subgroupid
    let link_sel = Selector::parse("a[href*='subgroupid']")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;

    let mut items: Vec<DetailItem> = Vec::new();
    let mut seen_subgroups = std::collections::HashSet::new();

    for element in document.select(&link_sel) {
        let href = match element.value().attr("href") {
            Some(h) => h,
            None => continue,
        };

        // Extract subgroupid from href
        let subgroup_id = href
            .split("subgroupid=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .unwrap_or("");

        if subgroup_id.is_empty() || seen_subgroups.contains(subgroup_id) {
            continue;
        }
        seen_subgroups.insert(subgroup_id.to_string());

        let subgroup_name = element.text().collect::<String>().trim().to_string();
        if subgroup_name.is_empty() {
            continue;
        }

        let rss_url = format!(
            "https://mikanani.me/RSS/Bangumi?bangumiId={}&subgroupid={}",
            bangumi_id, subgroup_id
        );

        items.push(DetailItem { subgroup_name, rss_url });
    }

    // Always add the root RSS (all subgroups) as the last item
    items.push(DetailItem {
        subgroup_name: "全部".to_string(),
        rss_url: format!("https://mikanani.me/RSS/Bangumi?bangumiId={}", bangumi_id),
    });

    Ok(DetailResponse { items })
}

/// Parse source search results and group by subgroup.
/// Each episode result title contains a subgroup in brackets: "[SubgroupName] ..."
/// Uses the title-truncated-at-last-underscore as the RSS searchstr.
pub fn parse_source_detail(html: &str, _original_searchstr: &str) -> Result<DetailResponse, String> {
    let document = Html::parse_document(html);

    let item_sel = Selector::parse("a.an-info-group")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;
    let title_sel = Selector::parse("p.an-text")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;

    let mut items: Vec<DetailItem> = Vec::new();
    let mut seen_subgroups: std::collections::HashSet<String> = std::collections::HashSet::new();

    for element in document.select(&item_sel) {
        let href = element.value().attr("href").unwrap_or("");
        if !href.contains("/Home/Episode/") {
            continue;
        }

        let title = element
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        // Extract subgroup name from "[SubgroupName] ..." format
        let subgroup_name = if title.starts_with('[') {
            title
                .find(']')
                .map(|i| title[1..i].trim().to_string())
                .unwrap_or_else(|| title.clone())
        } else {
            title.clone()
        };

        // Deduplicate by subgroup name
        if seen_subgroups.contains(&subgroup_name) {
            continue;
        }
        seen_subgroups.insert(subgroup_name.clone());

        // Compute RSS searchstr: truncate at last '_'
        let searchstr = match title.rfind('_') {
            Some(idx) => &title[..idx],
            None => &title,
        };

        let rss_url = format!(
            "https://mikanani.me/RSS/Search?searchstr={}",
            urlencoding::encode(searchstr)
        );

        items.push(DetailItem { subgroup_name, rss_url });
    }

    Ok(DetailResponse { items })
}

pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockDetailScraper {
        result: Mutex<Result<DetailResponse, String>>,
    }

    impl MockDetailScraper {
        pub fn with_items(items: Vec<DetailItem>) -> Self {
            Self {
                result: Mutex::new(Ok(DetailResponse { items })),
            }
        }

        pub fn with_error(message: impl Into<String>) -> Self {
            Self {
                result: Mutex::new(Err(message.into())),
            }
        }
    }

    #[async_trait]
    impl DetailScraper for MockDetailScraper {
        async fn scrape(&self, _detail_key: &str) -> Result<DetailResponse, String> {
            self.result.lock().unwrap().clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bangumi_detail_with_subgroups() {
        let html = r#"
            <html><body>
              <a href="/RSS/Bangumi?bangumiId=3822&subgroupid=202">花山映画</a>
              <a href="/RSS/Bangumi?bangumiId=3822&subgroupid=80">喵萌奶茶屋</a>
              <a href="/other-link">不相關連結</a>
            </body></html>
        "#;

        let result = parse_bangumi_detail(html, "3822").unwrap();
        // 2 subgroups + 1 root
        assert_eq!(result.items.len(), 3);
        assert_eq!(result.items[0].subgroup_name, "花山映画");
        assert_eq!(
            result.items[0].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3822&subgroupid=202"
        );
        assert_eq!(result.items[2].subgroup_name, "全部");
        assert_eq!(
            result.items[2].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3822"
        );
    }

    #[test]
    fn test_parse_bangumi_detail_no_subgroups_returns_root_only() {
        let html = "<html><body><p>no subgroups</p></body></html>";
        let result = parse_bangumi_detail(html, "9999").unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].subgroup_name, "全部");
    }

    #[test]
    fn test_parse_source_detail_groups_by_subgroup() {
        let html = r#"
            <html><body>
              <a class="an-info-group" href="/Home/Episode/abc">
                <p class="an-text">[KITA]金牌得主19_Ciallo</p>
              </a>
              <a class="an-info-group" href="/Home/Episode/def">
                <p class="an-text">[KITA]金牌得主18_Ciallo</p>
              </a>
              <a class="an-info-group" href="/Home/Episode/ghi">
                <p class="an-text">[SubB]金牌得主19_release</p>
              </a>
            </body></html>
        "#;

        let result = parse_source_detail(html, "[KITA]金牌").unwrap();
        // KITA appears twice (same searchstr after truncation), SubB once
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].subgroup_name, "KITA");
        assert_eq!(result.items[1].subgroup_name, "SubB");
    }

    #[test]
    fn test_parse_source_detail_title_without_brackets() {
        let html = r#"
            <html><body>
              <a class="an-info-group" href="/Home/Episode/abc">
                <p class="an-text">金牌得主19_noBrackets</p>
              </a>
            </body></html>
        "#;

        let result = parse_source_detail(html, "金牌得主19").unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].subgroup_name, "金牌得主19_noBrackets");
    }
}
