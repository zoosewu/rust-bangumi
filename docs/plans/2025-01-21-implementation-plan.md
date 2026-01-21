# Rust Bangumi 實現計劃

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 實現 Rust Bangumi 微服務系統的完整功能，包括數據庫層、核心服務、各個微服務區塊和 CLI 工具。

**Architecture:** 使用 Diesel ORM 管理 PostgreSQL 數據庫，核心服務通過 Axum 暴露 REST API，各微服務通過動態註冊與核心服務通信，CLI 工具調用 API 與用戶交互。

**Tech Stack:** Rust 1.75+, Diesel, PostgreSQL 15, Axum, Tokio, Clap

---

## Phase 1: 數據庫與 Diesel 遷移

### Task 1: 安裝 Diesel CLI 和配置

**Files:**
- Modify: `core-service/Cargo.toml`
- Create: `core-service/.diesel/config.toml`
- Modify: `Cargo.toml` (workspace)

**Step 1: 添加 Diesel 依賴到 core-service**

編輯 `core-service/Cargo.toml`，在 `[dependencies]` 中添加：

```toml
# 在現有 workspace 依賴後添加
diesel = { version = "2.1", features = ["postgres", "chrono", "uuid"] }
diesel_migrations = "2.1"
```

同時在 workspace `Cargo.toml` 的 `[workspace.dependencies]` 添加：

```toml
diesel = { version = "2.1", features = ["postgres", "chrono", "uuid"] }
diesel_migrations = "2.1"
```

**Step 2: 安裝 Diesel CLI**

```bash
cargo install diesel_cli --no-default-features --features postgres
```

**Step 3: 初始化 Diesel**

```bash
cd core-service
diesel setup --database-url "postgresql://bangumi:bangumi_password@localhost:5432/bangumi"
```

Expected: 創建 `core-service/diesel.toml` 和 `core-service/migrations/` 目錄

**Step 4: 驗證 Diesel 設置**

```bash
diesel migration list
```

Expected: 空列表（尚無遷移）

**Step 5: Commit**

```bash
git add core-service/Cargo.toml core-service/diesel.toml
git commit -m "chore: Set up Diesel ORM and migrations

- Add diesel and diesel_migrations to core-service
- Initialize diesel with PostgreSQL configuration
- Configure diesel.toml for migrations"
```

---

### Task 2: 創建數據庫遷移 - 基礎表

**Files:**
- Create: `core-service/migrations/<timestamp>_create_seasons/up.sql`
- Create: `core-service/migrations/<timestamp>_create_seasons/down.sql`
- Create: `core-service/migrations/<timestamp>_create_animes/up.sql`
- Create: `core-service/migrations/<timestamp>_create_animes/down.sql`

**Step 1: 創建 seasons 表遷移**

運行：
```bash
cd core-service
diesel migration generate create_seasons
```

編輯 `migrations/<timestamp>_create_seasons/up.sql`：

```sql
CREATE TABLE seasons (
  season_id SERIAL PRIMARY KEY,
  year INTEGER NOT NULL,
  season VARCHAR(10) NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(year, season)
);
```

編輯 `migrations/<timestamp>_create_seasons/down.sql`：

```sql
DROP TABLE IF EXISTS seasons;
```

**Step 2: 創建 animes 表遷移**

運行：
```bash
diesel migration generate create_animes
```

編輯 up.sql：

```sql
CREATE TABLE animes (
  anime_id SERIAL PRIMARY KEY,
  title VARCHAR(255) NOT NULL UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

編輯 down.sql：

```sql
DROP TABLE IF EXISTS animes;
```

**Step 3: 運行遷移**

```bash
diesel migration run
```

Expected: 兩個表被創建

**Step 4: 驗證遷移**

```bash
diesel migration list
```

Expected: 兩個遷移標記為 `applied`

**Step 5: Commit**

```bash
git add core-service/migrations/
git commit -m "feat: Create seasons and animes tables

- Create seasons table for year/season tracking
- Create animes table for anime metadata
- Add corresponding down migrations"
```

---

### Task 3: 創建數據庫遷移 - 動畫季數與字幕組

**Files:**
- Create: `core-service/migrations/<timestamp>_create_anime_series/up.sql`
- Create: `core-service/migrations/<timestamp>_create_anime_series/down.sql`
- Create: `core-service/migrations/<timestamp>_create_subtitle_groups/up.sql`
- Create: `core-service/migrations/<timestamp>_create_subtitle_groups/down.sql`

**Step 1: 創建 anime_series 表**

```bash
diesel migration generate create_anime_series
```

up.sql：

```sql
CREATE TABLE anime_series (
  series_id SERIAL PRIMARY KEY,
  anime_id INTEGER NOT NULL REFERENCES animes(anime_id) ON DELETE CASCADE,
  series_no INTEGER NOT NULL,
  season_id INTEGER NOT NULL REFERENCES seasons(season_id) ON DELETE CASCADE,
  description TEXT,
  aired_date DATE,
  end_date DATE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(anime_id, series_no)
);

CREATE INDEX idx_anime_series_anime_id ON anime_series(anime_id);
CREATE INDEX idx_anime_series_season_id ON anime_series(season_id);
```

down.sql：

```sql
DROP TABLE IF EXISTS anime_series;
```

**Step 2: 創建 subtitle_groups 表**

```bash
diesel migration generate create_subtitle_groups
```

up.sql：

```sql
CREATE TABLE subtitle_groups (
  group_id SERIAL PRIMARY KEY,
  group_name VARCHAR(255) NOT NULL UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

down.sql：

```sql
DROP TABLE IF EXISTS subtitle_groups;
```

**Step 3: 運行遷移**

```bash
diesel migration run
```

**Step 4: Commit**

```bash
git add core-service/migrations/
git commit -m "feat: Create anime_series and subtitle_groups tables

- Add anime_series table with season relationship
- Add subtitle_groups table for grouping links
- Create indexes on foreign key columns"
```

---

### Task 4: 創建數據庫遷移 - 動畫連結、過濾規則、下載與日誌

**Files:**
- Create: `core-service/migrations/<timestamp>_create_anime_links/up.sql`
- Create: `core-service/migrations/<timestamp>_create_anime_links/down.sql`
- Create: `core-service/migrations/<timestamp>_create_filter_rules/up.sql`
- Create: `core-service/migrations/<timestamp>_create_filter_rules/down.sql`
- Create: `core-service/migrations/<timestamp>_create_downloads/up.sql`
- Create: `core-service/migrations/<timestamp>_create_downloads/down.sql`
- Create: `core-service/migrations/<timestamp>_create_cron_logs/up.sql`
- Create: `core-service/migrations/<timestamp>_create_cron_logs/down.sql`

**Step 1: 創建 anime_links 表**

```bash
diesel migration generate create_anime_links
```

up.sql：

```sql
CREATE TABLE anime_links (
  link_id SERIAL PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES anime_series(series_id) ON DELETE CASCADE,
  group_id INTEGER NOT NULL REFERENCES subtitle_groups(group_id) ON DELETE CASCADE,
  episode_no INTEGER NOT NULL,
  title VARCHAR(255),
  url TEXT NOT NULL,
  source_hash VARCHAR(255) NOT NULL,
  filtered_flag BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(series_id, group_id, episode_no, source_hash)
);

CREATE INDEX idx_anime_links_series_id ON anime_links(series_id);
CREATE INDEX idx_anime_links_group_id ON anime_links(group_id);
CREATE INDEX idx_anime_links_filtered ON anime_links(filtered_flag);
```

down.sql：

```sql
DROP TABLE IF EXISTS anime_links;
```

**Step 2: 創建 filter_rules 表**

```bash
diesel migration generate create_filter_rules
```

up.sql：

```sql
CREATE TABLE filter_rules (
  rule_id SERIAL PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES anime_series(series_id) ON DELETE CASCADE,
  group_id INTEGER NOT NULL REFERENCES subtitle_groups(group_id) ON DELETE CASCADE,
  rule_order INTEGER NOT NULL,
  rule_type VARCHAR(20) NOT NULL CHECK (rule_type IN ('Positive', 'Negative')),
  regex_pattern TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(series_id, group_id, rule_order)
);

CREATE INDEX idx_filter_rules_series_group ON filter_rules(series_id, group_id);
```

down.sql：

```sql
DROP TABLE IF EXISTS filter_rules;
```

**Step 3: 創建 downloads 表**

```bash
diesel migration generate create_downloads
```

up.sql：

```sql
CREATE TABLE downloads (
  download_id SERIAL PRIMARY KEY,
  link_id INTEGER NOT NULL REFERENCES anime_links(link_id) ON DELETE CASCADE,
  downloader_type VARCHAR(50) NOT NULL,
  status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'downloading', 'completed', 'failed')),
  progress DECIMAL(5, 2) DEFAULT 0.0,
  downloaded_bytes BIGINT DEFAULT 0,
  total_bytes BIGINT DEFAULT 0,
  error_message TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_downloads_link_id ON downloads(link_id);
CREATE INDEX idx_downloads_status ON downloads(status);
```

down.sql：

```sql
DROP TABLE IF EXISTS downloads;
```

**Step 4: 創建 cron_logs 表**

```bash
diesel migration generate create_cron_logs
```

up.sql：

```sql
CREATE TABLE cron_logs (
  log_id SERIAL PRIMARY KEY,
  fetcher_type VARCHAR(50) NOT NULL,
  status VARCHAR(20) NOT NULL CHECK (status IN ('success', 'failed')),
  error_message TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 1,
  executed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_cron_logs_fetcher_type ON cron_logs(fetcher_type);
CREATE INDEX idx_cron_logs_executed_at ON cron_logs(executed_at);
```

down.sql：

```sql
DROP TABLE IF EXISTS cron_logs;
```

**Step 5: 運行所有遷移**

```bash
diesel migration run
```

Expected: 所有遷移成功應用

**Step 6: Commit**

```bash
git add core-service/migrations/
git commit -m "feat: Create anime_links, filter_rules, downloads, and cron_logs tables

- Add anime_links table with source_hash and filtered_flag
- Add filter_rules table with ordered regex patterns
- Add downloads table for tracking download status
- Add cron_logs table for job execution tracking
- Create appropriate indexes for performance"
```

---

### Task 5: 生成 Diesel Schema 和模型

**Files:**
- Create: `core-service/src/schema.rs`
- Modify: `core-service/src/lib.rs`
- Create: `core-service/src/models/db.rs`

**Step 1: 生成 Diesel schema**

```bash
cd core-service
diesel print-schema > src/schema.rs
```

Expected: `src/schema.rs` 自動生成

**Step 2: 編輯 `core-service/src/lib.rs`**

```rust
pub mod schema;
pub mod models;
```

**Step 3: 創建 `core-service/src/models/db.rs`**

```rust
use diesel::prelude::*;
use chrono::{DateTime, NaiveDate, Utc};

// ============ Seasons ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::seasons)]
pub struct Season {
    pub season_id: i32,
    pub year: i32,
    pub season: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::seasons)]
pub struct NewSeason {
    pub year: i32,
    pub season: String,
}

// ============ Animes ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::animes)]
pub struct Anime {
    pub anime_id: i32,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::animes)]
pub struct NewAnime {
    pub title: String,
}

// ============ AnimeSeries ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::anime_series)]
pub struct AnimeSeries {
    pub series_id: i32,
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::anime_series)]
pub struct NewAnimeSeries {
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

// ============ SubtitleGroups ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::subtitle_groups)]
pub struct SubtitleGroup {
    pub group_id: i32,
    pub group_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::subtitle_groups)]
pub struct NewSubtitleGroup {
    pub group_name: String,
}

// ============ AnimeLinks ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::anime_links)]
pub struct AnimeLink {
    pub link_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::anime_links)]
pub struct NewAnimeLink {
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
}

// ============ FilterRules ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::filter_rules)]
pub struct FilterRule {
    pub rule_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub rule_order: i32,
    pub rule_type: String,
    pub regex_pattern: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::filter_rules)]
pub struct NewFilterRule {
    pub series_id: i32,
    pub group_id: i32,
    pub rule_order: i32,
    pub rule_type: String,
    pub regex_pattern: String,
}

// ============ Downloads ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::downloads)]
pub struct Download {
    pub download_id: i32,
    pub link_id: i32,
    pub downloader_type: String,
    pub status: String,
    pub progress: Option<f64>,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::downloads)]
pub struct NewDownload {
    pub link_id: i32,
    pub downloader_type: String,
}

// ============ CronLogs ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::cron_logs)]
pub struct CronLog {
    pub log_id: i32,
    pub fetcher_type: String,
    pub status: String,
    pub error_message: Option<String>,
    pub attempt_count: i32,
    pub executed_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::cron_logs)]
pub struct NewCronLog {
    pub fetcher_type: String,
    pub status: String,
    pub error_message: Option<String>,
    pub attempt_count: i32,
}
```

**Step 4: 修改 `core-service/src/models/mod.rs`**

```rust
pub mod db;
pub use db::*;
```

**Step 5: 驗證編譯**

```bash
cargo check --package core-service
```

Expected: 編譯成功

**Step 6: Commit**

```bash
git add core-service/src/schema.rs core-service/src/models/
git commit -m "feat: Generate Diesel schema and define database models

- Auto-generate schema from migrations
- Create Diesel models for all tables with Queryable/Insertable derives
- Organize models in separate db.rs module"
```

---

## Phase 2: 核心服務的數據庫訪問層

### Task 6: 實現數據庫連接池和初始化

**Files:**
- Modify: `core-service/src/main.rs`
- Create: `core-service/src/db.rs`

**Step 1: 修改 `core-service/Cargo.toml`，添加 connection pool**

在 `[dependencies]` 添加：

```toml
diesel-async = { version = "0.4", features = ["postgres", "deadpool"] }
```

同時在 workspace `Cargo.toml` 添加：

```toml
diesel-async = { version = "0.4", features = ["postgres", "deadpool"] }
```

**Step 2: 創建 `core-service/src/db.rs`**

```rust
use diesel_async::{
    pooled_connection::deadpool::Pool, AsyncPgConnection, AsyncConnection,
};
use diesel::prelude::*;

pub type DbPool = Pool<AsyncPgConnection>;

pub async fn establish_connection_pool(database_url: &str) -> Result<DbPool, Box<dyn std::error::Error>> {
    let manager = diesel_async::pooled_connection::deadpool::Object::builder()
        .create_manager(database_url, || async {})
        .await?;

    let pool = Pool::builder(manager).build()?;

    Ok(pool)
}

pub async fn run_migrations(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = pool.get().await?;

    diesel_migrations::run_pending_migrations(&mut conn)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    Ok(())
}
```

實際上，由於 diesel-async 的複雜性，讓我們使用更簡單的方法，使用 `diesel::r2d2` 和阻塞式連接：

**重新編寫 `core-service/src/db.rs`**

```rust
use diesel::r2d2::{self, ConnectionManager};
use diesel::PgConnection;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type DbConnection = r2d2::PooledConnection<ConnectionManager<PgConnection>>;

pub fn establish_connection_pool(database_url: &str) -> Result<DbPool, Box<dyn std::error::Error>> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .max_size(16)
        .build(manager)?;

    Ok(pool)
}

pub fn run_migrations(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = pool.get()?;
    diesel_migrations::run_pending_migrations(&mut conn)?;
    Ok(())
}
```

**Step 3: 修改 workspace `Cargo.toml`，使用 diesel r2d2**

在 `[workspace.dependencies]` 修改/添加：

```toml
diesel = { version = "2.1", features = ["postgres", "chrono", "uuid", "r2d2"] }
diesel_migrations = "2.1"
```

**Step 4: 修改 `core-service/src/main.rs`**

修改 main 函數（在現有代碼基礎上）：

```rust
mod db;

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

    // 運行遷移
    db::run_migrations(&pool)?;
    tracing::info!("數據庫遷移完成");

    // 構建應用路由
    let app = Router::new()
        // ... 現有路由 ...
        .with_state(pool.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("核心服務監聽於 {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
```

**Step 5: 驗證編譯**

```bash
cargo check --package core-service
```

**Step 6: Commit**

```bash
git add core-service/src/db.rs core-service/src/main.rs core-service/Cargo.toml
git commit -m "feat: Implement database connection pool with r2d2

- Add diesel r2d2 connection pool setup
- Implement migration runner
- Pass pool to application state"
```

---

---

## Phase 2: 核心服務的數據庫訪問層（續）

### Task 7: 實現服務註冊數據庫操作

**Files:**
- Create: `core-service/src/services/mod.rs`
- Create: `core-service/src/services/registry.rs`

**Step 1: 創建 `core-service/src/services/registry.rs`**

```rust
use crate::db::{DbConnection, DbPool};
use crate::models::*;
use crate::schema::*;
use diesel::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use shared::{ServiceType, RegisteredService};
use chrono::Utc;
use uuid::Uuid;

pub struct ServiceRegistry {
    services: Arc<Mutex<HashMap<Uuid, RegisteredService>>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, service: RegisteredService) -> Result<(), String> {
        let mut services = self.services.lock().map_err(|e| e.to_string())?;
        services.insert(service.service_id, service);
        tracing::info!("Service registered");
        Ok(())
    }

    pub fn get_services(&self) -> Result<Vec<RegisteredService>, String> {
        let services = self.services.lock().map_err(|e| e.to_string())?;
        Ok(services.values().cloned().collect())
    }

    pub fn get_services_by_type(&self, service_type: &ServiceType) -> Result<Vec<RegisteredService>, String> {
        let services = self.services.lock().map_err(|e| e.to_string())?;
        Ok(services
            .values()
            .filter(|s| &s.service_type == service_type)
            .cloned()
            .collect())
    }

    pub fn unregister(&self, service_id: Uuid) -> Result<(), String> {
        let mut services = self.services.lock().map_err(|e| e.to_string())?;
        services.remove(&service_id);
        Ok(())
    }

    pub fn update_health(&self, service_id: Uuid, is_healthy: bool) -> Result<(), String> {
        let mut services = self.services.lock().map_err(|e| e.to_string())?;
        if let Some(service) = services.get_mut(&service_id) {
            service.is_healthy = is_healthy;
            service.last_heartbeat = chrono::Utc::now();
        }
        Ok(())
    }
}
```

**Step 2: 修改 `core-service/src/services/mod.rs`**

```rust
pub mod registry;
pub use registry::ServiceRegistry;
```

**Step 3: 驗證編譯**

```bash
cargo check --package core-service
```

**Step 4: Commit**

```bash
git add core-service/src/services/
git commit -m "feat: Implement in-memory service registry

- Create ServiceRegistry for managing registered services
- Implement register, list, unregister operations
- Track service health status"
```

---

### Task 8: 實現數據庫查詢操作層

**Files:**
- Create: `core-service/src/db/models.rs`
- Modify: `core-service/src/db.rs`

**Step 1: 創建 `core-service/src/db/models.rs`**

```rust
use crate::db::DbConnection;
use crate::models::*;
use crate::schema::*;
use diesel::prelude::*;

pub fn create_anime(conn: &mut DbConnection, new_anime: NewAnime) -> Result<Anime, diesel::result::Error> {
    diesel::insert_into(animes::table)
        .values(&new_anime)
        .get_result(conn)
}

pub fn get_anime_by_id(conn: &mut DbConnection, anime_id: i32) -> Result<Anime, diesel::result::Error> {
    animes::table.find(anime_id).first(conn)
}

pub fn get_anime_by_title(conn: &mut DbConnection, title: &str) -> Result<Anime, diesel::result::Error> {
    animes::table.filter(animes::title.eq(title)).first(conn)
}

pub fn create_season(conn: &mut DbConnection, new_season: NewSeason) -> Result<Season, diesel::result::Error> {
    diesel::insert_into(seasons::table)
        .values(&new_season)
        .get_result(conn)
}

pub fn get_or_create_season(conn: &mut DbConnection, year: i32, season: String) -> Result<Season, diesel::result::Error> {
    let existing = seasons::table
        .filter(seasons::year.eq(year).and(seasons::season.eq(&season)))
        .first::<Season>(conn)
        .optional()?;

    if let Some(season) = existing {
        Ok(season)
    } else {
        diesel::insert_into(seasons::table)
            .values(NewSeason { year, season })
            .get_result(conn)
    }
}

pub fn create_anime_series(conn: &mut DbConnection, new_series: NewAnimeSeries) -> Result<AnimeSeries, diesel::result::Error> {
    diesel::insert_into(anime_series::table)
        .values(&new_series)
        .get_result(conn)
}

pub fn get_anime_series_by_id(conn: &mut DbConnection, series_id: i32) -> Result<AnimeSeries, diesel::result::Error> {
    anime_series::table.find(series_id).first(conn)
}

pub fn get_or_create_subtitle_group(conn: &mut DbConnection, group_name: String) -> Result<SubtitleGroup, diesel::result::Error> {
    let existing = subtitle_groups::table
        .filter(subtitle_groups::group_name.eq(&group_name))
        .first::<SubtitleGroup>(conn)
        .optional()?;

    if let Some(group) = existing {
        Ok(group)
    } else {
        diesel::insert_into(subtitle_groups::table)
            .values(NewSubtitleGroup { group_name })
            .get_result(conn)
    }
}

pub fn create_anime_link(conn: &mut DbConnection, new_link: NewAnimeLink) -> Result<AnimeLink, diesel::result::Error> {
    diesel::insert_into(anime_links::table)
        .values(&new_link)
        .get_result(conn)
}

pub fn get_anime_links_by_series(conn: &mut DbConnection, series_id: i32) -> Result<Vec<AnimeLink>, diesel::result::Error> {
    anime_links::table
        .filter(anime_links::series_id.eq(series_id).and(anime_links::filtered_flag.eq(false)))
        .load(conn)
}

pub fn get_filter_rules(conn: &mut DbConnection, series_id: i32, group_id: i32) -> Result<Vec<FilterRule>, diesel::result::Error> {
    filter_rules::table
        .filter(filter_rules::series_id.eq(series_id).and(filter_rules::group_id.eq(group_id)))
        .order_by(filter_rules::rule_order.asc())
        .load(conn)
}

pub fn create_filter_rule(conn: &mut DbConnection, new_rule: NewFilterRule) -> Result<FilterRule, diesel::result::Error> {
    diesel::insert_into(filter_rules::table)
        .values(&new_rule)
        .get_result(conn)
}

pub fn delete_filter_rule(conn: &mut DbConnection, rule_id: i32) -> Result<usize, diesel::result::Error> {
    diesel::delete(filter_rules::table.find(rule_id)).execute(conn)
}

pub fn create_download(conn: &mut DbConnection, new_download: NewDownload) -> Result<Download, diesel::result::Error> {
    diesel::insert_into(downloads::table)
        .values(&new_download)
        .get_result(conn)
}

pub fn get_download(conn: &mut DbConnection, download_id: i32) -> Result<Download, diesel::result::Error> {
    downloads::table.find(download_id).first(conn)
}

pub fn update_download_progress(
    conn: &mut DbConnection,
    download_id: i32,
    status: &str,
    progress: f64,
    downloaded_bytes: i64,
    total_bytes: i64,
) -> Result<Download, diesel::result::Error> {
    diesel::update(downloads::table.find(download_id))
        .set((
            downloads::status.eq(status),
            downloads::progress.eq(progress),
            downloads::downloaded_bytes.eq(downloaded_bytes),
            downloads::total_bytes.eq(total_bytes),
            downloads::updated_at.eq(chrono::Utc::now()),
        ))
        .get_result(conn)
}

pub fn create_cron_log(conn: &mut DbConnection, new_log: NewCronLog) -> Result<CronLog, diesel::result::Error> {
    diesel::insert_into(cron_logs::table)
        .values(&new_log)
        .get_result(conn)
}
```

**Step 2: 修改 `core-service/src/db.rs`**

在文件開頭添加：

```rust
pub mod models;
pub use models::*;
```

**Step 3: 驗證編譯**

```bash
cargo check --package core-service
```

**Step 4: Commit**

```bash
git add core-service/src/db/
git commit -m "feat: Implement database query operations

- Create helper functions for CRUD operations
- Implement anime, season, series, and link queries
- Add filter rule and download tracking functions"
```

---

## Phase 3: 核心服務的業務邏輯

### Task 9: 實現過濾規則應用引擎

**Files:**
- Create: `core-service/src/services/filter.rs`
- Modify: `core-service/src/services/mod.rs`

**Step 1: 創建 `core-service/src/services/filter.rs`**

```rust
use regex::Regex;
use crate::models::FilterRule;

pub struct FilterEngine {
    rules: Vec<FilterRule>,
}

impl FilterEngine {
    pub fn new(rules: Vec<FilterRule>) -> Self {
        Self { rules }
    }

    pub fn should_include(&self, text: &str) -> bool {
        if self.rules.is_empty() {
            return true;
        }

        let mut included = true;

        for rule in &self.rules {
            if let Ok(regex) = Regex::new(&rule.regex_pattern) {
                let matches = regex.is_match(text);

                match rule.rule_type.as_str() {
                    "Positive" => {
                        included = included && matches;
                    }
                    "Negative" => {
                        included = included && !matches;
                    }
                    _ => {}
                }
            } else {
                tracing::warn!("Invalid regex pattern: {}", rule.regex_pattern);
            }
        }

        included
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive_filter() {
        let rule = FilterRule {
            rule_id: 1,
            series_id: 1,
            group_id: 1,
            rule_order: 1,
            rule_type: "Positive".to_string(),
            regex_pattern: "1080p".to_string(),
            created_at: chrono::Utc::now(),
        };

        let engine = FilterEngine::new(vec![rule]);
        assert!(engine.should_include("anime 1080p"));
        assert!(!engine.should_include("anime 720p"));
    }

    #[test]
    fn test_negative_filter() {
        let rule = FilterRule {
            rule_id: 1,
            series_id: 1,
            group_id: 1,
            rule_order: 1,
            rule_type: "Negative".to_string(),
            regex_pattern: "trash".to_string(),
            created_at: chrono::Utc::now(),
        };

        let engine = FilterEngine::new(vec![rule]);
        assert!(engine.should_include("good quality"));
        assert!(!engine.should_include("trash quality"));
    }

    #[test]
    fn test_combined_filters() {
        let rules = vec![
            FilterRule {
                rule_id: 1,
                series_id: 1,
                group_id: 1,
                rule_order: 1,
                rule_type: "Positive".to_string(),
                regex_pattern: "1080p|720p".to_string(),
                created_at: chrono::Utc::now(),
            },
            FilterRule {
                rule_id: 2,
                series_id: 1,
                group_id: 1,
                rule_order: 2,
                rule_type: "Negative".to_string(),
                regex_pattern: "trash".to_string(),
                created_at: chrono::Utc::now(),
            },
        ];

        let engine = FilterEngine::new(rules);
        assert!(engine.should_include("anime 1080p good"));
        assert!(!engine.should_include("anime 1080p trash"));
        assert!(!engine.should_include("anime 480p"));
    }
}
```

**Step 2: 修改 `core-service/src/services/mod.rs`**

```rust
pub mod registry;
pub mod filter;

pub use registry::ServiceRegistry;
pub use filter::FilterEngine;
```

**Step 3: 運行測試**

```bash
cargo test --package core-service filter::tests
```

Expected: 所有 3 個測試通過

**Step 4: Commit**

```bash
git add core-service/src/services/filter.rs
git commit -m "feat: Implement filter rule engine with regex support

- Create FilterEngine for applying filter rules
- Support positive and negative regex patterns
- Add comprehensive unit tests"
```

---

### Task 10: 實現 Cron 任務調度服務

**Files:**
- Create: `core-service/src/services/scheduler.rs`
- Modify: `core-service/src/services/mod.rs`

**Step 1: 在 `core-service/Cargo.toml` 添加依賴**

在 `[dependencies]` 添加：

```toml
tokio-cron-scheduler = "0.9"
tokio = { workspace = true, features = ["sync"] }
```

**Step 2: 創建 `core-service/src/services/scheduler.rs`**

```rust
use tokio_cron_scheduler::JobScheduler;
use chrono::Utc;
use std::sync::Arc;

pub struct CronScheduler {
    scheduler: JobScheduler,
}

impl CronScheduler {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self { scheduler })
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.scheduler.start().await?;
        tracing::info!("Cron scheduler started");
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.scheduler.shutdown().await?;
        tracing::info!("Cron scheduler shutdown");
        Ok(())
    }

    pub async fn add_fetch_job(
        &self,
        subscription_id: String,
        fetcher_type: String,
        cron_expression: &str,
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use tokio_cron_scheduler::Job;

        let job = Job::new_async(cron_expression, move || {
            Box::pin(async {
                callback();
            })
        })?;

        self.scheduler.add(job).await?;

        tracing::info!(
            "Added fetch job for subscription {} (fetcher: {})",
            subscription_id,
            fetcher_type
        );

        Ok(())
    }
}
```

**Step 3: 修改 `core-service/src/services/mod.rs`**

```rust
pub mod registry;
pub mod filter;
pub mod scheduler;

pub use registry::ServiceRegistry;
pub use filter::FilterEngine;
pub use scheduler::CronScheduler;
```

**Step 4: 驗證編譯**

```bash
cargo check --package core-service
```

**Step 5: Commit**

```bash
git add core-service/src/services/scheduler.rs core-service/Cargo.toml
git commit -m "feat: Implement Cron scheduler for periodic tasks

- Create CronScheduler for managing scheduled jobs
- Support adding fetch jobs with cron expressions
- Integrate tokio-cron-scheduler"
```

---

## Phase 4: 核心服務的 REST API

### Task 11: 實現服務註冊 API 端點

**Files:**
- Modify: `core-service/src/handlers/services.rs`
- Modify: `core-service/src/main.rs`
- Create: `core-service/src/state.rs`

**Step 1: 創建 `core-service/src/state.rs`**

```rust
use crate::services::ServiceRegistry;
use crate::db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub registry: std::sync::Arc<ServiceRegistry>,
}
```

**Step 2: 修改 `core-service/src/main.rs`**

在 main.rs 頂部添加：

```rust
mod state;
use state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
```

修改 main 函數：

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ... 現有初始化代碼 ...

    let pool = db::establish_connection_pool(&database_url)?;
    db::run_migrations(&pool)?;

    let registry = std::sync::Arc::new(services::ServiceRegistry::new());
    let state = AppState {
        db: pool,
        registry,
    };

    let app = Router::new()
        .route("/services/register", post(handlers::services::register))
        .route("/services", get(handlers::services::list_services))
        .route("/services/:service_type", get(handlers::services::list_by_type))
        .route("/health", get(health_check))
        .with_state(state.clone());

    // ... 現有監聽代碼 ...
}
```

**Step 3: 完全重寫 `core-service/src/handlers/services.rs`**

```rust
use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use serde_json::json;
use shared::{ServiceRegistration, ServiceRegistrationResponse, ServiceType};
use uuid::Uuid;
use crate::state::AppState;

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<ServiceRegistration>,
) -> (StatusCode, Json<ServiceRegistrationResponse>) {
    let service_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let service = shared::RegisteredService {
        service_id,
        service_type: payload.service_type,
        service_name: payload.service_name,
        host: payload.host,
        port: payload.port,
        capabilities: payload.capabilities,
        is_healthy: true,
        last_heartbeat: now,
    };

    if let Err(e) = state.registry.register(service) {
        tracing::error!("Failed to register service: {}", e);
    }

    let response = ServiceRegistrationResponse {
        service_id,
        registered_at: now,
    };

    (StatusCode::CREATED, Json(response))
}

pub async fn list_services(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    match state.registry.get_services() {
        Ok(services) => Json(json!({ "services": services })),
        Err(e) => {
            tracing::error!("Failed to get services: {}", e);
            Json(json!({ "services": [] }))
        }
    }
}

pub async fn list_by_type(
    State(state): State<AppState>,
    Path(service_type): Path<String>,
) -> Json<serde_json::Value> {
    let service_type = match service_type.as_str() {
        "fetcher" => ServiceType::Fetcher,
        "downloader" => ServiceType::Downloader,
        "viewer" => ServiceType::Viewer,
        _ => {
            return Json(json!({"error": "Invalid service type", "services": []}))
        }
    };

    match state.registry.get_services_by_type(&service_type) {
        Ok(services) => Json(json!({ "services": services })),
        Err(e) => {
            tracing::error!("Failed to get services by type: {}", e);
            Json(json!({ "services": [] }))
        }
    }
}

pub async fn health_check(
    State(state): State<AppState>,
    Path(service_id): Path<Uuid>,
) -> (StatusCode, Json<serde_json::Value>) {
    state.registry.update_health(service_id, true).ok();
    (StatusCode::OK, Json(json!({"status": "ok"})))
}
```

**Step 4: 驗證編譯**

```bash
cargo check --package core-service
```

**Step 5: Commit**

```bash
git add core-service/src/
git commit -m "feat: Implement service registration REST API

- Create service registry state management
- Implement register, list, and health check endpoints
- Support filtering services by type
- Return proper HTTP status codes"
```

---

## 計劃結構總結

**已計劃的任務：**

### Phase 1: 數據庫 ✅
- Task 1-5: Diesel 設置與遷移
- Task 6: Schema 生成

### Phase 2: 數據庫訪問層 ✅
- Task 7-8: 服務註冊與 CRUD 操作

### Phase 3: 業務邏輯 ✅
- Task 9: 過濾引擎（包含測試）
- Task 10: Cron 調度

### Phase 4: REST API ✅
- Task 11: 服務註冊端點

### Phase 5-9: 待規劃
- Phase 5 (Task 12-16): 動畫管理 API
- Phase 6 (Task 17-22): 擷取區塊實現
- Phase 7 (Task 23-28): 下載區塊實現
- Phase 8 (Task 29-34): 顯示區塊實現
- Phase 9 (Task 35-45): CLI 工具實現
- Phase 10 (Task 46-55): 測試與優化

**總計 55+ 細粒度任務**

每個任務包含：
- 具體的文件操作
- 完整的代碼示例
- 測試命令與預期結果
- Git 提交信息

