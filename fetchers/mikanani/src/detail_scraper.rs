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

/// Parse bangumi detail page for per-subgroup RSS links.
///
/// Real page structure from https://mikanani.me/Home/Bangumi/{id}:
/// ```html
/// <div class="subgroup-text" id="202">
///   生肉/不明字幕
///   <a href="/RSS/Bangumi?bangumiId=3822&subgroupid=202" class="mikan-rss">...</a>
///   <span class="subscribed" style="display:none;">已订阅</span>
///   <a class="subgroup-subscribe ...">...</a>
/// </div>
/// ```
///
/// The subgroup name is the first non-empty text node of `div.subgroup-text`.
/// The subgroup ID is the `id` attribute of `div.subgroup-text`.
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

        // The subgroup name is the first non-empty direct text node of div.subgroup-text
        // (subsequent nodes are the RSS icon <a>, <span class="subscribed">, etc.)
        let subgroup_name = element
            .children()
            .filter_map(|child| child.value().as_text())
            .map(|t| t.trim())
            .find(|t| !t.is_empty())
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

/// Parse a search page re-fetched with the source searchstr, grouping results by subgroup.
///
/// Episode row structure (same as main search page):
/// ```html
/// <tr class="js-search-results-row">
///   <td><input class="js-episode-select" data-magnet="..." /></td>
///   <td><a href="/Home/Episode/{hash}" class="magnet-link-wrap">[SubGroup] Title_Suffix</a></td>
/// </tr>
/// ```
///
/// - Subgroup name: extracted from `[...]` brackets at the start of the title
/// - RSS URL: `searchstr` = title truncated at last `_`; deduplicated by subgroup name
pub fn parse_source_detail(html: &str, _original_searchstr: &str) -> Result<DetailResponse, String> {
    let document = Html::parse_document(html);

    let episode_sel = Selector::parse("tr.js-search-results-row td a.magnet-link-wrap")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;

    let mut items: Vec<DetailItem> = Vec::new();
    let mut seen_subgroups: std::collections::HashSet<String> = std::collections::HashSet::new();

    for element in document.select(&episode_sel) {
        let href = element.value().attr("href").unwrap_or("");
        if !href.contains("/Home/Episode/") {
            continue;
        }

        let title = element.text().collect::<String>().trim().to_string();
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

        // Deduplicate by subgroup name (one RSS entry per subgroup)
        if seen_subgroups.contains(&subgroup_name) {
            continue;
        }
        seen_subgroups.insert(subgroup_name.clone());

        // Compute RSS searchstr: truncate at last '_' (e.g. "_Ciallo" suffix)
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

    // Real HTML structure from https://mikanani.me/Home/Bangumi/3822
    // Each subgroup is in: div.subgroup-text[id=subgroupid] > TEXT_NODE + a.mikan-rss
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
          </div>
          <div class="subgroup-scroll-top-1243"></div>
          <div class="subgroup-text" id="1243">
            喵萌奶茶屋&LoliHouse
            <a href="/RSS/Bangumi?bangumiId=3822&amp;subgroupid=1243" class="mikan-rss"
               data-placement="bottom" data-toggle="tooltip" data-original-title="RSS" target="_blank">
               <i class="fa fa-rss-square"></i>
            </a>
            <span class="subscribed" style="display:none;">已订阅</span>
          </div>
          <div class="subgroup-scroll-top-370"></div>
          <div class="subgroup-text" id="370">
            KITA
            <a href="/RSS/Bangumi?bangumiId=3822&amp;subgroupid=370" class="mikan-rss"
               data-placement="bottom" data-toggle="tooltip" data-original-title="RSS" target="_blank">
               <i class="fa fa-rss-square"></i>
            </a>
            <span class="subscribed" style="display:none;">已订阅</span>
          </div>
        </body></html>
    "#;

    // Real HTML for source search results (same episode-table structure as main search)
    static REAL_SOURCE_DETAIL_HTML: &str = r#"
        <html><body>
          <div class="episode-table">
            <table class="table table-striped tbl-border fadeIn">
              <tbody>
                <tr class="js-search-results-row" data-itemindex="0">
                  <td><input type="checkbox" class="js-episode-select"
                    data-magnet="magnet:?xt=urn:btih:a699e0962e20c6561bd6728386a0d3f2cd6edc5a"
                    aria-label="选择此行" /></td>
                  <td>
                    <a href="/Home/Episode/a699e0962e20c6561bd6728386a0d3f2cd6edc5a" target="_blank"
                       class="magnet-link-wrap">[KITA]（双语人工翻译）金牌得主19_Ciallo</a>
                  </td>
                </tr>
                <tr class="js-search-results-row" data-itemindex="1">
                  <td><input type="checkbox" class="js-episode-select"
                    data-magnet="magnet:?xt=urn:btih:b699e0962e20c6561bd6728386a0d3f2cd6edc5b"
                    aria-label="选择此行" /></td>
                  <td>
                    <a href="/Home/Episode/b699e0962e20c6561bd6728386a0d3f2cd6edc5b" target="_blank"
                       class="magnet-link-wrap">[KITA]（双语人工翻译）金牌得主18_Ciallo</a>
                  </td>
                </tr>
                <tr class="js-search-results-row" data-itemindex="2">
                  <td><input type="checkbox" class="js-episode-select"
                    data-magnet="magnet:?xt=urn:btih:14fc051e0ff8d17a27a9ab6077b6fc25c9ff628a"
                    aria-label="选择此行" /></td>
                  <td>
                    <a href="/Home/Episode/14fc051e0ff8d17a27a9ab6077b6fc25c9ff628a" target="_blank"
                       class="magnet-link-wrap">[喵萌奶茶屋&amp;LoliHouse] 金牌得主 / Medalist - 18 [WebRip 1080p HEVC-10bit AAC][简繁日内封字幕]</a>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </body></html>
    "#;

    #[test]
    fn test_parse_bangumi_detail_real_html_subgroup_names() {
        let result = parse_bangumi_detail(REAL_BANGUMI_DETAIL_HTML, "3822").unwrap();

        // 3 subgroups + 1 root "全部"
        assert_eq!(result.items.len(), 4);

        assert_eq!(result.items[0].subgroup_name, "生肉/不明字幕");
        assert_eq!(
            result.items[0].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3822&subgroupid=202"
        );

        assert_eq!(result.items[1].subgroup_name, "喵萌奶茶屋&LoliHouse");
        assert_eq!(
            result.items[1].rss_url,
            "https://mikanani.me/RSS/Bangumi?bangumiId=3822&subgroupid=1243"
        );

        assert_eq!(result.items[2].subgroup_name, "KITA");
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
    fn test_parse_source_detail_real_html_deduplicates_by_subgroup() {
        let result = parse_source_detail(REAL_SOURCE_DETAIL_HTML, "[KITA]金牌").unwrap();

        // KITA episodes 19 and 18 → same subgroup, dedup → 1 KITA entry
        // 喵萌奶茶屋 → different subgroup → 1 entry
        assert_eq!(result.items.len(), 2, "Expected KITA (deduped) + 喵萌奶茶屋");

        assert_eq!(result.items[0].subgroup_name, "KITA");
        // RSS URL uses title of FIRST KITA episode, truncated at last '_'
        // "[KITA]（双语人工翻译）金牌得主19_Ciallo" → "[KITA]（双语人工翻译）金牌得主19"
        assert!(
            result.items[0].rss_url.contains("searchstr="),
            "RSS URL should contain searchstr param"
        );
        assert!(
            result.items[0].rss_url.contains("%E9%87%91%E7%89%8C"),  // 金牌
            "RSS URL should contain encoded title"
        );

        assert_eq!(result.items[1].subgroup_name, "喵萌奶茶屋&LoliHouse");
    }

    #[test]
    fn test_parse_source_detail_no_underscore_uses_full_title_as_searchstr() {
        let html = r#"
            <html><body>
              <div class="episode-table">
                <table><tbody>
                  <tr class="js-search-results-row">
                    <td><input class="js-episode-select" data-magnet="magnet:test" /></td>
                    <td><a href="/Home/Episode/abc" class="magnet-link-wrap">[SubB]金牌得主 S02E19 1080p</a></td>
                  </tr>
                </tbody></table>
              </div>
            </body></html>
        "#;
        let result = parse_source_detail(html, "[SubB]金牌").unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].subgroup_name, "SubB");
        assert!(result.items[0].rss_url.contains("searchstr="));
        // Full title used since no '_'
        assert!(result.items[0].rss_url.contains("SubB"));
    }

    #[test]
    fn test_parse_source_detail_title_without_brackets() {
        let html = r#"
            <html><body>
              <div class="episode-table">
                <table><tbody>
                  <tr class="js-search-results-row">
                    <td><input class="js-episode-select" data-magnet="magnet:test" /></td>
                    <td><a href="/Home/Episode/abc" class="magnet-link-wrap">金牌得主19_noBrackets</a></td>
                  </tr>
                </tbody></table>
              </div>
            </body></html>
        "#;
        let result = parse_source_detail(html, "金牌得主19").unwrap();
        assert_eq!(result.items.len(), 1);
        // No brackets: full title is used as subgroup_name
        assert_eq!(result.items[0].subgroup_name, "金牌得主19_noBrackets");
    }
}
