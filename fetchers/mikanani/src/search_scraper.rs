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
///
/// Real page structure (two sections):
///
/// 1. Bangumi cards: `ul.list-inline.an-ul > li > a`
///    - href: "/Home/Bangumi/{id}"
///    - thumbnail: `span.b-lazy[data-src]`
///    - title: `div.an-text` (title attribute preferred, then text content)
///
/// 2. Episode table rows: `tr.js-search-results-row td a.magnet-link-wrap`
///    - href: "/Home/Episode/{hash}"
///    - title: text content of the anchor
///    - detail_key: "source:{title_truncated_at_last_underscore}"
pub fn parse_search_results(html: &str) -> Result<Vec<SearchResult>, String> {
    let document = Html::parse_document(html);
    let mut results = Vec::new();

    // --- Part 1: Bangumi cards ---
    let bangumi_sel = Selector::parse("ul.an-ul li a")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;
    let title_sel = Selector::parse("div.an-text")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;
    let img_sel = Selector::parse("span.b-lazy")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;

    for element in document.select(&bangumi_sel) {
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
            .map(|el| {
                // Use title attribute for untruncated text; fallback to text content
                el.value()
                    .attr("title")
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| el.text().collect::<String>().trim().to_string())
            })
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        // Image is in span.b-lazy[data-src] (lazy-loaded, not a standard img tag)
        let thumbnail_url = element
            .select(&img_sel)
            .next()
            .and_then(|el| el.value().attr("data-src"))
            .map(|src| {
                if src.starts_with("http") {
                    src.to_string()
                } else {
                    format!("https://mikanani.me{}", src)
                }
            });

        results.push(SearchResult {
            title,
            thumbnail_url,
            detail_key: format!("bangumi:{}", bangumi_id),
        });
    }

    // --- Part 2: Episode table rows ---
    let episode_sel = Selector::parse("tr.js-search-results-row td a.magnet-link-wrap")
        .map_err(|e| format!("Invalid selector: {:?}", e))?;

    for element in document.select(&episode_sel) {
        let href = match element.value().attr("href") {
            Some(h) => h,
            None => continue,
        };
        if !href.contains("/Home/Episode/") {
            continue;
        }

        let title = element.text().collect::<String>().trim().to_string();
        if title.is_empty() {
            continue;
        }

        // Truncate title at last '_' to produce a stable searchstr for the RSS URL
        let searchstr = match title.rfind('_') {
            Some(idx) => title[..idx].to_string(),
            None => title.clone(),
        };

        results.push(SearchResult {
            title,
            thumbnail_url: None,
            detail_key: format!("source:{}", searchstr),
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

    // Real HTML structure from https://mikanani.me/Home/Search?searchstr=金牌
    // Bangumi section: ul.list-inline.an-ul > li > a
    // Episode section: div.episode-table > table > tr.js-search-results-row
    static REAL_SEARCH_HTML: &str = r#"
        <html><body>
          <ul class="list-inline an-ul" style="margin-top:20px;">
            <li>
              <a href="/Home/Bangumi/3519" target="_blank">
                <span data-src="/images/Bangumi/202501/27eeaf1a.jpg?width=400&amp;height=400&amp;format=webp" class="b-lazy"></span>
                <div class="an-info">
                  <div class="an-info-group">
                    <div class="an-text" title="金牌得主" style="white-space:nowrap; width:170px; overflow:hidden; text-overflow:ellipsis;line-height: 40px;">金牌得主</div>
                  </div>
                </div>
              </a>
            </li>
            <li>
              <a href="/Home/Bangumi/3822" target="_blank">
                <span data-src="/images/Bangumi/202601/cbad1678.jpg?width=400&amp;height=400&amp;format=webp" class="b-lazy"></span>
                <div class="an-info">
                  <div class="an-info-group">
                    <div class="an-text" title="金牌得主 第二季" style="white-space:nowrap; width:170px; overflow:hidden; text-overflow:ellipsis;line-height: 40px;">金牌得主 第二季</div>
                  </div>
                </div>
              </a>
            </li>
          </ul>
          <div class="episode-table">
            <table class="table table-striped tbl-border fadeIn">
              <tbody>
                <tr class="js-search-results-row" data-itemindex="0">
                  <td>
                    <input type="checkbox" class="js-episode-select"
                      data-magnet="magnet:?xt=urn:btih:a699e0962e20c6561bd6728386a0d3f2cd6edc5a&amp;tr=http%3a%2f%2ft.nyaatracker.com%2fannounce"
                      aria-label="选择此行" />
                  </td>
                  <td>
                    <a href="/Home/Episode/a699e0962e20c6561bd6728386a0d3f2cd6edc5a" target="_blank"
                        class="magnet-link-wrap">[KITA]（双语人工翻译）&#x200B;金牌得主19，无法下载b站搜索KITA_Ciallo</a>
                  </td>
                  <td>232.26 MB</td>
                  <td>2026/03/01 12:40</td>
                </tr>
                <tr class="js-search-results-row" data-itemindex="1">
                  <td>
                    <input type="checkbox" class="js-episode-select"
                      data-magnet="magnet:?xt=urn:btih:2f2be30566da45ac7fb9849c2386fa787d6ff2d4&amp;tr=http%3a%2f%2ft.nyaatracker.com%2fannounce"
                      aria-label="选择此行" />
                  </td>
                  <td>
                    <a href="/Home/Episode/2f2be30566da45ac7fb9849c2386fa787d6ff2d4" target="_blank"
                        class="magnet-link-wrap">六四位元字幕组★金牌得主 第二季 Medalist 2★19★1920x1080★AVC AAC MP4★繁体中文</a>
                  </td>
                  <td>1.1 GB</td>
                  <td>2026/03/01 10:00</td>
                </tr>
                <tr class="js-search-results-row" data-itemindex="2">
                  <td>
                    <input type="checkbox" class="js-episode-select"
                      data-magnet="magnet:?xt=urn:btih:14fc051e0ff8d17a27a9ab6077b6fc25c9ff628a"
                      aria-label="选择此行" />
                  </td>
                  <td>
                    <a href="/Home/Episode/14fc051e0ff8d17a27a9ab6077b6fc25c9ff628a" target="_blank"
                        class="magnet-link-wrap">[喵萌奶茶屋&amp;LoliHouse] 金牌得主 / Medalist - 18 [WebRip 1080p HEVC-10bit AAC][简繁日内封字幕]</a>
                  </td>
                  <td>800 MB</td>
                  <td>2026/02/28 20:00</td>
                </tr>
              </tbody>
            </table>
          </div>
        </body></html>
    "#;

    #[test]
    fn test_parse_empty_html() {
        let result = parse_search_results("<html><body></body></html>").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_real_search_html_bangumi_cards() {
        let results = parse_search_results(REAL_SEARCH_HTML).unwrap();

        // Should have 2 bangumi + 3 episodes = 5 total
        assert_eq!(results.len(), 5, "Expected 2 bangumi + 3 episodes");

        // First two are bangumi
        assert_eq!(results[0].title, "金牌得主");
        assert_eq!(results[0].detail_key, "bangumi:3519");
        assert_eq!(
            results[0].thumbnail_url,
            Some("https://mikanani.me/images/Bangumi/202501/27eeaf1a.jpg?width=400&height=400&format=webp".to_string())
        );

        assert_eq!(results[1].title, "金牌得主 第二季");
        assert_eq!(results[1].detail_key, "bangumi:3822");
        assert_eq!(
            results[1].thumbnail_url,
            Some("https://mikanani.me/images/Bangumi/202601/cbad1678.jpg?width=400&height=400&format=webp".to_string())
        );
    }

    #[test]
    fn test_parse_real_search_html_episode_rows() {
        let results = parse_search_results(REAL_SEARCH_HTML).unwrap();

        // Episodes start at index 2
        let episodes = &results[2..];
        assert_eq!(episodes.len(), 3);

        // KITA episode: title has '_Ciallo' suffix, truncated at last '_'
        assert_eq!(
            episodes[0].title,
            "[KITA]（双语人工翻译）\u{200b}金牌得主19，无法下载b站搜索KITA_Ciallo"
        );
        assert_eq!(
            episodes[0].detail_key,
            "source:[KITA]（双语人工翻译）\u{200b}金牌得主19，无法下载b站搜索KITA"
        );
        assert_eq!(episodes[0].thumbnail_url, None);

        // 六四位元 episode: no '_', full title used as searchstr
        assert_eq!(
            episodes[1].title,
            "六四位元字幕组★金牌得主 第二季 Medalist 2★19★1920x1080★AVC AAC MP4★繁体中文"
        );
        assert_eq!(
            episodes[1].detail_key,
            "source:六四位元字幕组★金牌得主 第二季 Medalist 2★19★1920x1080★AVC AAC MP4★繁体中文"
        );

        // 喵萌奶茶屋 episode: no '_'
        assert!(episodes[2].detail_key.starts_with("source:"));
        assert!(episodes[2].title.contains("喵萌奶茶屋"));
    }

    #[test]
    fn test_parse_bangumi_with_data_src_thumbnail() {
        // Verify that span.b-lazy[data-src] is used for images (not img[src])
        let html = r#"
            <html><body>
              <ul class="list-inline an-ul">
                <li>
                  <a href="/Home/Bangumi/9999" target="_blank">
                    <span data-src="/images/Bangumi/test/cover.jpg" class="b-lazy"></span>
                    <div class="an-info">
                      <div class="an-info-group">
                        <div class="an-text" title="テストアニメ">テストアニメ</div>
                      </div>
                    </div>
                  </a>
                </li>
              </ul>
            </body></html>
        "#;
        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].detail_key, "bangumi:9999");
        assert_eq!(
            results[0].thumbnail_url,
            Some("https://mikanani.me/images/Bangumi/test/cover.jpg".to_string())
        );
    }

    #[test]
    fn test_parse_episode_underscore_truncation() {
        // title = "SubGroup_Show19_Ciallo" → searchstr = "SubGroup_Show19"
        let html = r#"
            <html><body>
              <div class="episode-table">
                <table>
                  <tbody>
                    <tr class="js-search-results-row">
                      <td><input class="js-episode-select" data-magnet="magnet:test" /></td>
                      <td><a href="/Home/Episode/abc123" class="magnet-link-wrap">SubGroup_Show19_Ciallo</a></td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </body></html>
        "#;
        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "SubGroup_Show19_Ciallo");
        assert_eq!(results[0].detail_key, "source:SubGroup_Show19");
    }

    #[test]
    fn test_parse_episode_no_underscore_uses_full_title() {
        let html = r#"
            <html><body>
              <div class="episode-table">
                <table>
                  <tbody>
                    <tr class="js-search-results-row">
                      <td><input class="js-episode-select" data-magnet="magnet:test" /></td>
                      <td><a href="/Home/Episode/xyz789" class="magnet-link-wrap">[喵萌奶茶屋] 金牌得主 S02E19 1080p</a></td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </body></html>
        "#;
        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].detail_key, "source:[喵萌奶茶屋] 金牌得主 S02E19 1080p");
    }

    #[test]
    fn test_parse_ignores_nav_links_outside_an_ul() {
        // Navigation links like /Home/MyBangumi should NOT be parsed
        let html = r#"
            <html><body>
              <a href="/Home/MyBangumi">我的追番</a>
              <a href="/Home/Classic">经典</a>
              <ul class="list-inline an-ul">
                <li>
                  <a href="/Home/Bangumi/3519" target="_blank">
                    <span data-src="/img/cover.jpg" class="b-lazy"></span>
                    <div class="an-info"><div class="an-info-group">
                      <div class="an-text" title="金牌得主">金牌得主</div>
                    </div></div>
                  </a>
                </li>
              </ul>
            </body></html>
        "#;
        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].detail_key, "bangumi:3519");
    }
}
