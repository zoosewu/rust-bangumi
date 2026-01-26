use fetcher_mikanani::RssParser;

#[tokio::main]
async fn main() {
    let parser = RssParser::new();
    let rss_url = "https://mikanani.me/RSS/MyBangumi?token=iha85xwcvVAPOXWwmGnUtw%3d%3d";

    println!("\n🔍 正在測試 RSS 訂閱...\n");
    println!("URL: {}\n", rss_url);

    match parser.parse_feed(rss_url).await {
        Ok(animes) => {
            println!("✅ 成功解析 RSS！\n");
            println!("📊 找到 {} 部動畫\n", animes.len());

            if animes.is_empty() {
                println!("⚠️  未找到任何動畫，可能是標題格式不匹配");
                println!("\n測試 parser 的標題解析功能：\n");

                // Test various title formats
                let test_titles = vec![
                    "[SubGroup] Test Anime [01][1080p]",
                    "[LoliHouse] 黄金神威 最终章 / Golden Kamuy - 52 [WebRip 1080p HEVC-10bit AAC][无字幕]",
                    "六四位元字幕组★可以帮忙洗干净吗？Kirei ni Shite Moraemasu ka★03★1920x1080★AVC AAC MP4★繁体中文",
                    "[LoliHouse] 神八小妹不可怕 - 02 [1080p]",
                ];

                for title in test_titles {
                    print!("測試: \"{}\" ... ", title);
                    if let Some((anime, group, ep)) = parser.parse_title_public(title) {
                        println!("✅ 解析成功");
                        println!("  動畫: {}", anime);
                        println!("  字幕組: {}", group);
                        println!("  劇集: {}\n", ep);
                    } else {
                        println!("❌ 無法解析\n");
                    }
                }
            } else {
                for (idx, anime) in animes.iter().enumerate() {
                    println!("{}. 《{}》", idx + 1, anime.title);
                    println!("   分類: {} | 年份: {}", anime.season, anime.year);
                    println!("   下載連結數: {}", anime.links.len());

                    for link in anime.links.iter().take(3) {
                        let url_preview = if link.url.len() > 60 {
                            format!("{}...", &link.url[..60])
                        } else {
                            link.url.clone()
                        };
                        println!("     • 第 {} 話 [{}]", link.episode_no, link.subtitle_group);
                        println!("       URL: {}", url_preview);
                    }

                    if anime.links.len() > 3 {
                        println!("     ... 還有 {} 個連結", anime.links.len() - 3);
                    }
                    println!();
                }
            }
        },
        Err(e) => {
            println!("❌ 解析失敗: {}\n", e);
            println!("詳細錯誤信息：{:?}", e);
        }
    }
}
