use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tracing_subscriber;

mod config;
mod handlers;
mod models;
mod services;
mod db;
mod schema;
mod state;
mod dto;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        .unwrap_or_else(|_| "postgresql://bangumi:bangumi_password@localhost:5432/bangumi".to_string());

    let pool = db::establish_connection_pool(&database_url)?;

    // 嘗試運行遷移（如果 PostgreSQL 未運行將失敗，但這是可以接受的）
    match db::run_migrations(&pool) {
        Ok(_) => tracing::info!("數據庫遷移完成"),
        Err(e) => tracing::warn!("數據庫遷移失敗: {}", e),
    }

    // 初始化應用狀態
    let registry = std::sync::Arc::new(services::ServiceRegistry::new());
    let app_state = state::AppState {
        db: pool,
        registry,
    };

    // 構建應用路由
    let app = Router::new()
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
        .route("/filters", post(handlers::filters::create_filter))
        .route("/filters/:series_id/:group_id", get(handlers::filters::list_filters))
        .route("/filters/:rule_id", post(handlers::filters::delete_filter))

        // 健康檢查
        .route("/health", get(health_check))

        // 應用狀態
        .with_state(app_state);

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
