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

async fn scrape_source(client: &reqwest::Client, query: &str) -> Result<DetailResponse, String> {
    let html = client
        .get("https://mikanani.me/Home/Search")
        .query(&[("searchstr", query)])
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Read body failed: {}", e))?;

    parse_source_detail(&html, query)
}

/// Parse bangumi detail page for per-subgroup RSS links.
///
/// Real page structure from https://mikanani.me/Home/Bangumi/{id} has TWO variants:
///
/// Variant A — "生肉/不明字幕" (plain text node):
/// ```html
/// <div class="subgroup-text" id="202">
///   生肉/不明字幕
///   <a href="/RSS/Bangumi?bangumiId=3822&subgroupid=202" class="mikan-rss">...</a>
///   <span class="subscribed" style="display:none;">已订阅</span>
///   <a class="subgroup-subscribe ...">订阅</a>
/// </div>
/// ```
///
/// Variant B — official subgroup (name in `<a href="/Home/PublishGroup/...">`) :
/// ```html
/// <div class="subgroup-text" id="1243">
///   <a href="/Home/PublishGroup/1015" style="color:#3bc0c3;">六四位元字幕组</a>
///   <a href="/RSS/Bangumi?bangumiId=3822&subgroupid=1243" class="mikan-rss">...</a>
///   <span class="subscribed" style="display:none;">已订阅</span>
///   <a class="subgroup-subscribe ...">订阅</a>
/// </div>
/// ```
///
/// We use `element.text()` (all descendant text) and take the first non-empty,
/// non-button text fragment, skipping "已订阅" and "订阅".
pub fn parse_bangumi_detail(html: &str, bangumi_id: &str) -> Result<DetailResponse, String> {
    let document = Html::parse_document(html);

    let subgroup_sel = Selector::parse("div.subgroup-text")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;

    let mut items: Vec<DetailItem> = Vec::new();

    for element in document.select(&subgroup_sel) {
        let subgroup_id = match element.value().attr("id") {
            Some(id) if !id.is_empty() => id,
            _ => continue,
        };

        // Walk ALL text descendants; the name is the first non-trivial text fragment
        // that is not the hidden "已订阅" span or the "订阅" subscribe button.
        let subgroup_name = element
            .text()
            .map(|t| t.trim())
            .find(|t| !t.is_empty() && *t != "已订阅" && *t != "订阅")
            .unwrap_or("")
            .to_string();

        if subgroup_name.is_empty() {
            continue;
        }

        let rss_url = format!(
            "https://mikanani.me/RSS/Bangumi?bangumiId={}&subgroupid={}",
            bangumi_id, subgroup_id
        );

        items.push(DetailItem { subgroup_name, rss_url });
    }

    // Always include a root RSS entry that covers all subgroups
    items.push(DetailItem {
        subgroup_name: "全部".to_string(),
        rss_url: format!("https://mikanani.me/RSS/Bangumi?bangumiId={}", bangumi_id),
    });

    Ok(DetailResponse { items })
}

/// Parse a search page for per-subgroup RSS links.
///
/// Uses the leftbar `a.subgroup-longname[data-subgroupid]` — the sidebar that lists
/// all subgroups found for the current search. Each entry has a `data-subgroupid`
/// attribute that can be combined with the search query to form a per-subgroup RSS URL.
///
/// ```html
/// <a class="subgroup-longname" onclick="AddFilter(this)" data-subgroupid="382">喵萌奶茶屋</a>
/// <a class="subgroup-longname" onclick="AddFilter(this)" data-subgroupid="370">LoliHouse</a>
/// ```
///
/// RSS URL format: `https://mikanani.me/RSS/Search?searchstr={encoded_query}&subgroupid={id}`
pub fn parse_source_detail(html: &str, query: &str) -> Result<DetailResponse, String> {
    let document = Html::parse_document(html);

    let subgroup_sel = Selector::parse("a.subgroup-longname[data-subgroupid]")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;

    let mut items: Vec<DetailItem> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for element in document.select(&subgroup_sel) {
        let subgroup_id = match element.value().attr("data-subgroupid") {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => continue,
        };

        if seen_ids.contains(&subgroup_id) {
            continue;
        }
        seen_ids.insert(subgroup_id.clone());

        let subgroup_name = element.text().collect::<String>().trim().to_string();
        if subgroup_name.is_empty() {
            continue;
        }

        let rss_url = format!(
            "https://mikanani.me/RSS/Search?searchstr={}&subgroupid={}",
            urlencoding::encode(query),
            subgroup_id
        );

        items.push(DetailItem { subgroup_name, rss_url });
    }

    // Always include a catch-all RSS entry for all subgroups
    items.push(DetailItem {
        subgroup_name: "全部".to_string(),
        rss_url: format!(
            "https://mikanani.me/RSS/Search?searchstr={}",
            urlencoding::encode(query)
        ),
    });

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

    // Real HTML structure from https://mikanani.me/Home/Bangumi/3822
    //
    // Variant A (id=202): subgroup name is a DIRECT TEXT NODE inside div.subgroup-text.
    // Variant B (id=1243, 370, 382): name is inside <a href="/Home/PublishGroup/..."> link.
    static REAL_BANGUMI_DETAIL_HTML: &str = r#"
        <html><body>
          <div class="subgroup-scroll-top-202"></div>
          <div class="subgroup-text" id="202">
            生肉/不明字幕
            <a href="/RSS/Bangumi?bangumiId=3822&amp;subgroupid=202" class="mikan-rss"
               data-placement="bottom" data-toggle="tooltip" data-original-title="RSS" target="_blank">
               <i class="fa fa-rss-square"></i>
            </a>
            <span class="subscribed" style="display:none;">已订阅</span>
            <a class="pull-right subgroup-subscribe js-subscribe_bangumi_page" data-bangumiid="3822" data-subtitlegroupid="202">订阅</a>
          </div>
          <div class="subgroup-scroll-top-1243"></div>
          <div class="subgroup-text" id="1243">
            <a href="/Home/PublishGroup/1015" target="_blank" style="color: #3bc0c3;">六四位元字幕组</a>
            <a href="/RSS/Bangumi?bangumiId=3822&amp;subgroupid=1243" class="mikan-rss"
               data-placement="bottom" data-toggle="tooltip" data-original-title="RSS" target="_blank">
               <i class="fa fa-rss-square"></i>
            </a>
            <span class="subscribed" style="display:none;">已订阅</span>
            <a class="pull-right subgroup-subscribe js-subscribe_bangumi_page" data-bangumiid="3822" data-subtitlegroupid="1243">订阅</a>
          </div>
          <div class="subgroup-scroll-top-370"></div>
          <div class="subgroup-text" id="370">
            <a href="/Home/PublishGroup/223" target="_blank" style="color: #3bc0c3;">LoliHouse</a>
            <a href="/RSS/Bangumi?bangumiId=3822&amp;subgroupid=370" class="mikan-rss"
               data-placement="bottom" data-toggle="tooltip" data-original-title="RSS" target="_blank">
               <i class="fa fa-rss-square"></i>
            </a>
            <span class="subscribed" style="display:none;">已订阅</span>
            <a class="pull-right subgroup-subscribe js-subscribe_bangumi_page" data-bangumiid="3822" data-subtitlegroupid="370">订阅</a>
          </div>
        </body></html>
    "#;

    // Real HTML for source detail: uses the leftbar a.subgroup-longname[data-subgroupid]
    // to identify subgroups (the same page as a search result).
    static REAL_SOURCE_DETAIL_HTML: &str = r#"
        <html><body>
          <div id="sk-container" class="container">
            <div class="pull-left leftbar-container">
              <div class="leftbar-nav">
                <div class="header">相关字幕组</div>
                <ul class="list-unstyled">
                  <li class="leftbar-item"><span>
                    <a class="subgroup-longname active" onclick="AddFilter(this)" data-subgroupid="">显示全部</a>
                  </span></li>
                  <li class="leftbar-item"><span>
                    <a class="subgroup-longname" onclick="AddFilter(this)" data-subgroupid="382">喵萌奶茶屋</a>
                  </span></li>
                  <li class="leftbar-item"><span>
                    <a class="subgroup-longname" onclick="AddFilter(this)" data-subgroupid="370">KITA</a>
                  </span></li>
                </ul>
              </div>
            </div>
          </div>
        </body></html>
    "#;

    #[test]
    fn test_parse_bangumi_detail_real_html_subgroup_names() {
        let result = parse_bangumi_detail(REAL_BANGUMI_DETAIL_HTML, "3822").unwrap();

        // 3 subgroups + 1 root "全部"
        assert_eq!(result.items.len(), 4);

        // Variant A: name is a direct text node
        assert_eq!(result.items[0].subgroup_name, "生肉/不明字幕");
        assert_eq!(
            result.items[0].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3822&subgroupid=202"
        );

        // Variant B: name is inside <a href="/Home/PublishGroup/...">
        assert_eq!(result.items[1].subgroup_name, "六四位元字幕组");
        assert_eq!(
            result.items[1].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3822&subgroupid=1243"
        );

        assert_eq!(result.items[2].subgroup_name, "LoliHouse");
        assert_eq!(
            result.items[2].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3822&subgroupid=370"
        );

        // Last item is always 全部
        assert_eq!(result.items[3].subgroup_name, "全部");
        assert_eq!(
            result.items[3].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3822"
        );
    }

    #[test]
    fn test_parse_bangumi_detail_no_subgroups_returns_root_only() {
        let html = "<html><body><p>no subgroups here</p></body></html>";
        let result = parse_bangumi_detail(html, "9999").unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].subgroup_name, "全部");
        assert_eq!(
            result.items[0].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=9999"
        );
    }

    #[test]
    fn test_parse_source_detail_real_html_uses_leftbar_subgroups() {
        let result = parse_source_detail(REAL_SOURCE_DETAIL_HTML, "金牌").unwrap();

        // leftbar has 2 subgroups (382=喵萌奶茶屋, 370=KITA) + 1 "全部" appended = 3 total
        assert_eq!(result.items.len(), 3, "Expected 2 subgroups + 全部");

        assert_eq!(result.items[0].subgroup_name, "喵萌奶茶屋");
        assert_eq!(
            result.items[0].rss_url,
            "https://mikanani.me/RSS/Search?searchstr=%E9%87%91%E7%89%8C&subgroupid=382"
        );

        assert_eq!(result.items[1].subgroup_name, "KITA");
        assert_eq!(
            result.items[1].rss_url,
            "https://mikanani.me/RSS/Search?searchstr=%E9%87%91%E7%89%8C&subgroupid=370"
        );

        // Last item is always 全部
        assert_eq!(result.items[2].subgroup_name, "全部");
        assert_eq!(
            result.items[2].rss_url,
            "https://mikanani.me/RSS/Search?searchstr=%E9%87%91%E7%89%8C"
        );
    }

    #[test]
    fn test_parse_source_detail_deduplicates_repeated_leftbar_subgroups() {
        // If the leftbar appears in both desktop and mobile sections, deduplicate by ID
        let html = r#"
            <html><body>
              <div id="sk-container">
                <a class="subgroup-longname" data-subgroupid="382">喵萌奶茶屋</a>
                <a class="subgroup-longname" data-subgroupid="">显示全部</a>
              </div>
              <div id="m-nav">
                <a class="subgroup-longname" data-subgroupid="382">喵萌奶茶屋</a>
              </div>
            </body></html>
        "#;
        let result = parse_source_detail(html, "金牌").unwrap();
        // Deduplicated: 1 subgroup + 全部
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].subgroup_name, "喵萌奶茶屋");
        assert_eq!(result.items[1].subgroup_name, "全部");
    }

    #[test]
    fn test_parse_source_detail_no_subgroups_returns_only_all() {
        let html = "<html><body><p>no results</p></body></html>";
        let result = parse_source_detail(html, "nonexistent").unwrap();
        // Only "全部"
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].subgroup_name, "全部");
    }
}
