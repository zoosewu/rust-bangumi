use fetcher_mikanani::RssParser;

#[tokio::main]
async fn main() {
    env_logger::init();

    let parser = RssParser::new();
    let rss_url = "https://mikanani.me/RSS/MyBangumi?token=iha85xwcvVAPOXWwmGnUtw%3d%3d";

    println!("🔍 正在測試 RSS 訂閱: {}\n", rss_url);

    match parser.parse_feed(rss_url).await {
        Ok(animes) => {
            println!("✅ 成功解析 RSS！\n");
            println!("📊 找到 {} 部動畫\n", animes.len());

            for (idx, anime) in animes.iter().enumerate() {
                println!("{}. 《{}》", idx + 1, anime.title);
                println!("   分類: {} | 年份: {}", anime.season, anime.year);
                println!("   下載連結數: {}", anime.links.len());

                for link in &anime.links {
                    println!("     • 第 {} 話 [{}] - {}",
                        link.episode_no,
                        link.subtitle_group,
                        &link.url[..link.url.len().min(60)]
                    );
                }
                println!();
            }
        },
        Err(e) => {
            println!("❌ 解析失敗: {}", e);
        }
    }
}
