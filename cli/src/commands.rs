use anyhow::Result;

pub async fn subscribe(api_url: &str, rss_url: &str, fetcher: &str) -> Result<()> {
    println!("訂閱 RSS: {} (使用 {})", rss_url, fetcher);
    Ok(())
}

pub async fn list(api_url: &str, anime_id: Option<i64>, season: Option<String>) -> Result<()> {
    println!("列出動畫");
    Ok(())
}

pub async fn links(api_url: &str, anime_id: i64, series: Option<i32>, group: Option<String>) -> Result<()> {
    println!("列出動畫 {} 的連結", anime_id);
    Ok(())
}

pub async fn filter_add(api_url: &str, series_id: i64, group: &str, rule_type: &str, regex: &str) -> Result<()> {
    println!("添加過濾規則: {} {} {} {}", series_id, group, rule_type, regex);
    Ok(())
}

pub async fn filter_list(api_url: &str, series_id: i64, group: &str) -> Result<()> {
    println!("列出過濾規則: {} {}", series_id, group);
    Ok(())
}

pub async fn filter_remove(api_url: &str, rule_id: i64) -> Result<()> {
    println!("刪除過濾規則: {}", rule_id);
    Ok(())
}

pub async fn download(api_url: &str, link_id: i64, downloader: Option<String>) -> Result<()> {
    println!("下載連結: {}", link_id);
    Ok(())
}

pub async fn status(api_url: &str) -> Result<()> {
    println!("查看狀態");
    Ok(())
}

pub async fn services(api_url: &str) -> Result<()> {
    println!("列出服務");
    Ok(())
}

pub async fn logs(api_url: &str, log_type: &str) -> Result<()> {
    println!("查看日誌: {}", log_type);
    Ok(())
}
