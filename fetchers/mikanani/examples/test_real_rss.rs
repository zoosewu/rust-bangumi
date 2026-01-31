//! 測試真實 RSS 資料的解析能力
//!
//! 執行方式: cargo run --example test_real_rss

use fetcher_mikanani::RssParser;

const TEST_RSS_URL: &str = "https://mikanani.me/RSS/MyBangumi?token=iha85xwcvVAPOXWwmGnUtw%3d%3d";

#[tokio::main]
async fn main() {
    println!("=== Fetcher RSS 解析測試 ===\n");
    println!("RSS URL: {}\n", TEST_RSS_URL);

    let parser = RssParser::new();

    // 測試 parse_feed
    println!("--- 測試 parse_feed() ---\n");

    match parser.parse_feed(TEST_RSS_URL).await {
        Ok(animes) => {
            println!("✅ 解析成功！\n");
            println!("總共解析出 {} 個動畫\n", animes.len());

            let total_links: usize = animes.iter().map(|a| a.links.len()).sum();
            println!("總共 {} 個連結\n", total_links);

            println!("========================================");
            println!("           詳細解析結果");
            println!("========================================\n");

            for (i, anime) in animes.iter().enumerate() {
                println!("【動畫 #{}】", i + 1);
                println!("  標題: {}", anime.title);
                println!("  描述: {}", if anime.description.is_empty() { "(空)" } else { &anime.description });
                println!("  季度: {}", anime.season);
                println!("  年份: {}", anime.year);
                println!("  系列號: {}", anime.series_no);
                println!("  連結數: {}", anime.links.len());
                println!();

                for (j, link) in anime.links.iter().enumerate() {
                    println!("    【連結 #{}.{}】", i + 1, j + 1);
                    println!("      集數: {}", link.episode_no);
                    println!("      字幕組: {}", link.subtitle_group);
                    println!("      標題: {}", link.title);
                    println!("      URL: {}", link.url);
                    println!("      Hash: {}", &link.source_hash[..16]);
                    println!();
                }
            }
        }
        Err(e) => {
            println!("❌ 解析失敗: {}", e);
        }
    }

    // 測試標題解析
    println!("\n========================================");
    println!("        標題解析測試（parse_title）");
    println!("========================================\n");

    let test_titles = vec![
        "[LoliHouse] Yuusha Party ni Kawaii Ko ga Ita node, Kokuhaku shitemita. / 身为魔族的我想向勇者小队的可爱女孩告白。 - 04 [WebRip 1080p HEVC-10bit AAC][简繁内封字幕]",
        "[LoliHouse] 神八小妹不可怕 / カヤちゃんはコワくない / Kaya-chan wa Kowakunai - 03 [WebRip 1080p HEVC-10bit AAC][简繁内封字幕]",
        "六四位元字幕组★可以帮忙洗干净吗？Kirei ni Shite Moraemasu ka★04★1920x1080★AVC AAC MP4★繁体中文",
        "[LoliHouse] 黄金神威 最终章 / Golden Kamuy - 53 [WebRip 1080p HEVC-10bit AAC][无字幕]",
        "[豌豆字幕组&风之圣殿字幕组&LoliHouse] 地狱乐 / Jigokuraku - 16 [WebRip 1080p HEVC-10bit AAC][简繁外挂字幕]",
    ];

    for (i, title) in test_titles.iter().enumerate() {
        println!("測試 #{}", i + 1);
        println!("  原始標題: {}", title);

        match parser.parse_title_public(title) {
            Some((anime_title, subtitle_group, episode_no)) => {
                println!("  ✅ 解析成功:");
                println!("     動畫標題: {}", anime_title);
                println!("     字幕組: {}", subtitle_group);
                println!("     集數: {}", episode_no);
            }
            None => {
                println!("  ❌ 解析失敗");
            }
        }
        println!();
    }

    println!("========================================");
    println!("              測試完成");
    println!("========================================");
}
