use colored::Colorize;
use serde::Serialize;

/// 列印任何 Serialize 類型為 JSON（--json 模式）
pub fn print_json<T: Serialize>(data: &T) {
    match serde_json::to_string_pretty(data) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("JSON 序列化失敗: {}", e),
    }
}

/// 列印 key-value 詳情（show 指令用）
pub fn print_kv(title: &str, pairs: &[(&str, String)]) {
    println!("{}", title.bold());
    for (k, v) in pairs {
        println!("  {}: {}", k.cyan(), v);
    }
}

/// 格式化狀態顯示（帶顏色）
pub fn format_status(status: &str) -> String {
    match status {
        "active" | "completed" | "synced" | "healthy" | "true" | "parsed" => {
            status.green().to_string()
        }
        "downloading" | "pending" | "in_progress" => status.yellow().to_string(),
        "failed" | "error" | "unhealthy" | "false" | "no_match" => {
            status.red().to_string()
        }
        "skipped" | "paused" | "inactive" => status.dimmed().to_string(),
        _ => status.to_string(),
    }
}

/// 格式化布林值
pub fn format_bool(v: bool) -> String {
    if v {
        "✓".green().to_string()
    } else {
        "✗".red().to_string()
    }
}

/// 格式化 Option<String>，None 顯示 "-"
pub fn opt_str(v: &Option<String>) -> String {
    v.as_deref().unwrap_or("-").to_string()
}

/// 格式化 Option<i64>
pub fn opt_i64(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "-".to_string())
}

/// 顯示成功訊息
pub fn print_success(msg: &str) {
    println!("{} {}", "✓".green(), msg);
}

/// 顯示錯誤訊息
pub fn print_error(msg: &str) {
    eprintln!("{} {}", "✗".red(), msg);
}
