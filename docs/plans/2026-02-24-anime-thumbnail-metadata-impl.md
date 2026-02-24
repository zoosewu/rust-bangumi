# Anime Thumbnail Display & Metadata Service Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a new stateless Metadata Service for Bangumi.tv queries, add `anime_cover_images` to the DB, wire Core to auto-fetch covers on anime creation, update Viewer to call Metadata Service instead of Bangumi.tv directly, and add a thumbnail grid view with cover switching to the frontend.

**Architecture:** New `metadata/` Rust service (port 8004) queries Bangumi.tv; Core stores cover images in `anime_cover_images` and fires a `tokio::spawn` background fetch when creating an anime; Viewer replaces its own `bangumi_client.rs` with a `metadata_client.rs` that calls the new service; Frontend adds `AnimeSeriesCard` grid view + cover switcher in `AnimeDialog`.

**Tech Stack:** Rust (Axum 0.7, reqwest 0.12, tokio 1.37), PostgreSQL (Diesel 2.1), React (Effect.js, shadcn/ui, TypeScript)

---

### Task 1: Database Migration

**Files:**
- Create: `core-service/migrations/2026-02-24-000000-add-cover-images/up.sql`
- Create: `core-service/migrations/2026-02-24-000000-add-cover-images/down.sql`

**Step 1: Generate migration scaffolding**

```bash
cd /workspace/core-service
diesel migration generate add-cover-images
```

Expected: creates `migrations/2026-02-24-<timestamp>-add-cover-images/up.sql` and `down.sql`.
Rename the timestamp portion to `000000` for consistency: `2026-02-24-000000-add-cover-images`.

**Step 2: Write `up.sql`**

```sql
-- Add 'metadata' to the module_type enum
-- NOTE: PostgreSQL 12+ allows ALTER TYPE ADD VALUE in a transaction block
ALTER TYPE module_type ADD VALUE IF NOT EXISTS 'metadata';

-- Cover images table (one anime can have many, one is default)
CREATE TABLE anime_cover_images (
    cover_id          SERIAL PRIMARY KEY,
    anime_id          INTEGER NOT NULL REFERENCES animes(anime_id) ON DELETE CASCADE,
    image_url         TEXT NOT NULL,
    service_module_id INTEGER REFERENCES service_modules(module_id) ON DELETE SET NULL,
    source_name       VARCHAR(100) NOT NULL,
    is_default        BOOLEAN NOT NULL DEFAULT FALSE,
    created_at        TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(anime_id, image_url)
);

CREATE INDEX idx_anime_cover_images_anime_id ON anime_cover_images(anime_id);
CREATE INDEX idx_anime_cover_images_default
    ON anime_cover_images(anime_id) WHERE is_default = TRUE;
```

**Step 3: Write `down.sql`**

```sql
DROP TABLE IF EXISTS anime_cover_images;
-- Note: PostgreSQL cannot remove enum values; 'metadata' remains in module_type
```

**Step 4: Run migration**

```bash
cd /workspace/core-service
diesel migration run
```

Expected output: `Running migration 2026-02-24-000000-add-cover-images`

**Step 5: Verify**

```bash
psql $DATABASE_URL -c "\d anime_cover_images"
psql $DATABASE_URL -c "SELECT enum_range(NULL::module_type)"
```

Expected: table shows 7 columns; enum includes `metadata`.

**Step 6: Commit**

```bash
git add core-service/migrations/
git commit -m "feat(db): add anime_cover_images table and metadata module_type enum value"
```

---

### Task 2: Shared Models — ServiceType + ViewerSyncRequest

**Files:**
- Modify: `shared/src/models.rs`

**Step 1: Read current file**

Read `shared/src/models.rs`. Find `ServiceType` enum and `ViewerSyncRequest` struct.

**Step 2: Add `Metadata` to `ServiceType`**

Find the `ServiceType` enum (likely near the top of the file) and add `Metadata`:

```rust
pub enum ServiceType {
    Fetcher,
    Downloader,
    Viewer,
    Metadata,  // NEW
}
```

**Step 3: Update `ViewerSyncRequest` — add `bangumi_id` and `cover_image_url`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerSyncRequest {
    pub download_id: i32,
    pub series_id: i32,
    pub anime_title: String,
    pub series_no: i32,
    pub episode_no: i32,
    pub subtitle_group: String,
    pub file_path: String,
    pub callback_url: String,
    pub bangumi_id: Option<i32>,          // NEW — from anime_cover_images
    pub cover_image_url: Option<String>,  // NEW — default cover URL for poster download
}
```

**Step 4: Build shared crate**

```bash
cd /workspace
cargo build -p shared
```

Expected: compiles. If `ServiceType` has a `match` elsewhere, add the `Metadata` arm.

**Step 5: Fix compilation errors in callers**

```bash
grep -r "ViewerSyncRequest {" /workspace --include="*.rs" -l
```

For each file, add `bangumi_id: None, cover_image_url: None` to existing struct literals.

**Step 6: Commit**

```bash
git add shared/src/models.rs
git commit -m "feat(shared): add Metadata ServiceType and bangumi_id/cover_image_url to ViewerSyncRequest"
```

---

### Task 3: Core — Update ModuleTypeEnum + Add AnimeCoverImage Model

**Files:**
- Modify: `core-service/src/models/db.rs`
- Modify: `core-service/src/schema.rs` (regenerate)

**Step 1: Regenerate schema.rs**

```bash
cd /workspace/core-service
diesel print-schema > src/schema.rs
```

Expected: `anime_cover_images` table and updated `sql_types::ModuleType` appear in the file.

**Step 2: Verify the generated schema**

Read `core-service/src/schema.rs`. Confirm:
- `anime_cover_images (cover_id)` table exists with the correct columns
- `joinable!(anime_cover_images -> animes (anime_id));` is present
- `joinable!(anime_cover_images -> service_modules (service_module_id));` is present
- `anime_cover_images` is in `allow_tables_to_appear_in_same_query!`

If any `joinable!` or `allow_tables_to_appear_in_same_query!` entries are missing, add them manually.

**Step 3: Update `ModuleTypeEnum` in `db.rs`**

Find the existing `ModuleTypeEnum` (which uses manual `FromSql`/`ToSql` — NOT `#[derive(DbEnum)]`).
Add `Metadata` variant and update all three `match` blocks:

```rust
pub enum ModuleTypeEnum {
    Fetcher,
    Downloader,
    Viewer,
    Metadata,  // NEW
}

// In FromSql impl — add:
b"metadata" => Ok(ModuleTypeEnum::Metadata),

// In ToSql impl — add:
ModuleTypeEnum::Metadata => out.write_all(b"metadata")?,

// In Display impl — add:
ModuleTypeEnum::Metadata => write!(f, "metadata"),
```

**Step 4: Update `From<&shared::ServiceType>` conversion**

```rust
impl From<&shared::ServiceType> for ModuleTypeEnum {
    fn from(service_type: &shared::ServiceType) -> Self {
        match service_type {
            shared::ServiceType::Fetcher => ModuleTypeEnum::Fetcher,
            shared::ServiceType::Downloader => ModuleTypeEnum::Downloader,
            shared::ServiceType::Viewer => ModuleTypeEnum::Viewer,
            shared::ServiceType::Metadata => ModuleTypeEnum::Metadata,  // NEW
        }
    }
}
```

**Step 5: Add `AnimeCoverImage` and `NewAnimeCoverImage` models**

Add to `core-service/src/models/db.rs`:

```rust
#[derive(Debug, Queryable, Selectable, Serialize, Clone)]
#[diesel(table_name = crate::schema::anime_cover_images)]
pub struct AnimeCoverImage {
    pub cover_id: i32,
    pub anime_id: i32,
    pub image_url: String,
    pub service_module_id: Option<i32>,
    pub source_name: String,
    pub is_default: bool,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::anime_cover_images)]
pub struct NewAnimeCoverImage {
    pub anime_id: i32,
    pub image_url: String,
    pub service_module_id: Option<i32>,
    pub source_name: String,
    pub is_default: bool,
    pub created_at: chrono::NaiveDateTime,
}
```

**Step 6: Compile**

```bash
cargo build -p core-service
```

Fix any `match` non-exhaustive errors for `ModuleTypeEnum`.

**Step 7: Commit**

```bash
git add core-service/src/schema.rs core-service/src/models/db.rs
git commit -m "feat(core): add AnimeCoverImage model and Metadata variant to ModuleTypeEnum"
```

---

### Task 4: Bootstrap Metadata Service

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `metadata/Cargo.toml`
- Create: `metadata/src/main.rs`
- Create: `metadata/src/models.rs`
- Create: `metadata/src/handlers.rs`
- Create: `metadata/src/bangumi_client.rs`

**Step 1: Add `metadata` to workspace**

In `/workspace/Cargo.toml`, find `members` and add:

```toml
[workspace]
members = [
    "shared",
    "core-service",
    "fetchers/mikanani",
    "downloaders/qbittorrent",
    "viewers/jellyfin",
    "cli",
    "metadata",   # ADD THIS
]
```

**Step 2: Create directory**

```bash
mkdir -p /workspace/metadata/src
```

**Step 3: Create `metadata/Cargo.toml`**

```toml
[package]
name = "metadata-service"
version.workspace = true
edition.workspace = true

[[bin]]
name = "metadata-service"
path = "src/main.rs"

[dependencies]
shared = { path = "../shared" }
tokio.workspace = true
axum.workspace = true
reqwest = { workspace = true, features = ["json"] }
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
urlencoding = "2"
```

**Step 4: Create `metadata/src/models.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct EnrichAnimeRequest {
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct CoverImageInfo {
    pub url: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct EnrichAnimeResponse {
    pub bangumi_id: Option<i32>,
    pub cover_images: Vec<CoverImageInfo>,
    pub summary: Option<String>,
    pub air_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EnrichEpisodesRequest {
    pub bangumi_id: i32,
    pub episode_no: i32,
}

#[derive(Debug, Serialize)]
pub struct EnrichEpisodesResponse {
    pub episode_no: i32,
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<String>,
    pub summary: Option<String>,
}
```

**Step 5: Create stub `metadata/src/bangumi_client.rs`**

```rust
pub struct BangumiClient {
    http: reqwest::Client,
}

impl BangumiClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent("anime-manager/0.1")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to build HTTP client");
        Self { http }
    }
}
```

**Step 6: Create stub `metadata/src/handlers.rs`**

```rust
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;
use crate::bangumi_client::BangumiClient;

#[derive(Clone)]
pub struct AppState {
    pub bangumi: Arc<BangumiClient>,
}

pub async fn health() -> StatusCode {
    StatusCode::OK
}

pub async fn enrich_anime(
    State(_state): State<AppState>,
    Json(_req): Json<crate::models::EnrichAnimeRequest>,
) -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn enrich_episodes(
    State(_state): State<AppState>,
    Json(_req): Json<crate::models::EnrichEpisodesRequest>,
) -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}
```

**Step 7: Create `metadata/src/main.rs`**

```rust
mod bangumi_client;
mod handlers;
mod models;

use axum::{routing::{get, post}, Router};
use handlers::AppState;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let bangumi = Arc::new(bangumi_client::BangumiClient::new());
    let state = AppState { bangumi };

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/enrich/anime", post(handlers::enrich_anime))
        .route("/enrich/episodes", post(handlers::enrich_episodes))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8004".to_string());
    let addr = format!("0.0.0.0:{}", port);
    info!("Metadata service listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

**Step 8: Verify compilation**

```bash
cd /workspace
cargo build -p metadata-service
```

Expected: compiles with stub NOT_IMPLEMENTED handlers.

**Step 9: Commit**

```bash
git add metadata/ Cargo.toml
git commit -m "feat(metadata): bootstrap metadata service skeleton"
```

---

### Task 5: Metadata Service — Bangumi Client Implementation

**Files:**
- Modify: `metadata/src/bangumi_client.rs`

**Step 1: Write unit tests first**

Add to `metadata/src/bangumi_client.rs` (above the impl):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests — require network access
    // Run: cargo test -p metadata-service -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_search_anime_finds_attack_on_titan() {
        let client = BangumiClient::new();
        let result = client.search_anime("進撃の巨人").await;
        assert!(result.is_ok(), "search failed: {:?}", result.err());
        let id = result.unwrap();
        assert!(id.is_some(), "expected Some(id), got None");
        println!("bangumi_id = {:?}", id);
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_cover_images_returns_url() {
        let client = BangumiClient::new();
        // Attack on Titan: bangumi id 101 or similar — use the result from search test
        let result = client.get_cover_images(101).await;
        assert!(result.is_ok());
        let images = result.unwrap();
        assert!(!images.is_empty(), "expected at least one cover image");
        assert!(images[0].url.starts_with("https://"));
    }
}
```

**Step 2: Run to verify tests fail**

```bash
cargo test -p metadata-service -- --ignored 2>&1 | head -20
```

Expected: compile error `get_cover_images not found`.

**Step 3: Implement `BangumiClient`**

Replace the stub in `metadata/src/bangumi_client.rs`:

```rust
use anyhow::{anyhow, Result};
use crate::models::CoverImageInfo;

const BANGUMI_API_BASE: &str = "https://api.bgm.tv";

pub struct BangumiClient {
    http: reqwest::Client,
}

impl BangumiClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent("anime-manager/0.1 (github.com/yourrepo)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to build HTTP client");
        Self { http }
    }

    /// Search by title; return the first matching Bangumi subject_id.
    pub async fn search_anime(&self, title: &str) -> Result<Option<i32>> {
        let url = format!(
            "{}/search/subject/{}?type=2&responseGroup=small",
            BANGUMI_API_BASE,
            urlencoding::encode(title)
        );
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let body: serde_json::Value = resp.json().await?;
        let id = body["list"][0]["id"].as_i64().map(|v| v as i32);
        Ok(id)
    }

    /// Get cover image URLs for a subject.
    pub async fn get_cover_images(&self, bangumi_id: i32) -> Result<Vec<CoverImageInfo>> {
        let url = format!("{}/v0/subjects/{}", BANGUMI_API_BASE, bangumi_id);
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }
        let body: serde_json::Value = resp.json().await?;
        let mut images = vec![];
        if let Some(large) = body["images"]["large"].as_str() {
            if !large.is_empty() && !large.ends_with("no_img.gif") {
                images.push(CoverImageInfo {
                    url: large.to_string(),
                    source: "bangumi".to_string(),
                });
            }
        }
        Ok(images)
    }

    /// Get summary and air_date for a subject.
    pub async fn get_subject_meta(&self, bangumi_id: i32) -> Result<SubjectMeta> {
        let url = format!("{}/v0/subjects/{}", BANGUMI_API_BASE, bangumi_id);
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Bangumi subject {} returned {}", bangumi_id, resp.status()));
        }
        let body: serde_json::Value = resp.json().await?;
        Ok(SubjectMeta {
            summary: body["summary"].as_str().map(|s| s.to_string()),
            air_date: body["date"].as_str().map(|s| s.to_string()),
        })
    }

    /// Get episode metadata for a specific episode number.
    pub async fn get_episode(
        &self,
        bangumi_id: i32,
        episode_no: i32,
    ) -> Result<Option<EpisodeMeta>> {
        let url = format!("{}/v0/episodes", BANGUMI_API_BASE);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("subject_id", bangumi_id.to_string()),
                ("type", "0".to_string()),
                ("limit", "100".to_string()),
            ])
            .send()
            .await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let body: serde_json::Value = resp.json().await?;
        let eps = body["data"].as_array().cloned().unwrap_or_default();
        let ep = eps
            .iter()
            .find(|e| e["ep"].as_i64().map(|n| n as i32) == Some(episode_no));
        Ok(ep.map(|e| EpisodeMeta {
            episode_no,
            title: e["name"].as_str().map(|s| s.to_string()),
            title_cn: e["name_cn"].as_str().map(|s| s.to_string()),
            air_date: e["airdate"].as_str().map(|s| s.to_string()),
            summary: e["desc"].as_str().map(|s| s.to_string()),
        }))
    }
}

pub struct SubjectMeta {
    pub summary: Option<String>,
    pub air_date: Option<String>,
}

pub struct EpisodeMeta {
    pub episode_no: i32,
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<String>,
    pub summary: Option<String>,
}
```

**Step 4: Run tests**

```bash
cargo test -p metadata-service -- --ignored 2>&1
```

Expected: `test_search_anime_finds_attack_on_titan ... ok` (if network available).

**Step 5: Build check**

```bash
cargo build -p metadata-service
```

**Step 6: Commit**

```bash
git add metadata/src/bangumi_client.rs
git commit -m "feat(metadata): implement BangumiClient with search, cover, and episode methods"
```

---

### Task 6: Metadata Service — Handler Implementations

**Files:**
- Modify: `metadata/src/handlers.rs`

**Step 1: Write handler tests**

Add to the bottom of `metadata/src/handlers.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::{Request, StatusCode}};
    use tower::ServiceExt;

    fn test_app() -> Router<AppState> {
        let state = AppState {
            bangumi: std::sync::Arc::new(crate::bangumi_client::BangumiClient::new()),
        };
        Router::new()
            .route("/enrich/anime", post(enrich_anime))
            .route("/enrich/episodes", post(enrich_episodes))
            .with_state(state)
    }

    #[tokio::test]
    #[ignore]
    async fn test_enrich_anime_returns_bangumi_id() {
        let app = test_app();
        let body = serde_json::json!({ "title": "進撃の巨人" });
        let req = Request::builder()
            .method("POST")
            .uri("/enrich/anime")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(result["bangumi_id"].as_i64().is_some(), "expected bangumi_id");
        assert!(!result["cover_images"].as_array().unwrap().is_empty(), "expected cover images");
    }

    #[tokio::test]
    async fn test_enrich_anime_with_unknown_title_returns_empty() {
        let app = test_app();
        let body = serde_json::json!({ "title": "xyzzy_nonexistent_anime_12345" });
        let req = Request::builder()
            .method("POST")
            .uri("/enrich/anime")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        // Should return 200 with empty results, not 500
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
```

**Step 2: Run to verify tests fail**

```bash
cargo test -p metadata-service test_enrich 2>&1 | head -30
```

Expected: fails because handlers still return NOT_IMPLEMENTED.

**Step 3: Implement `enrich_anime` handler**

Replace stub in `handlers.rs`:

```rust
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;
use crate::{
    bangumi_client::BangumiClient,
    models::{
        CoverImageInfo, EnrichAnimeRequest, EnrichAnimeResponse,
        EnrichEpisodesRequest, EnrichEpisodesResponse,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub bangumi: Arc<BangumiClient>,
}

pub async fn health() -> StatusCode {
    StatusCode::OK
}

pub async fn enrich_anime(
    State(state): State<AppState>,
    Json(req): Json<EnrichAnimeRequest>,
) -> impl IntoResponse {
    let bangumi_id = match state.bangumi.search_anime(&req.title).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Json(EnrichAnimeResponse {
                bangumi_id: None,
                cover_images: vec![],
                summary: None,
                air_date: None,
            })
            .into_response();
        }
        Err(e) => {
            tracing::warn!("Bangumi search failed for '{}': {}", req.title, e);
            return Json(EnrichAnimeResponse {
                bangumi_id: None,
                cover_images: vec![],
                summary: None,
                air_date: None,
            })
            .into_response();
        }
    };

    let cover_images = state
        .bangumi
        .get_cover_images(bangumi_id)
        .await
        .unwrap_or_default();

    let meta = state
        .bangumi
        .get_subject_meta(bangumi_id)
        .await
        .unwrap_or(crate::bangumi_client::SubjectMeta {
            summary: None,
            air_date: None,
        });

    Json(EnrichAnimeResponse {
        bangumi_id: Some(bangumi_id),
        cover_images,
        summary: meta.summary,
        air_date: meta.air_date,
    })
    .into_response()
}

pub async fn enrich_episodes(
    State(state): State<AppState>,
    Json(req): Json<EnrichEpisodesRequest>,
) -> impl IntoResponse {
    match state.bangumi.get_episode(req.bangumi_id, req.episode_no).await {
        Ok(Some(ep)) => Json(EnrichEpisodesResponse {
            episode_no: ep.episode_no,
            title: ep.title,
            title_cn: ep.title_cn,
            air_date: ep.air_date,
            summary: ep.summary,
        })
        .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("get_episode failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
```

**Step 4: Run tests**

```bash
cargo test -p metadata-service test_enrich_anime_with_unknown 2>&1
cargo test -p metadata-service test_enrich_anime_returns -- --ignored 2>&1
```

Expected: both pass (second requires network).

**Step 5: Commit**

```bash
git add metadata/src/handlers.rs
git commit -m "feat(metadata): implement enrich_anime and enrich_episodes handlers"
```

---

### Task 7: Metadata Service — Service Registration on Startup

**Files:**
- Modify: `metadata/src/main.rs`

**Step 1: Read `viewers/jellyfin/src/main.rs`**

Skim the file to confirm the exact structure of `shared::ServiceRegistration` and `register_to_core`. The Metadata Service will follow the same pattern.

**Step 2: Implement registration in `metadata/src/main.rs`**

Add a `register_with_core` function and call it after server startup:

```rust
mod bangumi_client;
mod handlers;
mod models;

use axum::{routing::{get, post}, Router};
use handlers::AppState;
use std::sync::Arc;
use tracing::info;

async fn register_with_core(core_url: &str, host: &str, port: u16) {
    let registration = shared::ServiceRegistration {
        service_type: shared::ServiceType::Metadata,
        service_name: "bangumi".to_string(),
        host: host.to_string(),
        port,
        capabilities: shared::Capabilities {
            fetch_endpoint: None,
            download_endpoint: None,
            sync_endpoint: None,
            supported_download_types: vec![],
        },
    };
    let client = reqwest::Client::new();
    for attempt in 1..=5u32 {
        match client
            .post(format!("{}/services/register", core_url))
            .json(&registration)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                info!("Registered with Core successfully");
                return;
            }
            Ok(r) => tracing::warn!("Registration attempt {}: HTTP {}", attempt, r.status()),
            Err(e) => tracing::warn!("Registration attempt {} failed: {}", attempt, e),
        }
        if attempt < 5 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
    tracing::error!("Failed to register with Core after 5 attempts");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8004".to_string())
        .parse()
        .unwrap_or(8004);
    let service_host =
        std::env::var("SERVICE_HOST").unwrap_or_else(|_| "metadata".to_string());
    let core_url = std::env::var("CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://core-service:8000".to_string());

    let bangumi = Arc::new(bangumi_client::BangumiClient::new());
    let state = AppState { bangumi };

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/enrich/anime", post(handlers::enrich_anime))
        .route("/enrich/episodes", post(handlers::enrich_episodes))
        .with_state(state);

    // Spawn registration after a brief delay (Core may not be ready yet)
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        register_with_core(&core_url, &service_host, port).await;
    });

    let addr = format!("0.0.0.0:{}", port);
    info!("Metadata service listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

**Step 3: Build**

```bash
cargo build -p metadata-service
```

**Step 4: Commit**

```bash
git add metadata/src/main.rs
git commit -m "feat(metadata): add auto-registration with Core on startup"
```

---

### Task 8: Core — Cover Image API Handlers

**Files:**
- Create: `core-service/src/handlers/covers.rs`
- Modify: `core-service/src/handlers/mod.rs`
- Modify: `core-service/src/main.rs`

**Step 1: Write tests**

Create `core-service/tests/covers_api_tests.rs`:

```rust
// Integration tests — require DATABASE_URL env var
// Run: cargo test -p core-service covers -- --ignored

#[cfg(test)]
mod covers_api_tests {
    use diesel::prelude::*;

    fn get_test_conn() -> diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>> {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
        let manager = diesel::r2d2::ConnectionManager::<diesel::PgConnection>::new(url);
        diesel::r2d2::Pool::builder().build(manager).unwrap().get().unwrap()
    }

    #[test]
    #[ignore]
    fn test_insert_and_list_cover_images() {
        use core_service::schema::anime_cover_images;
        use core_service::models::db::NewAnimeCoverImage;

        let mut conn = get_test_conn();
        // Assumes anime_id=1 exists
        let new_cover = NewAnimeCoverImage {
            anime_id: 1,
            image_url: "https://example.com/test.jpg".to_string(),
            service_module_id: None,
            source_name: "test".to_string(),
            is_default: true,
            created_at: chrono::Utc::now().naive_utc(),
        };
        diesel::insert_into(anime_cover_images::table)
            .values(&new_cover)
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .unwrap();

        let covers = anime_cover_images::table
            .filter(anime_cover_images::anime_id.eq(1))
            .load::<core_service::models::db::AnimeCoverImage>(&mut conn)
            .unwrap();
        assert!(!covers.is_empty());
    }
}
```

**Step 2: Implement `core-service/src/handlers/covers.rs`**

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use crate::{models::db::AnimeCoverImage, schema::anime_cover_images, AppState};

pub async fn list_anime_covers(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
) -> impl IntoResponse {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match anime_cover_images::table
        .filter(anime_cover_images::anime_id.eq(anime_id))
        .order(anime_cover_images::created_at.asc())
        .load::<AnimeCoverImage>(&mut conn)
    {
        Ok(list) => Json(list).into_response(),
        Err(e) => {
            tracing::error!("list_anime_covers failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn set_default_cover(
    State(state): State<AppState>,
    Path((anime_id, cover_id)): Path<(i32, i32)>,
) -> impl IntoResponse {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let result = conn.transaction::<_, diesel::result::Error, _>(|conn| {
        // Clear all defaults for this anime
        diesel::update(
            anime_cover_images::table.filter(anime_cover_images::anime_id.eq(anime_id)),
        )
        .set(anime_cover_images::is_default.eq(false))
        .execute(conn)?;
        // Set the requested one as default
        let updated = diesel::update(
            anime_cover_images::table
                .filter(anime_cover_images::cover_id.eq(cover_id))
                .filter(anime_cover_images::anime_id.eq(anime_id)),
        )
        .set(anime_cover_images::is_default.eq(true))
        .execute(conn)?;
        if updated == 0 {
            return Err(diesel::result::Error::NotFound);
        }
        Ok(())
    });
    match result {
        Ok(_) => StatusCode::OK.into_response(),
        Err(diesel::result::Error::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("set_default_cover failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
```

**Step 3: Register module and routes**

In `core-service/src/handlers/mod.rs`, add:

```rust
pub mod covers;
```

In `core-service/src/main.rs`, add to the router (after existing anime routes):

```rust
.route("/anime/:anime_id/covers",
    get(handlers::covers::list_anime_covers))
.route("/anime/:anime_id/covers/:cover_id/set-default",
    post(handlers::covers::set_default_cover))
```

**Step 4: Compile**

```bash
cargo build -p core-service
```

**Step 5: Commit**

```bash
git add core-service/src/handlers/covers.rs \
        core-service/src/handlers/mod.rs \
        core-service/src/main.rs \
        core-service/tests/covers_api_tests.rs
git commit -m "feat(core): add GET /anime/:id/covers and POST .../set-default endpoints"
```

---

### Task 9: Core — Background Metadata Fetch on Anime Create

**Files:**
- Modify: `core-service/src/handlers/anime.rs`

**Step 1: Read `create_anime` handler**

Read the current `create_anime` implementation. It calls `state.repos.anime.create(payload.title)`. Note what `AppState` fields are available (particularly `state.db`).

**Step 2: Add `fetch_and_store_covers` function**

Add this helper at the bottom of `core-service/src/handlers/anime.rs`:

```rust
pub async fn fetch_and_store_covers(db: crate::db::DbPool, anime_id: i32, anime_title: String) {
    use crate::schema::{anime_cover_images, service_modules};
    use crate::models::db::{ModuleTypeEnum, NewAnimeCoverImage};

    // 1. Find the metadata service base_url
    let (metadata_url, module_id) = {
        let mut conn = match db.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("DB pool error in metadata fetch: {}", e);
                return;
            }
        };
        match service_modules::table
            .filter(service_modules::module_type.eq(ModuleTypeEnum::Metadata))
            .filter(service_modules::is_enabled.eq(true))
            .select((service_modules::base_url, service_modules::module_id))
            .first::<(String, i32)>(&mut conn)
            .optional()
        {
            Ok(Some((url, id))) => (url, id),
            Ok(None) => {
                tracing::debug!("No metadata service registered — skipping cover fetch");
                return;
            }
            Err(e) => {
                tracing::error!("DB query error: {}", e);
                return;
            }
        }
    };

    // 2. Call metadata service
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/enrich/anime", metadata_url))
        .json(&serde_json::json!({ "title": anime_title }))
        .send()
        .await;

    let data: serde_json::Value = match resp {
        Ok(r) if r.status().is_success() => match r.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Failed to parse metadata response: {}", e);
                return;
            }
        },
        Ok(r) => {
            tracing::warn!("Metadata service returned HTTP {}", r.status());
            return;
        }
        Err(e) => {
            tracing::error!("Metadata service unreachable: {}", e);
            return;
        }
    };

    // 3. Insert cover images
    let cover_images = data["cover_images"].as_array().cloned().unwrap_or_default();
    if cover_images.is_empty() {
        return;
    }

    let mut conn = match db.get() {
        Ok(c) => c,
        Err(_) => return,
    };

    for (idx, img) in cover_images.iter().enumerate() {
        let url = match img["url"].as_str() {
            Some(u) => u.to_string(),
            None => continue,
        };
        let source = img["source"].as_str().unwrap_or("bangumi").to_string();
        let new_cover = NewAnimeCoverImage {
            anime_id,
            image_url: url,
            service_module_id: Some(module_id),
            source_name: source,
            is_default: idx == 0,
            created_at: chrono::Utc::now().naive_utc(),
        };
        let _ = diesel::insert_into(anime_cover_images::table)
            .values(&new_cover)
            .on_conflict_do_nothing()
            .execute(&mut conn);
    }
    tracing::info!(
        "Stored {} cover images for anime_id={}",
        cover_images.len(),
        anime_id
    );
}
```

**Step 3: Call from `create_anime`**

Inside `create_anime`, after the successful `Ok(anime)` branch, before returning:

```rust
Ok(anime) => {
    tracing::info!("Created anime: {}", anime.anime_id);

    // Spawn background cover image fetch
    let db_clone = state.db.clone();
    let title_clone = anime.title.clone();
    let aid = anime.anime_id;
    tokio::spawn(async move {
        fetch_and_store_covers(db_clone, aid, title_clone).await;
    });

    let response = AnimeResponse { /* ... existing ... */ };
    (StatusCode::CREATED, Json(json!(response)))
}
```

**Step 4: Compile**

```bash
cargo build -p core-service
```

If `state.db` is not directly available in `create_anime` (which uses `state.repos`), check how `AppState` is defined and find the pool field. It may be `state.db` or `state.repos.pool()` — read `AppState` definition carefully.

**Step 5: Commit**

```bash
git add core-service/src/handlers/anime.rs
git commit -m "feat(core): spawn background Metadata Service fetch when creating anime"
```

---

### Task 10: Core — Include `cover_image_url` in AnimeSeriesRich

**Files:**
- Modify: `core-service/src/dto.rs`
- Modify: `core-service/src/handlers/anime.rs` (`list_all_anime_series`)

**Step 1: Add field to DTO**

In `core-service/src/dto.rs`, find `AnimeSeriesRichResponse` and add:

```rust
pub struct AnimeSeriesRichResponse {
    // ... all existing fields ...
    pub cover_image_url: Option<String>,   // NEW
}
```

**Step 2: Pre-fetch all default cover images**

In `list_all_anime_series`, before the main loop that builds results, add a bulk cover image fetch:

```rust
// Fetch all default cover images in one query
let cover_map: std::collections::HashMap<i32, String> = {
    use crate::schema::anime_cover_images;
    match anime_cover_images::table
        .filter(anime_cover_images::is_default.eq(true))
        .select((anime_cover_images::anime_id, anime_cover_images::image_url))
        .load::<(i32, String)>(&mut conn)
    {
        Ok(rows) => rows.into_iter().collect(),
        Err(_) => std::collections::HashMap::new(),
    }
};
```

**Step 3: Use `cover_map` in the loop**

In the `results.push(AnimeSeriesRichResponse { ... })` call, add:

```rust
cover_image_url: cover_map.get(&series.anime_id).cloned(),
```

**Step 4: Compile**

```bash
cargo build -p core-service
```

Fix any `missing field` errors in test files or other code that constructs `AnimeSeriesRichResponse`.

**Step 5: Commit**

```bash
git add core-service/src/dto.rs core-service/src/handlers/anime.rs
git commit -m "feat(core): include cover_image_url in AnimeSeriesRich response"
```

---

### Task 11: Viewer — Replace Direct Bangumi Calls with Metadata Client

**Files:**
- Create: `viewers/jellyfin/src/metadata_client.rs`
- Modify: `viewers/jellyfin/src/main.rs`
- Modify: `viewers/jellyfin/src/handlers.rs`

**Step 1: Read the full `viewers/jellyfin/src/handlers.rs`**

Identify every call to `state.bangumi.*`. List each one (e.g., `search_anime`, `get_subject`, `get_episodes`, `download_image`).

**Step 2: Create `metadata_client.rs`**

```rust
use anyhow::Result;

pub struct MetadataClient {
    http: reqwest::Client,
    base_url: String,
}

pub struct EpisodeInfo {
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<String>,
    pub summary: Option<String>,
}

impl MetadataClient {
    pub fn new(base_url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
        }
    }

    /// Get episode metadata for a specific episode number.
    pub async fn enrich_episodes(
        &self,
        bangumi_id: i32,
        episode_no: i32,
    ) -> Result<Option<EpisodeInfo>> {
        let resp = self
            .http
            .post(format!("{}/enrich/episodes", self.base_url))
            .json(&serde_json::json!({
                "bangumi_id": bangumi_id,
                "episode_no": episode_no
            }))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Ok(None);
        }
        let body: serde_json::Value = resp.json().await?;
        Ok(Some(EpisodeInfo {
            title: body["title"].as_str().map(|s| s.to_string()),
            title_cn: body["title_cn"].as_str().map(|s| s.to_string()),
            air_date: body["air_date"].as_str().map(|s| s.to_string()),
            summary: body["summary"].as_str().map(|s| s.to_string()),
        }))
    }
}
```

**Step 3: Update `AppState` in `main.rs`**

```rust
// OLD:
pub struct AppState {
    pub organizer: Arc<FileOrganizer>,
    pub db: db::DbPool,
    pub bangumi: Arc<bangumi_client::BangumiClient>,
}

// NEW:
pub struct AppState {
    pub organizer: Arc<FileOrganizer>,
    pub db: db::DbPool,
    pub metadata: Arc<metadata_client::MetadataClient>,
}
```

Update instantiation in `main()`:

```rust
let metadata_url = std::env::var("METADATA_SERVICE_URL")
    .unwrap_or_else(|_| "http://metadata:8004".to_string());
let metadata = Arc::new(metadata_client::MetadataClient::new(metadata_url));
let state = AppState { organizer, db, metadata };
```

Add `mod metadata_client;` to `main.rs`.

**Step 4: Update `handlers.rs`**

The `ViewerSyncRequest` now includes `bangumi_id` and `cover_image_url`.

In `fetch_and_generate_metadata` (or equivalent function), replace all `state.bangumi.*` calls:

```rust
// OLD: let bangumi_id = bangumi.search_anime(anime_title).await?;
// NEW: use bangumi_id from the request
let bangumi_id = match req.bangumi_id {
    Some(id) => id,
    None => {
        tracing::info!("No bangumi_id in sync request — skipping NFO generation");
        return Ok(());
    }
};

// OLD: let subject = bangumi.get_subject(bangumi_id).await?;
// OLD: let episodes = bangumi.get_episodes(bangumi_id).await?;
// NEW:
let ep_info = state.metadata
    .enrich_episodes(bangumi_id, req.episode_no)
    .await
    .unwrap_or(None);

// For poster download: use cover_image_url from request directly
if let Some(poster_url) = &req.cover_image_url {
    // download_image still works — it's just an HTTP GET, not a Bangumi API call
    // Use reqwest directly or keep a simple download helper
}
```

Adapt any NFO generation calls to use `ep_info` instead of the old subject/episode structures. Read the existing handler code carefully to map each call.

**Step 5: Build Viewer**

```bash
cargo build -p viewer-jellyfin
```

Fix all remaining `state.bangumi` references. Do not delete `bangumi_client.rs` yet — comment out `mod bangumi_client;` only after the build succeeds without it.

**Step 6: Commit**

```bash
git add viewers/jellyfin/src/
git commit -m "feat(viewer): replace direct Bangumi.tv calls with Metadata Service client"
```

---

### Task 12: Frontend — Schema + CoreApi Methods

**Files:**
- Modify: `frontend/src/schemas/anime.ts`
- Modify: `frontend/src/services/CoreApi.ts`
- Modify: `frontend/src/layers/ApiLayer.ts`

**Step 1: Add `AnimeCoverImage` schema and update `AnimeSeriesRich`**

In `frontend/src/schemas/anime.ts`:

```typescript
export const AnimeCoverImage = Schema.Struct({
  cover_id: Schema.Number,
  anime_id: Schema.Number,
  image_url: Schema.String,
  service_module_id: Schema.NullOr(Schema.Number),
  source_name: Schema.String,
  is_default: Schema.Boolean,
  created_at: Schema.String,
})
export type AnimeCoverImage = typeof AnimeCoverImage.Type
```

Find `AnimeSeriesRich` and add `cover_image_url`:

```typescript
export const AnimeSeriesRich = Schema.Struct({
  // ... all existing fields ...
  cover_image_url: Schema.NullOr(Schema.String),   // NEW
})
```

**Step 2: Add methods to `CoreApi` tag**

In `frontend/src/services/CoreApi.ts`, add to the interface:

```typescript
readonly getAnimeCoverImages: (
  animeId: number,
) => Effect.Effect<readonly AnimeCoverImage[]>
readonly setDefaultCoverImage: (
  animeId: number,
  coverId: number,
) => Effect.Effect<void>
```

Import `AnimeCoverImage` from schemas.

**Step 3: Implement in `ApiLayer.ts`**

Read the existing file to confirm the `fetchJson` / `postJson` helper signatures and `API_BASE` constant. Then add:

```typescript
getAnimeCoverImages: (animeId) =>
  fetchJson(
    HttpClientRequest.get(`/api/core/anime/${animeId}/covers`),
    Schema.Array(AnimeCoverImage),
  ),

setDefaultCoverImage: (animeId, coverId) =>
  client
    .execute(
      HttpClientRequest.post(
        `/api/core/anime/${animeId}/covers/${coverId}/set-default`,
      ).pipe(HttpClientRequest.bodyUnsafeJson({})),
    )
    .pipe(Effect.scoped, Effect.orDie, Effect.map(() => undefined)),
```

Adjust to match the exact pattern used in the file (e.g., if `postJson` helper handles the response differently).

**Step 4: TypeScript check**

```bash
cd /workspace/frontend
npx tsc --noEmit
```

Fix any type errors.

**Step 5: Commit**

```bash
git add frontend/src/schemas/anime.ts \
        frontend/src/services/CoreApi.ts \
        frontend/src/layers/ApiLayer.ts
git commit -m "feat(frontend): add AnimeCoverImage schema and cover API methods"
```

---

### Task 13: Frontend — AnimeSeriesCard Component

**Files:**
- Create: `frontend/src/components/AnimeSeriesCard.tsx`

**Step 1: Check an existing component for import/style conventions**

Read one file from `frontend/src/components/` to confirm:
- Import path aliases (`@/` vs relative)
- Which shadcn/ui components are available
- Tailwind class conventions

**Step 2: Create `AnimeSeriesCard.tsx`**

```tsx
import type { AnimeSeriesRich } from "@/schemas/anime"

interface Props {
  series: AnimeSeriesRich
  onClick?: () => void
}

export function AnimeSeriesCard({ series, onClick }: Props) {
  const hasImage = !!series.cover_image_url
  const initial = series.anime_title.charAt(0).toUpperCase()

  return (
    <div
      className="cursor-pointer rounded-lg overflow-hidden border border-border hover:border-primary transition-colors"
      onClick={onClick}
    >
      {/* Cover image — 2:3 aspect ratio */}
      <div className="relative w-full aspect-[2/3] bg-muted">
        {hasImage ? (
          <img
            src={series.cover_image_url!}
            alt={series.anime_title}
            className="w-full h-full object-cover"
            loading="lazy"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center text-4xl font-bold text-muted-foreground">
            {initial}
          </div>
        )}
        {/* Title overlay */}
        <div className="absolute bottom-0 left-0 right-0 px-2 py-1 bg-gradient-to-t from-black/70 to-transparent">
          <p className="text-white text-sm font-medium line-clamp-2">
            {series.anime_title}
          </p>
        </div>
      </div>

      {/* Metadata — two lines, no decoration */}
      <div className="px-2 py-1.5 text-xs text-muted-foreground space-y-0.5">
        <p>
          S{series.series_no}
          {series.season
            ? ` · ${series.season.year} ${series.season.season}`
            : ""}
          {` · ${series.episode_downloaded}/${series.episode_found}`}
        </p>
        {series.subscriptions.length > 0 && (
          <p className="truncate">
            {series.subscriptions.map((s) => s.name ?? "Unknown").join(", ")}
          </p>
        )}
      </div>
    </div>
  )
}
```

**Step 3: TypeScript check**

```bash
cd /workspace/frontend
npx tsc --noEmit
```

**Step 4: Commit**

```bash
git add frontend/src/components/AnimeSeriesCard.tsx
git commit -m "feat(frontend): add AnimeSeriesCard thumbnail component"
```

---

### Task 14: Frontend — AnimeSeriesPage Grid/List Toggle

**Files:**
- Modify: `frontend/src/pages/anime-series/AnimeSeriesPage.tsx`

**Step 1: Read the full page file**

Read `/workspace/frontend/src/pages/anime-series/AnimeSeriesPage.tsx` to understand:
- How `seriesList` data is fetched
- How the DataTable `onRowClick` works (to replicate in grid card `onClick`)
- Where to place the toggle buttons

**Step 2: Add imports and state**

```tsx
import { useState } from "react"
import { LayoutGrid, List } from "lucide-react"
import { Button } from "@/components/ui/button"
import { AnimeSeriesCard } from "@/components/AnimeSeriesCard"

// Inside component:
const [viewMode, setViewMode] = useState<"grid" | "list">(() => {
  return (
    (localStorage.getItem("anime-series-view") as "grid" | "list") ?? "grid"
  )
})

const handleViewMode = (mode: "grid" | "list") => {
  setViewMode(mode)
  localStorage.setItem("anime-series-view", mode)
}
```

**Step 3: Add toggle buttons to page header**

Find the existing page header area (where the title or "Create" button lives). Add the toggle alongside:

```tsx
<div className="flex items-center gap-1">
  <Button
    variant={viewMode === "list" ? "secondary" : "ghost"}
    size="icon"
    onClick={() => handleViewMode("list")}
  >
    <List className="h-4 w-4" />
  </Button>
  <Button
    variant={viewMode === "grid" ? "secondary" : "ghost"}
    size="icon"
    onClick={() => handleViewMode("grid")}
  >
    <LayoutGrid className="h-4 w-4" />
  </Button>
</div>
```

**Step 4: Conditional render — grid vs list**

Replace the existing `<DataTable .../>` with:

```tsx
{viewMode === "grid" ? (
  <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4">
    {(seriesList ?? []).map((series) => (
      <AnimeSeriesCard
        key={series.series_id}
        series={series}
        onClick={() => {
          /* replicate the existing DataTable onRowClick logic here */
        }}
      />
    ))}
  </div>
) : (
  <DataTable columns={columns} data={seriesList ?? []} onRowClick={...} />
)}
```

Copy the exact `onRowClick` handler from the existing DataTable usage.

**Step 5: TypeScript check**

```bash
cd /workspace/frontend
npx tsc --noEmit
```

**Step 6: Commit**

```bash
git add frontend/src/pages/anime-series/AnimeSeriesPage.tsx
git commit -m "feat(frontend): add grid/list view toggle to AnimeSeriesPage"
```

---

### Task 15: Frontend — AnimeDialog Cover Image Switcher

**Files:**
- Modify: `frontend/src/pages/anime/AnimeDialog.tsx`

**Step 1: Read the full AnimeDialog file**

Read `/workspace/frontend/src/pages/anime/AnimeDialog.tsx` to understand:
- How `anime.anime_id` is available
- The Effect-based data fetching pattern used
- The query invalidation pattern (how other mutations refresh data)
- Where to inject the cover image section visually

**Step 2: Add cover state and fetch**

```tsx
import { useState } from "react"
import { ChevronLeft, ChevronRight } from "lucide-react"
import type { AnimeCoverImage } from "@/schemas/anime"

// Inside component body:
const [covers, setCovers] = useState<AnimeCoverImage[]>([])
const [coverIndex, setCoverIndex] = useState(0)
```

Fetch covers alongside existing queries, using the same Effect pattern as the file uses for other API calls:

```tsx
// Follow the file's existing pattern for useEffectQuery or similar
// Call: api.getAnimeCoverImages(anime.anime_id)
// On success: setCovers(result); setCoverIndex(result.findIndex(c => c.is_default) ?? 0)
```

Read the file first to determine the exact hook/Effect invocation style.

**Step 3: Add cover image section**

Insert above the main anime info block:

```tsx
{covers.length > 0 && (
  <div className="group relative w-40 mx-auto aspect-[2/3] flex-shrink-0">
    <img
      src={covers[coverIndex].image_url}
      alt="Cover"
      className="w-full h-full object-cover rounded-lg"
    />
    {covers.length > 1 && (
      <>
        <button
          className="absolute left-1 top-1/2 -translate-y-1/2 bg-black/50 hover:bg-black/70 text-white rounded-full p-0.5 opacity-0 group-hover:opacity-100 transition-opacity"
          onClick={async (e) => {
            e.stopPropagation()
            const newIdx = (coverIndex - 1 + covers.length) % covers.length
            setCoverIndex(newIdx)
            // Call set-default and invalidate series query
            await runSetDefault(covers[newIdx].cover_id)
          }}
        >
          <ChevronLeft className="h-4 w-4" />
        </button>
        <button
          className="absolute right-1 top-1/2 -translate-y-1/2 bg-black/50 hover:bg-black/70 text-white rounded-full p-0.5 opacity-0 group-hover:opacity-100 transition-opacity"
          onClick={async (e) => {
            e.stopPropagation()
            const newIdx = (coverIndex + 1) % covers.length
            setCoverIndex(newIdx)
            await runSetDefault(covers[newIdx].cover_id)
          }}
        >
          <ChevronRight className="h-4 w-4" />
        </button>
      </>
    )}
    {/* Source label */}
    <div className="absolute bottom-1 left-0 right-0 text-center pointer-events-none">
      <span className="text-white/70 text-xs">
        {coverIndex + 1}/{covers.length} · {covers[coverIndex].source_name}
      </span>
    </div>
  </div>
)}
```

**Step 4: Implement `runSetDefault`**

```tsx
const runSetDefault = async (coverId: number) => {
  // Follow the file's existing pattern for mutation/Effect execution
  // Call: api.setDefaultCoverImage(anime.anime_id, coverId)
  // Then: invalidate getAllAnimeSeries cache so grid thumbnails update
}
```

Read the existing pattern for mutations/invalidation in the dialog (look for how other updates trigger query refetches).

**Step 5: TypeScript check**

```bash
cd /workspace/frontend
npx tsc --noEmit
```

**Step 6: Commit**

```bash
git add frontend/src/pages/anime/AnimeDialog.tsx
git commit -m "feat(frontend): add cover image switcher in AnimeDialog"
```

---

### Task 16: Infrastructure — Dockerfile + docker-compose

**Files:**
- Create: `Dockerfile.metadata`
- Modify: `docker-compose.yaml`

**Step 1: Read `Dockerfile.viewer-jellyfin`**

Read the existing Dockerfile to match the multi-stage build pattern used for workspace crates.

**Step 2: Create `Dockerfile.metadata`**

Follow the same pattern. The metadata service has no DB dependency:

```dockerfile
# Build stage
FROM rust:1.82-slim AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY shared/ ./shared/
COPY metadata/ ./metadata/

# Stub other workspace members to cache dependencies
RUN mkdir -p core-service/src downloaders/qbittorrent/src fetchers/mikanani/src viewers/jellyfin/src cli/src \
    && echo 'fn main(){}' > core-service/src/main.rs \
    && echo 'fn main(){}' > downloaders/qbittorrent/src/main.rs \
    && echo 'fn main(){}' > fetchers/mikanani/src/main.rs \
    && echo 'fn main(){}' > viewers/jellyfin/src/main.rs \
    && echo 'fn main(){}' > cli/src/main.rs
COPY core-service/Cargo.toml ./core-service/
COPY downloaders/qbittorrent/Cargo.toml ./downloaders/qbittorrent/
COPY fetchers/mikanani/Cargo.toml ./fetchers/mikanani/
COPY viewers/jellyfin/Cargo.toml ./viewers/jellyfin/
COPY cli/Cargo.toml ./cli/

RUN cargo build --release -p metadata-service

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/metadata-service /usr/local/bin/
CMD ["metadata-service"]
```

NOTE: Read the existing Dockerfile carefully and adapt — exact stub paths and apt packages may differ.

**Step 3: Add service to `docker-compose.yaml`**

Read the existing `docker-compose.yaml` to match formatting and `depends_on` patterns. Add:

```yaml
metadata:
  build:
    context: .
    dockerfile: Dockerfile.metadata
  environment:
    - CORE_SERVICE_URL=http://core-service:8000/api/core
    - SERVICE_HOST=metadata
    - PORT=8004
  ports:
    - "8004:8004"
  depends_on:
    - core-service
  restart: unless-stopped
```

**Step 4: Validate**

```bash
docker compose config --quiet
```

Expected: no syntax errors.

**Step 5: Commit**

```bash
git add Dockerfile.metadata docker-compose.yaml
git commit -m "feat(infra): add metadata service to docker-compose"
```

---

## Summary

| # | Layer | Key Deliverable |
|---|-------|----------------|
| 1 | DB | `anime_cover_images` table + `metadata` enum value |
| 2 | Shared | `Metadata` in `ServiceType`; `bangumi_id`/`cover_image_url` in `ViewerSyncRequest` |
| 3 | Core | `AnimeCoverImage` model; `Metadata` in `ModuleTypeEnum` |
| 4–7 | Metadata Svc | New Rust service: Bangumi client, `/enrich/anime`, `/enrich/episodes`, auto-registration |
| 8 | Core | Cover image CRUD API (`list` + `set-default`) |
| 9 | Core | Background cover fetch on anime creation |
| 10 | Core | `cover_image_url` in `AnimeSeriesRich` response |
| 11 | Viewer | `metadata_client.rs` replaces direct Bangumi.tv calls |
| 12 | Frontend | `AnimeCoverImage` schema + `CoreApi` methods |
| 13 | Frontend | `AnimeSeriesCard` thumbnail component |
| 14 | Frontend | Grid/list toggle in `AnimeSeriesPage` |
| 15 | Frontend | Cover image switcher in `AnimeDialog` |
| 16 | Infra | `Dockerfile.metadata` + `docker-compose.yaml` |

**Dependency order**: Tasks 1–3 must complete before 4–10; Task 11 requires Tasks 2 + 7; Tasks 12–15 require Task 10.
