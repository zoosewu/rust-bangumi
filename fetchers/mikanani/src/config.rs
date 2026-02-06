/// Fetcher 服務配置
#[derive(Debug, Clone)]
pub struct FetcherConfig {
    pub core_service_url: String,
    pub service_host: String,
    pub service_port: u16,
    pub service_name: String,
}

impl FetcherConfig {
    /// 從環境變數載入配置
    pub fn from_env() -> Self {
        Self {
            core_service_url: std::env::var("CORE_SERVICE_URL")
                .unwrap_or_else(|_| "http://core-service:8000".to_string()),
            service_host: std::env::var("SERVICE_HOST")
                .unwrap_or_else(|_| "fetcher-mikanani".to_string()),
            service_port: std::env::var("SERVICE_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8001),
            service_name: "mikanani".to_string(),
        }
    }

    /// 建立測試用配置
    pub fn for_test() -> Self {
        Self {
            core_service_url: "http://test-core:8000".to_string(),
            service_host: "test-fetcher".to_string(),
            service_port: 8001,
            service_name: "mikanani".to_string(),
        }
    }

    /// 自訂配置
    pub fn new(
        core_service_url: String,
        service_host: String,
        service_port: u16,
        service_name: String,
    ) -> Self {
        Self {
            core_service_url,
            service_host,
            service_port,
            service_name,
        }
    }

    /// 取得 fetcher-results callback URL
    pub fn callback_url(&self) -> String {
        format!("{}/fetcher-results", self.core_service_url)
    }

    /// 取得服務註冊 URL
    pub fn register_url(&self) -> String {
        format!("{}/services/register", self.core_service_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_callback_url() {
        let config = FetcherConfig::new(
            "http://localhost:8000".to_string(),
            "localhost".to_string(),
            8001,
            "test".to_string(),
        );
        assert_eq!(
            config.callback_url(),
            "http://localhost:8000/fetcher-results"
        );
    }

    #[test]
    fn test_register_url() {
        let config = FetcherConfig::new(
            "http://localhost:8000".to_string(),
            "localhost".to_string(),
            8001,
            "test".to_string(),
        );
        assert_eq!(
            config.register_url(),
            "http://localhost:8000/services/register"
        );
    }
}
