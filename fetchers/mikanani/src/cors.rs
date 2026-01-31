use tower_http::cors::CorsLayer;

/// 建立 CORS 層
///
/// # 環境變數
/// - `ENABLE_CORS`: 是否啟用 CORS（"true" 或 "false"，預設為 "true"）
/// - `CORS_ALLOWED_ORIGINS`: 允許的來源，多個用逗號分隔（預設為 "*"）
///
/// 注意：目前實現支持 "*"（允許所有來源）。
/// 特定來源配置需要在 tower-http 支持字符串域名時實現。
pub fn create_cors_layer() -> Option<CorsLayer> {
    let enable_cors = std::env::var("ENABLE_CORS")
        .unwrap_or_else(|_| "true".to_string())
        .to_lowercase() == "true";

    if !enable_cors {
        tracing::info!("CORS 已禁用");
        return None;
    }

    let allowed_origins = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "*".to_string());

    // 目前使用寬鬆的 CORS 政策（允許所有來源和方法）
    // 當需要限制特定來源時，可以使用 CorsLayer::new() 和 allow_origin() 方法
    let cors = CorsLayer::permissive();

    if allowed_origins == "*" {
        tracing::info!("CORS 已啟用 - 允許所有來源");
    } else {
        tracing::warn!("CORS 已啟用 - 僅允許的來源配置需要進一步實現: {}", allowed_origins);
    }

    Some(cors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // 使用 mutex 確保測試不會並行執行（避免環境變數競爭）
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_cors_disabled() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::set_var("ENABLE_CORS", "false");
        let cors = create_cors_layer();
        std::env::remove_var("ENABLE_CORS");
        assert!(cors.is_none());
    }

    #[test]
    fn test_cors_enabled() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::set_var("ENABLE_CORS", "true");
        std::env::set_var("CORS_ALLOWED_ORIGINS", "*");
        let cors = create_cors_layer();
        std::env::remove_var("ENABLE_CORS");
        std::env::remove_var("CORS_ALLOWED_ORIGINS");
        assert!(cors.is_some());
    }
}
