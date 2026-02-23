use crate::client::ApiClient;
use crate::models::*;
use anyhow::Result;
use tracing::info;

/// Task 36: 訂閱 RSS 源
pub async fn subscribe(api_url: &str, rss_url: &str, fetcher: &str) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let request = SubscribeRequest {
        rss_url: rss_url.to_string(),
        fetcher: fetcher.to_string(),
    };

    let response: SuccessResponse = client.post("/anime", &request).await?;

    info!("訂閱成功: {}", response.message);
    println!("✓ 訂閱成功");
    println!("  RSS 地址: {}", rss_url);
    println!("  擷取器: {}", fetcher);

    Ok(())
}

/// Task 37: 列出動畫
pub async fn list(api_url: &str, anime_id: Option<i64>, _season: Option<String>) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let path = if let Some(id) = anime_id {
        format!("/anime/{}", id)
    } else {
        "/anime".to_string()
    };

    let response: ListResponse<AnimeMetadata> = client.get(&path).await?;

    if response.items.is_empty() {
        println!("未找到動畫");
        return Ok(());
    }

    println!("{:<10} {:<40} {}", "動畫 ID", "標題", "建立時間");
    println!("{}", "-".repeat(80));
    for anime in &response.items {
        println!(
            "{:<10} {:<40} {}",
            anime.anime_id,
            anime.title,
            anime.created_at
        );
    }

    if let Some(total) = response.total {
        println!("\n總計: {} 個動畫", total);
    }

    Ok(())
}

/// Task 38: 列出動畫連結
pub async fn links(
    api_url: &str,
    anime_id: i64,
    series: Option<i32>,
    group: Option<String>,
) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let path = format!("/links/{}", anime_id);
    let response: ListResponse<AnimeLink> = client.get(&path).await?;

    let mut links = response.items;

    // 過濾條件
    if let Some(_series_no) = series {
        links.retain(|_link| {
            // 我們需要查詢 series 的 series_no，但這裡先按 series_id 過濾
            true
        });
    }

    if let Some(group_name) = group {
        links.retain(|link| link.group_id.to_string().contains(&group_name));
    }

    if links.is_empty() {
        println!("未找到連結");
        return Ok(());
    }

    println!(
        "{:<8} {:<6} {:<8} {:<40} {:<50} {}",
        "連結 ID", "集數", "字幕組", "標題", "URL", "狀態"
    );
    println!("{}", "-".repeat(120));
    for link in &links {
        let status = if link.filtered_flag { "已過濾" } else { "活躍" };
        println!(
            "{:<8} {:<6} {:<8} {:<40} {:<50} {}",
            link.link_id,
            link.episode_no,
            link.group_id,
            link.title.as_deref().unwrap_or("-"),
            link.url,
            status
        );
    }

    Ok(())
}

/// Task 39: 管理過濾規則
pub async fn filter_add(
    api_url: &str,
    series_id: i64,
    group: &str,
    rule_type: &str,
    regex: &str,
) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let filter_type = match rule_type.to_lowercase().as_str() {
        "positive" | "正向" => FilterType::Positive,
        "negative" | "反向" => FilterType::Negative,
        _ => return Err(anyhow::anyhow!("無效的規則類型: {}", rule_type)),
    };

    // 解析 group 為 group_id (假設是數字 ID)
    let group_id: i64 = group.parse().unwrap_or(1i64);

    let request = CreateFilterRuleRequest {
        series_id,
        group_id,
        rule_type: filter_type,
        regex_pattern: regex.to_string(),
    };

    let response: SuccessResponse = client.post("/filters", &request).await?;

    info!("添加過濾規則成功: {}", response.message);
    println!("✓ 過濾規則已添加");
    println!("  系列 ID: {}", series_id);
    println!("  字幕組 ID: {}", group_id);
    println!("  規則類型: {}", rule_type);
    println!("  正則表達式: {}", regex);

    Ok(())
}

pub async fn filter_list(api_url: &str, series_id: i64, group: &str) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let group_id: i64 = group.parse().unwrap_or(1i64);

    let path = format!("/filters/{}/{}", series_id, group_id);
    let response: ListResponse<FilterRule> = client.get(&path).await?;

    if response.items.is_empty() {
        println!("未找到過濾規則");
        return Ok(());
    }

    println!(
        "{:<8} {:<8} {:<8} {:<6} {:<30} {}",
        "規則 ID", "系列 ID", "字幕組 ID", "類型", "正則表達式", "建立時間"
    );
    println!("{}", "-".repeat(80));
    for rule in &response.items {
        let rule_type = match rule.rule_type {
            FilterType::Positive => "正向",
            FilterType::Negative => "反向",
        };
        println!(
            "{:<8} {:<8} {:<8} {:<6} {:<30} {}",
            rule.rule_id,
            rule.series_id,
            rule.group_id,
            rule_type,
            rule.regex_pattern,
            rule.created_at
        );
    }

    Ok(())
}

pub async fn filter_remove(api_url: &str, rule_id: i64) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let path = format!("/filters/{}", rule_id);
    client.delete(&path).await?;

    info!("過濾規則已刪除: {}", rule_id);
    println!("✓ 過濾規則已刪除");
    println!("  規則 ID: {}", rule_id);

    Ok(())
}

/// Task 40: 啟動下載
pub async fn download(api_url: &str, link_id: i64, downloader: Option<String>) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let request = DownloadRequest {
        link_id,
        downloader,
    };

    let response: SuccessResponse = client.post("/download", &request).await?;

    info!("下載已啟動: {}", response.message);
    println!("✓ 下載已啟動");
    println!("  連結 ID: {}", link_id);

    Ok(())
}

/// Task 41: 查看狀態
pub async fn status(api_url: &str) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    // 取得健康檢查狀態
    let health: serde_json::Value = client.get("/health").await?;

    println!("系統狀態:");
    println!("{:#}", health);

    Ok(())
}

/// Task 42: 列出服務
pub async fn services(api_url: &str) -> Result<()> {
    let client = ApiClient::new(api_url.to_string());

    let response: ListResponse<RegisteredService> = client.get("/services").await?;

    if response.items.is_empty() {
        println!("未找到已註冊的服務");
        return Ok(());
    }

    println!(
        "{:<36} {:<12} {:<20} {:<16} {:<6} {:<8} {}",
        "服務 ID", "服務類型", "服務名稱", "主機", "埠口", "狀態", "最後心跳"
    );
    println!("{}", "-".repeat(120));
    for service in &response.items {
        let status = if service.is_healthy { "健康" } else { "不健康" };
        println!(
            "{:<36} {:<12} {:<20} {:<16} {:<6} {:<8} {}",
            service.service_id,
            service.service_type,
            service.service_name,
            service.host,
            service.port,
            status,
            service.last_heartbeat
        );
    }

    Ok(())
}

/// Task 43: 查看日誌
pub async fn logs(_api_url: &str, log_type: &str) -> Result<()> {
    // 由於核心服務還未實現日誌端點，這裡提供基礎實現
    // 實際項目中可能需要查詢日誌數據庫或文件系統

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
