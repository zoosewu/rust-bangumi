// API 路由常數
pub mod routes {
    pub const SERVICES_REGISTER: &str = "/services/register";
    pub const SERVICES_LIST: &str = "/services";
    pub const SERVICES_BY_TYPE: &str = "/services/:service_type";
    pub const SERVICES_HEALTH: &str = "/services/:service_id/health";

    pub const ANIME_LIST: &str = "/anime";
    pub const ANIME_DETAIL: &str = "/anime/:anime_id";
    pub const ANIME_SERIES: &str = "/anime/:anime_id/series/:series_no";
    pub const ANIME_LINKS: &str = "/anime/:anime_id/links";

    pub const FETCH_TRIGGER: &str = "/fetch/:subscription_id";
    pub const DOWNLOAD_TRIGGER: &str = "/download/:link_id";
    pub const DOWNLOAD_CALLBACK: &str = "/download-callback/progress";
    pub const SYNC_CALLBACK: &str = "/sync-callback";

    pub const FILTER_RULES_CREATE: &str = "/filters";
    pub const FILTER_RULES_LIST: &str = "/filters/:series_id/:group_id";
    pub const FILTER_RULES_DELETE: &str = "/filters/:rule_id";

    pub const CRON_STATUS: &str = "/cron/status";
    pub const CRON_LIST: &str = "/cron/jobs";
    pub const CRON_ADD: &str = "/cron/jobs";
    pub const CRON_DISABLE: &str = "/cron/jobs/:subscription_id/disable";

    pub const LOGS: &str = "/logs";

    // New Architecture Routes
    pub const RAW_FETCHER_RESULTS: &str = "/raw-fetcher-results";
    pub const PARSERS: &str = "/parsers";
    pub const PARSERS_BY_ID: &str = "/parsers/:parser_id";
    pub const RAW_ITEMS: &str = "/raw-items";
    pub const RAW_ITEMS_BY_ID: &str = "/raw-items/:item_id";
    pub const RAW_ITEMS_REPARSE: &str = "/raw-items/:item_id/reparse";
    pub const RAW_ITEMS_SKIP: &str = "/raw-items/:item_id/skip";
}

// HTTP 頭部常數
pub mod headers {
    pub const CONTENT_TYPE_JSON: &str = "application/json";
    pub const X_SERVICE_TYPE: &str = "X-Service-Type";
    pub const X_SERVICE_NAME: &str = "X-Service-Name";
}

// 默認配置
pub mod defaults {
    pub const DEFAULT_CORE_SERVICE_PORT: u16 = 8000;
    pub const DEFAULT_FETCHER_PORT: u16 = 8001;
    pub const DEFAULT_DOWNLOADER_PORT: u16 = 8002;
    pub const DEFAULT_VIEWER_PORT: u16 = 8003;
    pub const HEALTH_CHECK_INTERVAL_SECS: u64 = 30;
    pub const HTTP_TIMEOUT_SECS: u64 = 30;
    pub const DOWNLOAD_PROGRESS_UPDATE_INTERVAL_SECS: u64 = 30;
}

// 重試配置
pub mod retry {
    pub const MAX_RETRIES: u32 = 20;
    pub const INITIAL_DELAY_SECS: u64 = 60;
    pub const BACKOFF_MULTIPLIER: u64 = 2;
}
