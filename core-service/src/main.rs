use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post, delete},
    Router,
};
use std::net::SocketAddr;
use tracing_subscriber;
use uuid;

mod config;
mod cors;
mod handlers;
mod models;
mod services;
mod db;
mod schema;
mod state;
mod dto;

use shared;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 載入 .env 檔案
    dotenv::dotenv().ok();

    // 初始化日誌
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("core_service=debug".parse()?),
        )
        .init();

    tracing::info!("啟動核心服務");

    // 設置數據庫連接池
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://bangumi:bangumi_dev_password@172.20.0.2:5432/bangumi".to_string());

    let pool = db::establish_connection_pool(&database_url)?;

    // 嘗試運行遷移（如果 PostgreSQL 未運行將失敗，但這是可以接受的）
    match db::run_migrations(&pool) {
        Ok(_) => tracing::info!("數據庫遷移完成"),
        Err(e) => tracing::warn!("數據庫遷移失敗: {}", e),
    }

    // 初始化應用狀態
    let registry = std::sync::Arc::new(services::ServiceRegistry::new());
    let subscription_broadcaster = services::create_subscription_broadcaster();
    let app_state = state::AppState {
        db: pool,
        registry: registry.clone(),
        subscription_broadcaster,
    };

    // 啟動時從資料庫載入已有的所有服務模塊
    load_existing_services(&app_state).await;

    // 構建應用路由
    let mut app = Router::new()
        // 服務註冊
        .route("/services/register", post(handlers::services::register))
        .route("/services", get(handlers::services::list_services))
        .route("/services/:service_type", get(handlers::services::list_by_type))
        .route("/services/:service_id/health", get(handlers::services::health_check))

        // 動畫管理
        .route("/anime", post(handlers::anime::create_anime))
        .route("/anime", get(handlers::anime::list_anime))
        .route("/anime/:anime_id", get(handlers::anime::get_anime))
        .route("/anime/:anime_id", axum::routing::delete(handlers::anime::delete_anime))

        // 季度管理
        .route("/seasons", post(handlers::anime::create_season))
        .route("/seasons", get(handlers::anime::list_seasons))

        // 動畫系列管理
        .route("/anime/series", post(handlers::anime::create_anime_series))
        .route("/anime/series/:series_id", get(handlers::anime::get_anime_series))
        .route("/anime/:anime_id/series", get(handlers::anime::list_anime_series))

        // 字幕組管理
        .route("/subtitle-groups", post(handlers::anime::create_subtitle_group))
        .route("/subtitle-groups", get(handlers::anime::list_subtitle_groups))
        .route("/subtitle-groups/:group_id", axum::routing::delete(handlers::anime::delete_subtitle_group))

        // 過濾規則
        .route("/filters", post(handlers::filters::create_filter_rule))
        .route("/filters/:series_id/:group_id", get(handlers::filters::get_filter_rules))
        .route("/filters/:rule_id", delete(handlers::filters::delete_filter_rule))

        // 動畫連結
        .route("/links", post(handlers::links::create_anime_link))
        .route("/links/:series_id", get(handlers::links::get_anime_links))

        // 訂閱管理
        .route("/subscriptions", post(handlers::subscriptions::create_subscription))
        .route("/subscriptions", get(handlers::subscriptions::list_subscriptions))
        .route("/fetcher-modules/:fetcher_id/subscriptions", get(handlers::subscriptions::get_fetcher_subscriptions))
        .route("/fetcher-modules", get(handlers::subscriptions::list_fetcher_modules))
        .route("/subscriptions/:rss_url", delete(handlers::subscriptions::delete_subscription))

        // Fetcher 結果接收
        .route("/fetcher-results", post(handlers::fetcher_results::receive_fetcher_results))

        // 衝突解決
        .route("/conflicts", get(handlers::conflict_resolution::get_pending_conflicts))
        .route("/conflicts/:conflict_id/resolve", post(handlers::conflict_resolution::resolve_conflict))

        // 健康檢查
        .route("/health", get(health_check))

        // 應用狀態
        .with_state(app_state);

    // 有條件地應用 CORS 中間件
    if let Some(cors) = cors::create_cors_layer() {
        app = app.layer(cors);
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("核心服務監聽於 {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "core-service"
    }))
}

/// Load all service modules (Fetcher, Downloader, Viewer) from database and register them in memory
async fn load_existing_services(app_state: &state::AppState) {
    use crate::schema::service_modules;
    use crate::models::{ServiceModule, ModuleTypeEnum};
    use diesel::prelude::*;

    match app_state.db.get() {
        Ok(mut conn) => {
            // Load all enabled service modules
            match service_modules::table
                .filter(service_modules::is_enabled.eq(true))
                .select(ServiceModule::as_select())
                .load::<ServiceModule>(&mut conn)
            {
                Ok(modules) => {
                    for module in modules {
                        let service_id = uuid::Uuid::new_v4();
                        let (service_type, capabilities) = match module.module_type {
                            ModuleTypeEnum::Fetcher => (
                                shared::ServiceType::Fetcher,
                                shared::Capabilities {
                                    fetch_endpoint: Some("/fetch".to_string()),
                                    download_endpoint: None,
                                    sync_endpoint: None,
                                },
                            ),
                            ModuleTypeEnum::Downloader => (
                                shared::ServiceType::Downloader,
                                shared::Capabilities {
                                    fetch_endpoint: None,
                                    download_endpoint: Some("/download".to_string()),
                                    sync_endpoint: None,
                                },
                            ),
                            ModuleTypeEnum::Viewer => (
                                shared::ServiceType::Viewer,
                                shared::Capabilities {
                                    fetch_endpoint: None,
                                    download_endpoint: None,
                                    sync_endpoint: Some("/sync".to_string()),
                                },
                            ),
                        };

                        let service = shared::RegisteredService {
                            service_id,
                            service_type: service_type.clone(),
                            service_name: module.name.clone(),
                            host: extract_host(&module.base_url),
                            port: extract_port(&module.base_url),
                            capabilities,
                            is_healthy: true,
                            last_heartbeat: chrono::Utc::now(),
                        };

                        if let Err(e) = app_state.registry.register(service) {
                            tracing::error!("Failed to load {} {} into registry: {}", module.module_type, module.name, e);
                        } else {
                            tracing::info!("Loaded {} module from database: {} ({})", module.module_type, module.name, module.base_url);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load service modules from database: {}", e);
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection for loading services: {}", e);
        }
    }
}

/// Extract host from URL (e.g., "http://localhost:8001" -> "localhost")
fn extract_host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|s| s.split(':').next())
        .unwrap_or("unknown")
        .to_string()
}

/// Extract port from URL (e.g., "http://localhost:8001" -> 8001)
fn extract_port(url: &str) -> u16 {
    url.split(':')
        .last()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0)
}
