use crate::client::ApiClient;
use crate::models::*;
use anyhow::Result;

/// 訂閱 RSS 源
pub async fn subscribe(api_url: &str, rss_url: &str, _fetcher: &str) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let request = CreateSubscriptionRequest {
        source_url: rss_url.to_string(),
        name: None,
        fetch_interval_minutes: None,
    };

    let response: MessageResponse = client.post("/api/subscriptions", &request).await?;

    println!("✓ 訂閱成功");
    println!("  RSS 地址: {}", rss_url);
    println!("  訊息: {}", response.message);

    Ok(())
}

/// 列出動畫
pub async fn list(api_url: &str, anime_id: Option<i64>, _season: Option<String>) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    if let Some(id) = anime_id {
        let response: AnimeResponse = client.get(&format!("/api/animes/{}", id)).await?;
        println!("{:<10} {:<40} {}", "動畫 ID", "標題", "建立時間");
        println!("{}", "-".repeat(70));
        println!("{:<10} {:<40} {}", response.anime_id, response.title, response.created_at);
    } else {
        let response: AnimesResponse = client.get("/api/animes").await?;
        if response.animes.is_empty() {
            println!("未找到動畫");
            return Ok(());
        }
        println!("{:<10} {:<40} {}", "動畫 ID", "標題", "建立時間");
        println!("{}", "-".repeat(70));
        for anime in &response.animes {
            println!("{:<10} {:<40} {}", anime.anime_id, anime.title, anime.created_at);
        }
        println!("\n總計: {} 個動畫", response.animes.len());
    }

    Ok(())
}

/// 列出動畫連結
pub async fn links(
    api_url: &str,
    anime_id: i64,
    _series: Option<i32>,
    _group: Option<String>,
) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let response: LinksResponse = client.get(&format!("/api/animes/{}/links", anime_id)).await?;

    if response.links.is_empty() {
        println!("未找到連結");
        return Ok(());
    }

    println!(
        "{:<8} {:<6} {:<12} {:<40} {:<50} {}",
        "連結 ID", "集數", "字幕組", "標題", "URL", "狀態"
    );
    println!("{}", "-".repeat(120));
    for link in &response.links {
        let status = if link.filtered_flag { "已過濾" } else { "活躍" };
        println!(
            "{:<8} {:<6} {:<12} {:<40} {:<50} {}",
            link.link_id,
            link.episode_no,
            link.group_name.as_deref().unwrap_or("-"),
            link.title.as_deref().unwrap_or("-"),
            link.url,
            status
        );
    }

    Ok(())
}

/// 添加過濾規則
pub async fn filter_add(
    api_url: &str,
    _series_id: i64,
    _group: &str,
    rule_type: &str,
    regex: &str,
) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let is_positive = match rule_type.to_lowercase().as_str() {
        "positive" | "正向" => true,
        "negative" | "反向" => false,
        _ => return Err(anyhow::anyhow!("無效的規則類型: {}", rule_type)),
    };

    let request = CreateFilterRuleRequest {
        target_type: "global".to_string(),
        target_id: None,
        rule_order: 0,
        is_positive,
        regex_pattern: regex.to_string(),
    };

    let response: FilterRuleResponse = client.post("/api/filters", &request).await?;

    println!("✓ 過濾規則已添加");
    println!("  規則 ID: {}", response.rule_id);
    println!("  正則表達式: {}", response.regex_pattern);

    Ok(())
}

/// 列出過濾規則
pub async fn filter_list(api_url: &str, _series_id: i64, _group: &str) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let response: FiltersResponse = client.get("/api/filters").await?;

    if response.rules.is_empty() {
        println!("未找到過濾規則");
        return Ok(());
    }

    println!(
        "{:<8} {:<12} {:<8} {:<6} {:<30} {}",
        "規則 ID", "目標類型", "目標 ID", "類型", "正則表達式", "建立時間"
    );
    println!("{}", "-".repeat(80));
    for rule in &response.rules {
        let rule_type = if rule.is_positive { "正向" } else { "反向" };
        println!(
            "{:<8} {:<12} {:<8} {:<6} {:<30} {}",
            rule.rule_id,
            rule.target_type,
            rule.target_id.map(|i| i.to_string()).unwrap_or_else(|| "-".to_string()),
            rule_type,
            rule.regex_pattern,
            rule.created_at
        );
    }

    Ok(())
}

/// 刪除過濾規則
pub async fn filter_remove(api_url: &str, rule_id: i64) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    client.delete(&format!("/api/filters/{}", rule_id)).await?;

    println!("✓ 過濾規則已刪除");
    println!("  規則 ID: {}", rule_id);

    Ok(())
}

/// 啟動下載
pub async fn download(api_url: &str, link_id: i64, _downloader: Option<String>) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let response: MessageResponse = client
        .post_no_body(&format!("/api/links/{}/download", link_id))
        .await?;

    println!("✓ 下載已啟動");
    println!("  連結 ID: {}", link_id);
    println!("  訊息: {}", response.message);

    Ok(())
}

/// 查看狀態
pub async fn status(api_url: &str) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let health: serde_json::Value = client.get("/health").await?;

    println!("系統狀態:");
    println!("{:#}", health);

    Ok(())
}

/// 列出服務
pub async fn services(api_url: &str) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let stats: DashboardStats = client.get("/api/dashboard").await?;

    if stats.services.is_empty() {
        println!("未找到已註冊的服務");
        return Ok(());
    }

    println!("{:<30} {:<16} {}", "服務名稱", "類型", "狀態");
    println!("{}", "-".repeat(60));
    for service in &stats.services {
        let status = if service.is_healthy { "健康" } else { "不健康" };
        println!("{:<30} {:<16} {}", service.name, service.module_type, status);
    }

    Ok(())
}

/// 查看日誌
pub async fn logs(_api_url: &str, log_type: &str) -> Result<()> {
    println!("日誌查詢: {}", log_type);
    println!("注意: 日誌功能需要在核心服務中實現日誌端點");

    Ok(())
}

/// 設定 qBittorrent downloader 帳密
pub async fn qb_login(downloader_url: &str, user: &str, password: &str) -> Result<()> {
    let client = ApiClient::new(downloader_url.to_string());

    #[derive(serde::Serialize)]
    struct Req<'a> {
        username: &'a str,
        password: &'a str,
    }

    let _: serde_json::Value = client
        .post("/config/credentials", &Req { username: user, password })
        .await?;

    println!("✓ qBittorrent 帳密已設定");
    println!("  帳號: {}", user);
    println!("  Downloader URL: {}", downloader_url);
    Ok(())
}
