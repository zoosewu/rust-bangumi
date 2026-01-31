//! 深入診斷標題解析問題
//!
//! 執行方式: cargo run --example debug_parse_title

use regex::Regex;

fn main() {
    println!("=== 標題解析診斷 ===\n");

    // 目前的 regex pattern
    let episode_regex = Regex::new(r"(?:\[|第|EP)(\d+)(?:\]|話|集)?").unwrap();

    let test_titles = vec![
        "[LoliHouse] Yuusha Party ni Kawaii Ko ga Ita node, Kokuhaku shitemita. / 身为魔族的我想向勇者小队的可爱女孩告白。 - 04 [WebRip 1080p HEVC-10bit AAC][简繁内封字幕]",
        "[LoliHouse] 神八小妹不可怕 / カヤちゃんはコワくない / Kaya-chan wa Kowakunai - 03 [WebRip 1080p HEVC-10bit AAC][简繁内封字幕]",
        "六四位元字幕组★可以帮忙洗干净吗？Kirei ni Shite Moraemasu ka★04★1920x1080★AVC AAC MP4★繁体中文",
        "[LoliHouse] 黄金神威 最终章 / Golden Kamuy - 53 [WebRip 1080p HEVC-10bit AAC][无字幕]",
        "[豌豆字幕组&风之圣殿字幕组&LoliHouse] 地狱乐 / Jigokuraku - 16 [WebRip 1080p HEVC-10bit AAC][简繁外挂字幕]",
    ];

    for (i, title) in test_titles.iter().enumerate() {
        println!("========================================");
        println!("測試 #{}", i + 1);
        println!("========================================");
        println!("原始標題: {}", title);
        println!();

        // Step 1: 提取字幕組
        let subtitle_group = title
            .split('[')
            .nth(1)
            .and_then(|s| s.split(']').next());

        println!("Step 1 - 字幕組提取:");
        match subtitle_group {
            Some(g) => println!("  ✅ 找到: '{}'", g),
            None => println!("  ❌ 找不到字幕組"),
        }

        // Step 2: episode regex 匹配
        println!("\nStep 2 - 集數 regex 匹配:");
        println!("  Pattern: {:?}", episode_regex.as_str());

        if let Some(captures) = episode_regex.captures(title) {
            println!("  ✅ 找到匹配:");
            println!("     完整匹配: '{}'", captures.get(0).unwrap().as_str());
            println!("     匹配位置: {}", captures.get(0).unwrap().start());
            if let Some(ep) = captures.get(1) {
                println!("     捕獲的集數: '{}'", ep.as_str());
            }
        } else {
            println!("  ❌ 沒有匹配");
        }

        // Step 3: 分析真實的集數格式
        println!("\nStep 3 - 分析真實格式:");

        // 檢查 " - XX " 格式
        if let Some(pos) = title.find(" - ") {
            let after_dash = &title[pos + 3..];
            let ep_str: String = after_dash.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !ep_str.is_empty() {
                println!("  發現 ' - XX ' 格式: 集數 = {}", ep_str);
            }
        }

        // 檢查 ★XX★ 格式
        let star_parts: Vec<&str> = title.split('★').collect();
        if star_parts.len() > 2 {
            println!("  發現 '★' 分隔格式: {:?}", star_parts);
            for part in &star_parts {
                if part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty() {
                    println!("    可能的集數: {}", part);
                }
            }
        }

        // Step 4: 第一個 ] 後的內容
        if let Some(first_close) = title.find(']') {
            println!("\nStep 4 - 第一個 ] 後的內容:");
            println!("  '{}'", &title[first_close + 1..]);
        }

        println!();
    }

    println!("========================================");
    println!("           診斷建議");
    println!("========================================\n");

    println!("目前的 regex 問題：");
    println!("  Pattern: r\"(?:\\[|第|EP)(\\d+)(?:\\]|話|集)?\"");
    println!("  這個 pattern 只匹配:");
    println!("    - [XX] 格式");
    println!("    - 第XX話 格式");
    println!("    - EPXX 格式");
    println!();
    println!("但 mikanani 真實資料使用:");
    println!("    - ' - XX ' 格式（dash + 空格 + 數字）");
    println!("    - '★XX★' 格式（星號分隔）");
    println!();
    println!("建議的新 regex:");
    println!("  r\"(?:\\[|第|EP|\\s-\\s)(\\d+)(?:\\]|話|集|\\s|\\[)?\"");

    // 測試新的 regex
    println!("\n========================================");
    println!("       測試改進後的 regex");
    println!("========================================\n");

    let new_regex = Regex::new(r"(?:\s-\s|第|EP|\[)(\d{1,3})(?:\]|話|集|\s|\[|$)").unwrap();

    for (i, title) in test_titles.iter().enumerate() {
        print!("測試 #{}: ", i + 1);

        if let Some(captures) = new_regex.captures(title) {
            if let Some(ep) = captures.get(1) {
                println!("✅ 集數 = {}", ep.as_str());
            }
        } else {
            println!("❌ 仍然失敗");
        }
    }
}
