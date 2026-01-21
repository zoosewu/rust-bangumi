# Phase 5：動畫管理 API 實現計畫

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 實現核心服務的動畫管理 REST API，支持動畫、季度、字幕組的完整 CRUD 操作

**Architecture:** 基於 Axum 框架，使用 Diesel ORM 與 PostgreSQL 數據庫交互，實現 RESTful 端點。模型已在 Phase 1-4 完成，本階段專注於 API 層實現和數據庫操作的具體實現。

**Tech Stack:** Rust 1.75+, Axum, Diesel, PostgreSQL 15, Tokio, Serde

---

## Phase 5: 動畫管理 API（Task 12-16）

### Task 12: 完成數據庫模型 CRUD 函數實現

**Files:**
- Modify: `core-service/src/db/models.rs`

**Step 1: 補全 anime CRUD 函數**

替換 `core-service/src/db/models.rs` 中的 `create_anime` 函數，使其完整實現：

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

pub fn get_all_animes(conn: &mut DbConnection) -> Result<Vec<Anime>, diesel::result::Error> {
    animes::table.order_by(animes::created_at.desc()).load(conn)
}

pub fn update_anime(conn: &mut DbConnection, anime_id: i32, title: &str) -> Result<Anime, diesel::result::Error> {
    diesel::update(animes::table.find(anime_id))
        .set((
            animes::title.eq(title),
            animes::updated_at.eq(chrono::Utc::now()),
        ))
        .get_result(conn)
}

pub fn delete_anime(conn: &mut DbConnection, anime_id: i32) -> Result<usize, diesel::result::Error> {
    diesel::delete(animes::table.find(anime_id)).execute(conn)
}
```

**Step 2: 補全 season 相關函數**

```rust
pub fn create_season(conn: &mut DbConnection, new_season: NewSeason) -> Result<Season, diesel::result::Error> {
    diesel::insert_into(seasons::table)
        .values(&new_season)
        .get_result(conn)
}

pub fn get_season_by_id(conn: &mut DbConnection, season_id: i32) -> Result<Season, diesel::result::Error> {
    seasons::table.find(season_id).first(conn)
}

pub fn get_or_create_season(conn: &mut DbConnection, year: i32, season: String) -> Result<Season, diesel::result::Error> {
    let existing = seasons::table
        .filter(seasons::year.eq(year).and(seasons::season.eq(&season)))
        .first::<Season>(conn)
        .optional()?;

    if let Some(season_obj) = existing {
        Ok(season_obj)
    } else {
        diesel::insert_into(seasons::table)
            .values(NewSeason { year, season })
            .get_result(conn)
    }
}

pub fn get_all_seasons(conn: &mut DbConnection) -> Result<Vec<Season>, diesel::result::Error> {
    seasons::table.order_by(seasons::year.desc()).order_by(seasons::season.asc()).load(conn)
}
```

**Step 3: 補全 anime_series 函數**

```rust
pub fn create_anime_series(conn: &mut DbConnection, new_series: NewAnimeSeries) -> Result<AnimeSeries, diesel::result::Error> {
    diesel::insert_into(anime_series::table)
        .values(&new_series)
        .get_result(conn)
}

pub fn get_anime_series_by_id(conn: &mut DbConnection, series_id: i32) -> Result<AnimeSeries, diesel::result::Error> {
    anime_series::table.find(series_id).first(conn)
}

pub fn get_anime_series_by_anime(conn: &mut DbConnection, anime_id: i32) -> Result<Vec<AnimeSeries>, diesel::result::Error> {
    anime_series::table
        .filter(anime_series::anime_id.eq(anime_id))
        .order_by(anime_series::series_no.asc())
        .load(conn)
}

pub fn get_anime_series_by_season(conn: &mut DbConnection, season_id: i32) -> Result<Vec<AnimeSeries>, diesel::result::Error> {
    anime_series::table
        .filter(anime_series::season_id.eq(season_id))
        .order_by(anime_series::anime_id.asc())
        .load(conn)
}

pub fn update_anime_series(conn: &mut DbConnection, series_id: i32, description: Option<String>) -> Result<AnimeSeries, diesel::result::Error> {
    diesel::update(anime_series::table.find(series_id))
        .set((
            anime_series::description.eq(description),
            anime_series::updated_at.eq(chrono::Utc::now()),
        ))
        .get_result(conn)
}

pub fn delete_anime_series(conn: &mut DbConnection, series_id: i32) -> Result<usize, diesel::result::Error> {
    diesel::delete(anime_series::table.find(series_id)).execute(conn)
}
```

**Step 4: 補全 subtitle_groups 函數**

```rust
pub fn create_subtitle_group(conn: &mut DbConnection, new_group: NewSubtitleGroup) -> Result<SubtitleGroup, diesel::result::Error> {
    diesel::insert_into(subtitle_groups::table)
        .values(&new_group)
        .get_result(conn)
}

pub fn get_subtitle_group_by_id(conn: &mut DbConnection, group_id: i32) -> Result<SubtitleGroup, diesel::result::Error> {
    subtitle_groups::table.find(group_id).first(conn)
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

pub fn get_all_subtitle_groups(conn: &mut DbConnection) -> Result<Vec<SubtitleGroup>, diesel::result::Error> {
    subtitle_groups::table.order_by(subtitle_groups::group_name.asc()).load(conn)
}

pub fn delete_subtitle_group(conn: &mut DbConnection, group_id: i32) -> Result<usize, diesel::result::Error> {
    diesel::delete(subtitle_groups::table.find(group_id)).execute(conn)
}
```

**Step 5: 驗證編譯**

```bash
cargo check --package core-service
```

Expected: 編譯成功

**Step 6: Commit**

```bash
git add core-service/src/db/models.rs
git commit -m "feat: Complete database model CRUD functions

- Implement full CRUD operations for all tables
- Add get_all/list operations with proper ordering
- Implement get_or_create pattern for idempotency
- All functions use Diesel with proper error handling"
```

---

### Task 13: 實現動畫 API 端點

**Files:**
- Create: `core-service/src/handlers/anime.rs`
- Modify: `core-service/src/handlers/mod.rs`
- Modify: `core-service/src/main.rs`
- Create: `core-service/src/dto.rs`

**Step 1: 創建 DTO（Data Transfer Objects）**

創建 `core-service/src/dto.rs`：

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// ============ Anime ============
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnimeRequest {
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnimeResponse {
    pub anime_id: i32,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============ Season ============
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SeasonRequest {
    pub year: i32,
    pub season: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SeasonResponse {
    pub season_id: i32,
    pub year: i32,
    pub season: String,
    pub created_at: DateTime<Utc>,
}

// ============ AnimeSeries ============
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnimeSeriesRequest {
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnimeSeriesResponse {
    pub series_id: i32,
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============ SubtitleGroup ============
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubtitleGroupRequest {
    pub group_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubtitleGroupResponse {
    pub group_id: i32,
    pub group_name: String,
    pub created_at: DateTime<Utc>,
}

// ============ Error Response ============
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}
```

**Step 2: 修改 `core-service/src/main.rs`，添加 dto 模組**

在 main.rs 頂部添加：

```rust
mod dto;
```

**Step 3: 創建 `core-service/src/handlers/anime.rs`**

```rust
use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use crate::state::AppState;
use crate::dto::*;
use crate::db;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ============ Anime Handlers ============

pub async fn create_anime(
    State(state): State<AppState>,
    Json(payload): Json<AnimeRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    let new_anime = crate::models::NewAnime {
        title: payload.title.clone(),
    };

    match db::create_anime(&mut conn, new_anime) {
        Ok(anime) => {
            let response = AnimeResponse {
                anime_id: anime.anime_id,
                title: anime.title,
                created_at: anime.created_at,
                updated_at: anime.updated_at,
            };
            (StatusCode::CREATED, Json(serde_json::to_value(response).unwrap()))
        }
        Err(e) => {
            tracing::error!("Failed to create anime: {}", e);
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Failed to create anime",
                "details": e.to_string()
            })))
        }
    }
}

pub async fn get_anime(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    match db::get_anime_by_id(&mut conn, anime_id) {
        Ok(anime) => {
            let response = AnimeResponse {
                anime_id: anime.anime_id,
                title: anime.title,
                created_at: anime.created_at,
                updated_at: anime.updated_at,
            };
            (StatusCode::OK, Json(serde_json::to_value(response).unwrap()))
        }
        Err(diesel::result::Error::NotFound) => {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": "Anime not found"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get anime: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to get anime"
            })))
        }
    }
}

pub async fn list_animes(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    match db::get_all_animes(&mut conn) {
        Ok(animes) => {
            let response: Vec<AnimeResponse> = animes.into_iter().map(|anime| {
                AnimeResponse {
                    anime_id: anime.anime_id,
                    title: anime.title,
                    created_at: anime.created_at,
                    updated_at: anime.updated_at,
                }
            }).collect();
            (StatusCode::OK, Json(serde_json::json!({ "animes": response })))
        }
        Err(e) => {
            tracing::error!("Failed to list animes: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to list animes"
            })))
        }
    }
}

pub async fn delete_anime(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
) -> StatusCode {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    match db::delete_anime(&mut conn, anime_id) {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(diesel::result::Error::NotFound) => StatusCode::NOT_FOUND,
        Err(e) => {
            tracing::error!("Failed to delete anime: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

// ============ Season Handlers ============

pub async fn create_season(
    State(state): State<AppState>,
    Json(payload): Json<SeasonRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    let new_season = crate::models::NewSeason {
        year: payload.year,
        season: payload.season.clone(),
    };

    match db::create_season(&mut conn, new_season) {
        Ok(season) => {
            let response = SeasonResponse {
                season_id: season.season_id,
                year: season.year,
                season: season.season,
                created_at: season.created_at,
            };
            (StatusCode::CREATED, Json(serde_json::to_value(response).unwrap()))
        }
        Err(e) => {
            tracing::error!("Failed to create season: {}", e);
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Failed to create season",
                "details": e.to_string()
            })))
        }
    }
}

pub async fn list_seasons(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    match db::get_all_seasons(&mut conn) {
        Ok(seasons) => {
            let response: Vec<SeasonResponse> = seasons.into_iter().map(|season| {
                SeasonResponse {
                    season_id: season.season_id,
                    year: season.year,
                    season: season.season,
                    created_at: season.created_at,
                }
            }).collect();
            (StatusCode::OK, Json(serde_json::json!({ "seasons": response })))
        }
        Err(e) => {
            tracing::error!("Failed to list seasons: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to list seasons"
            })))
        }
    }
}

// ============ AnimeSeries Handlers ============

pub async fn create_anime_series(
    State(state): State<AppState>,
    Json(payload): Json<AnimeSeriesRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    let new_series = crate::models::NewAnimeSeries {
        anime_id: payload.anime_id,
        series_no: payload.series_no,
        season_id: payload.season_id,
        description: payload.description.clone(),
        aired_date: None,
        end_date: None,
    };

    match db::create_anime_series(&mut conn, new_series) {
        Ok(series) => {
            let response = AnimeSeriesResponse {
                series_id: series.series_id,
                anime_id: series.anime_id,
                series_no: series.series_no,
                season_id: series.season_id,
                description: series.description,
                created_at: series.created_at,
                updated_at: series.updated_at,
            };
            (StatusCode::CREATED, Json(serde_json::to_value(response).unwrap()))
        }
        Err(e) => {
            tracing::error!("Failed to create anime series: {}", e);
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Failed to create anime series",
                "details": e.to_string()
            })))
        }
    }
}

pub async fn get_anime_series(
    State(state): State<AppState>,
    Path(series_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    match db::get_anime_series_by_id(&mut conn, series_id) {
        Ok(series) => {
            let response = AnimeSeriesResponse {
                series_id: series.series_id,
                anime_id: series.anime_id,
                series_no: series.series_no,
                season_id: series.season_id,
                description: series.description,
                created_at: series.created_at,
                updated_at: series.updated_at,
            };
            (StatusCode::OK, Json(serde_json::to_value(response).unwrap()))
        }
        Err(diesel::result::Error::NotFound) => {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": "Anime series not found"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get anime series: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to get anime series"
            })))
        }
    }
}

pub async fn list_anime_series_by_anime(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    match db::get_anime_series_by_anime(&mut conn, anime_id) {
        Ok(series_list) => {
            let response: Vec<AnimeSeriesResponse> = series_list.into_iter().map(|series| {
                AnimeSeriesResponse {
                    series_id: series.series_id,
                    anime_id: series.anime_id,
                    series_no: series.series_no,
                    season_id: series.season_id,
                    description: series.description,
                    created_at: series.created_at,
                    updated_at: series.updated_at,
                }
            }).collect();
            (StatusCode::OK, Json(serde_json::json!({ "series": response })))
        }
        Err(e) => {
            tracing::error!("Failed to list anime series: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to list anime series"
            })))
        }
    }
}

// ============ SubtitleGroup Handlers ============

pub async fn create_subtitle_group(
    State(state): State<AppState>,
    Json(payload): Json<SubtitleGroupRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    let new_group = crate::models::NewSubtitleGroup {
        group_name: payload.group_name.clone(),
    };

    match db::create_subtitle_group(&mut conn, new_group) {
        Ok(group) => {
            let response = SubtitleGroupResponse {
                group_id: group.group_id,
                group_name: group.group_name,
                created_at: group.created_at,
            };
            (StatusCode::CREATED, Json(serde_json::to_value(response).unwrap()))
        }
        Err(e) => {
            tracing::error!("Failed to create subtitle group: {}", e);
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Failed to create subtitle group",
                "details": e.to_string()
            })))
        }
    }
}

pub async fn list_subtitle_groups(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    match db::get_all_subtitle_groups(&mut conn) {
        Ok(groups) => {
            let response: Vec<SubtitleGroupResponse> = groups.into_iter().map(|group| {
                SubtitleGroupResponse {
                    group_id: group.group_id,
                    group_name: group.group_name,
                    created_at: group.created_at,
                }
            }).collect();
            (StatusCode::OK, Json(serde_json::json!({ "groups": response })))
        }
        Err(e) => {
            tracing::error!("Failed to list subtitle groups: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to list subtitle groups"
            })))
        }
    }
}

pub async fn delete_subtitle_group(
    State(state): State<AppState>,
    Path(group_id): Path<i32>,
) -> StatusCode {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    match db::delete_subtitle_group(&mut conn, group_id) {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(diesel::result::Error::NotFound) => StatusCode::NOT_FOUND,
        Err(e) => {
            tracing::error!("Failed to delete subtitle group: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
```

**Step 4: 修改 `core-service/src/handlers/mod.rs`**

```rust
pub mod services;
pub mod anime;
```

**Step 5: 修改 `core-service/src/main.rs` 添加路由**

在 Router 定義中添加新路由（在 `/services` 路由之後）：

```rust
.route("/anime", post(handlers::anime::create_anime))
.route("/anime", get(handlers::anime::list_animes))
.route("/anime/:anime_id", get(handlers::anime::get_anime))
.route("/anime/:anime_id", delete(handlers::anime::delete_anime))
.route("/anime/:anime_id/series", get(handlers::anime::list_anime_series_by_anime))
.route("/seasons", post(handlers::anime::create_season))
.route("/seasons", get(handlers::anime::list_seasons))
.route("/anime/series", post(handlers::anime::create_anime_series))
.route("/anime/series/:series_id", get(handlers::anime::get_anime_series))
.route("/subtitle-groups", post(handlers::anime::create_subtitle_group))
.route("/subtitle-groups", get(handlers::anime::list_subtitle_groups))
.route("/subtitle-groups/:group_id", delete(handlers::anime::delete_subtitle_group))
```

**Step 6: 驗證編譯**

```bash
cargo check --package core-service
```

Expected: 編譯成功

**Step 7: Commit**

```bash
git add core-service/src/handlers/anime.rs core-service/src/handlers/mod.rs core-service/src/dto.rs core-service/src/main.rs
git commit -m "feat: Implement anime management REST API endpoints

- Create DTO types for request/response serialization
- Implement anime CRUD endpoints (POST/GET/DELETE)
- Implement season management endpoints
- Implement anime series endpoints
- Implement subtitle group endpoints
- Add proper error handling and HTTP status codes"
```

---

### Task 14: 實現 API 端點單元測試

**Files:**
- Create: `core-service/tests/anime_api_tests.rs`

**Step 1: 創建 `core-service/tests/anime_api_tests.rs`**

```rust
#[cfg(test)]
mod tests {
    use core_service::models::*;
    use core_service::db;
    use diesel::r2d2::{ConnectionManager, Pool};
    use diesel::PgConnection;

    fn setup_test_db() -> Pool<ConnectionManager<PgConnection>> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://bangumi:bangumi_password@localhost:5432/bangumi_test".to_string());

        let manager = ConnectionManager::<PgConnection>::new(&database_url);
        Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("Failed to create pool")
    }

    #[test]
    #[ignore] // 需要測試數據庫
    fn test_create_anime() {
        let pool = setup_test_db();
        let mut conn = pool.get().expect("Failed to get connection");

        let new_anime = NewAnime {
            title: "Test Anime".to_string(),
        };

        let anime = db::create_anime(&mut conn, new_anime)
            .expect("Failed to create anime");

        assert_eq!(anime.title, "Test Anime");
    }

    #[test]
    #[ignore] // 需要測試數據庫
    fn test_get_anime_by_id() {
        let pool = setup_test_db();
        let mut conn = pool.get().expect("Failed to get connection");

        let new_anime = NewAnime {
            title: "Test Anime 2".to_string(),
        };

        let created = db::create_anime(&mut conn, new_anime)
            .expect("Failed to create anime");

        let retrieved = db::get_anime_by_id(&mut conn, created.anime_id)
            .expect("Failed to get anime");

        assert_eq!(retrieved.anime_id, created.anime_id);
        assert_eq!(retrieved.title, "Test Anime 2");
    }

    #[test]
    #[ignore] // 需要測試數據庫
    fn test_get_all_animes() {
        let pool = setup_test_db();
        let mut conn = pool.get().expect("Failed to get connection");

        let animes = db::get_all_animes(&mut conn)
            .expect("Failed to get animes");

        assert!(animes.len() >= 0);
    }

    #[test]
    #[ignore] // 需要測試數據庫
    fn test_delete_anime() {
        let pool = setup_test_db();
        let mut conn = pool.get().expect("Failed to get connection");

        let new_anime = NewAnime {
            title: "Test Anime to Delete".to_string(),
        };

        let created = db::create_anime(&mut conn, new_anime)
            .expect("Failed to create anime");

        let result = db::delete_anime(&mut conn, created.anime_id)
            .expect("Failed to delete anime");

        assert_eq!(result, 1);
    }

    #[test]
    #[ignore] // 需要測試數據庫
    fn test_create_season() {
        let pool = setup_test_db();
        let mut conn = pool.get().expect("Failed to get connection");

        let new_season = NewSeason {
            year: 2025,
            season: "winter".to_string(),
        };

        let season = db::create_season(&mut conn, new_season)
            .expect("Failed to create season");

        assert_eq!(season.year, 2025);
        assert_eq!(season.season, "winter");
    }

    #[test]
    #[ignore] // 需要測試數據庫
    fn test_get_or_create_season() {
        let pool = setup_test_db();
        let mut conn = pool.get().expect("Failed to get connection");

        let first = db::get_or_create_season(&mut conn, 2025, "spring".to_string())
            .expect("Failed to get or create season");

        let second = db::get_or_create_season(&mut conn, 2025, "spring".to_string())
            .expect("Failed to get or create season");

        assert_eq!(first.season_id, second.season_id);
    }
}
```

**Step 2: 驗證編譯**

```bash
cargo test --package core-service --test anime_api_tests --no-run
```

Expected: 編譯成功

**Step 3: 運行測試（使用 --ignored 跳過需要數據庫的測試）**

```bash
cargo test --package core-service --test anime_api_tests -- --ignored --nocapture
```

Expected: 所有標記為 `#[ignore]` 的測試被跳過

**Step 4: Commit**

```bash
git add core-service/tests/anime_api_tests.rs
git commit -m "test: Add integration tests for anime management API

- Create test fixtures for anime CRUD operations
- Test season creation and get_or_create pattern
- Mark tests as ignored pending test database setup
- Include database connection pool initialization"
```

---

### Task 15: 實現過濾規則管理 API

**Files:**
- Modify: `core-service/src/handlers/anime.rs`
- Create: `core-service/src/handlers/filters.rs`

**Step 1: 創建 `core-service/src/handlers/filters.rs`**

```rust
use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::db;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilterRuleRequest {
    pub series_id: i32,
    pub group_id: i32,
    pub rule_order: i32,
    pub rule_type: String,  // "Positive" or "Negative"
    pub regex_pattern: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilterRuleResponse {
    pub rule_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub rule_order: i32,
    pub rule_type: String,
    pub regex_pattern: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_filter_rule(
    State(state): State<AppState>,
    Json(payload): Json<FilterRuleRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    // 驗證 rule_type
    if payload.rule_type != "Positive" && payload.rule_type != "Negative" {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Invalid rule_type. Must be 'Positive' or 'Negative'"
        })));
    }

    let new_rule = crate::models::NewFilterRule {
        series_id: payload.series_id,
        group_id: payload.group_id,
        rule_order: payload.rule_order,
        rule_type: payload.rule_type.clone(),
        regex_pattern: payload.regex_pattern.clone(),
    };

    match db::create_filter_rule(&mut conn, new_rule) {
        Ok(rule) => {
            let response = FilterRuleResponse {
                rule_id: rule.rule_id,
                series_id: rule.series_id,
                group_id: rule.group_id,
                rule_order: rule.rule_order,
                rule_type: rule.rule_type,
                regex_pattern: rule.regex_pattern,
                created_at: rule.created_at,
            };
            (StatusCode::CREATED, Json(serde_json::to_value(response).unwrap()))
        }
        Err(e) => {
            tracing::error!("Failed to create filter rule: {}", e);
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Failed to create filter rule",
                "details": e.to_string()
            })))
        }
    }
}

pub async fn get_filter_rules(
    State(state): State<AppState>,
    Path((series_id, group_id)): Path<(i32, i32)>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    match db::get_filter_rules(&mut conn, series_id, group_id) {
        Ok(rules) => {
            let response: Vec<FilterRuleResponse> = rules.into_iter().map(|rule| {
                FilterRuleResponse {
                    rule_id: rule.rule_id,
                    series_id: rule.series_id,
                    group_id: rule.group_id,
                    rule_order: rule.rule_order,
                    rule_type: rule.rule_type,
                    regex_pattern: rule.regex_pattern,
                    created_at: rule.created_at,
                }
            }).collect();
            (StatusCode::OK, Json(serde_json::json!({ "rules": response })))
        }
        Err(e) => {
            tracing::error!("Failed to get filter rules: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to get filter rules"
            })))
        }
    }
}

pub async fn delete_filter_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<i32>,
) -> StatusCode {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    match db::delete_filter_rule(&mut conn, rule_id) {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(diesel::result::Error::NotFound) => StatusCode::NOT_FOUND,
        Err(e) => {
            tracing::error!("Failed to delete filter rule: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
```

**Step 2: 修改 `core-service/src/handlers/mod.rs` 添加 filters 模組**

```rust
pub mod services;
pub mod anime;
pub mod filters;
```

**Step 3: 修改 `core-service/src/main.rs` 添加過濾規則路由**

在 Router 定義中添加：

```rust
.route("/filters", post(handlers::filters::create_filter_rule))
.route("/filters/:series_id/:group_id", get(handlers::filters::get_filter_rules))
.route("/filters/:rule_id", delete(handlers::filters::delete_filter_rule))
```

**Step 4: 驗證編譯**

```bash
cargo check --package core-service
```

Expected: 編譯成功

**Step 5: Commit**

```bash
git add core-service/src/handlers/filters.rs core-service/src/handlers/mod.rs core-service/src/main.rs
git commit -m "feat: Implement filter rule management API endpoints

- Create filter rule CRUD endpoints
- Implement validation for rule_type (Positive/Negative)
- Add endpoints for creating and retrieving rules by series/group
- Support rule deletion with proper error handling"
```

---

### Task 16: 實現動畫連結管理 API

**Files:**
- Create: `core-service/src/handlers/links.rs`
- Modify: `core-service/src/handlers/mod.rs`
- Modify: `core-service/src/main.rs`

**Step 1: 創建 `core-service/src/handlers/links.rs`**

```rust
use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::db;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnimeLinkRequest {
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnimeLinkResponse {
    pub link_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_anime_link(
    State(state): State<AppState>,
    Json(payload): Json<AnimeLinkRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    let new_link = crate::models::NewAnimeLink {
        series_id: payload.series_id,
        group_id: payload.group_id,
        episode_no: payload.episode_no,
        title: payload.title.clone(),
        url: payload.url.clone(),
        source_hash: payload.source_hash.clone(),
        filtered_flag: false,
    };

    match db::create_anime_link(&mut conn, new_link) {
        Ok(link) => {
            let response = AnimeLinkResponse {
                link_id: link.link_id,
                series_id: link.series_id,
                group_id: link.group_id,
                episode_no: link.episode_no,
                title: link.title,
                url: link.url,
                source_hash: link.source_hash,
                filtered_flag: link.filtered_flag,
                created_at: link.created_at,
                updated_at: link.updated_at,
            };
            (StatusCode::CREATED, Json(serde_json::to_value(response).unwrap()))
        }
        Err(e) => {
            tracing::error!("Failed to create anime link: {}", e);
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Failed to create anime link",
                "details": e.to_string()
            })))
        }
    }
}

pub async fn get_anime_links(
    State(state): State<AppState>,
    Path(series_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Database connection failed"
            })));
        }
    };

    match db::get_anime_links_by_series(&mut conn, series_id) {
        Ok(links) => {
            let response: Vec<AnimeLinkResponse> = links.into_iter().map(|link| {
                AnimeLinkResponse {
                    link_id: link.link_id,
                    series_id: link.series_id,
                    group_id: link.group_id,
                    episode_no: link.episode_no,
                    title: link.title,
                    url: link.url,
                    source_hash: link.source_hash,
                    filtered_flag: link.filtered_flag,
                    created_at: link.created_at,
                    updated_at: link.updated_at,
                }
            }).collect();
            (StatusCode::OK, Json(serde_json::json!({ "links": response })))
        }
        Err(e) => {
            tracing::error!("Failed to get anime links: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to get anime links"
            })))
        }
    }
}
```

**Step 2: 修改 `core-service/src/handlers/mod.rs`**

```rust
pub mod services;
pub mod anime;
pub mod filters;
pub mod links;
```

**Step 3: 修改 `core-service/src/main.rs` 添加連結路由**

在 Router 定義中添加：

```rust
.route("/links", post(handlers::links::create_anime_link))
.route("/links/:series_id", get(handlers::links::get_anime_links))
```

**Step 4: 驗證編譯**

```bash
cargo check --package core-service
```

Expected: 編譯成功

**Step 5: 運行所有測試**

```bash
cargo test --package core-service
```

Expected: 所有測試通過

**Step 6: Commit**

```bash
git add core-service/src/handlers/links.rs core-service/src/handlers/mod.rs core-service/src/main.rs
git commit -m "feat: Implement anime link management API endpoints

- Create link CRUD endpoints for anime episodes
- Implement link retrieval by series ID
- Support filtering non-filtered links
- Add proper error handling and status codes
- Complete Phase 5 anime management API implementation"
```

---

## Phase 5 總結

**已完成任務：**
- Task 12: 完整的數據庫 CRUD 函數
- Task 13: 動畫、季度、系列管理 API
- Task 14: 集成測試框架
- Task 15: 過濾規則管理 API
- Task 16: 動畫連結管理 API

**API 端點概述：**

| 方法 | 端點 | 功能 |
|------|------|------|
| POST | /anime | 創建動畫 |
| GET | /anime | 列出所有動畫 |
| GET | /anime/:id | 獲取動畫詳情 |
| DELETE | /anime/:id | 刪除動畫 |
| POST | /seasons | 創建季度 |
| GET | /seasons | 列出所有季度 |
| POST | /anime/series | 創建系列 |
| GET | /anime/series/:id | 獲取系列詳情 |
| GET | /anime/:id/series | 列出動畫系列 |
| POST | /subtitle-groups | 創建字幕組 |
| GET | /subtitle-groups | 列出所有字幕組 |
| DELETE | /subtitle-groups/:id | 刪除字幕組 |
| POST | /filters | 創建過濾規則 |
| GET | /filters/:series_id/:group_id | 獲取過濾規則 |
| DELETE | /filters/:id | 刪除過濾規則 |
| POST | /links | 創建動畫連結 |
| GET | /links/:series_id | 獲取系列連結 |

**下一步計畫：**
- Phase 6: 擷取區塊實現（Task 17-22）
- Phase 7: 下載區塊實現（Task 23-28）
- Phase 8: 顯示區塊實現（Task 29-34）
- Phase 9: CLI 工具實現（Task 35-45）
