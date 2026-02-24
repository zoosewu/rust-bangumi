# Anime Thumbnail Display & Metadata Service Design

**Date**: 2026-02-24
**Status**: Approved

## Overview

Add thumbnail card display to AnimeSeriesPage and introduce a dedicated Metadata Service that centralises all external anime metadata queries (Bangumi.tv). Cover images are stored per-anime in a new `anime_cover_images` table and displayed in the frontend with an in-image switching UI.

---

## 1. Database Changes

### New table: `anime_cover_images`

```sql
CREATE TABLE anime_cover_images (
  cover_id          SERIAL PRIMARY KEY,
  anime_id          INTEGER NOT NULL REFERENCES animes(anime_id) ON DELETE CASCADE,
  image_url         TEXT NOT NULL,
  service_module_id INTEGER REFERENCES service_modules(module_id) ON DELETE SET NULL,
  source_name       VARCHAR(100) NOT NULL,   -- e.g. "bangumi", "anilist"
  is_default        BOOLEAN NOT NULL DEFAULT FALSE,
  created_at        TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(anime_id, image_url)
);
```

- `service_module_id` references the Metadata Service entry in `service_modules`.
- For images originating from the Metadata Service, `service_module_id` is set; NULL is reserved for manually added images.
- Only one row per anime may have `is_default = TRUE`. Switching default is done in a single transaction (set all FALSE, then set target TRUE).

### `module_type` enum

Add a new value to the existing `module_type` enum:

```sql
ALTER TYPE module_type ADD VALUE 'metadata';
```

### `AnimeSeriesRich` DTO

Add `cover_image_url: Option<String>` to the existing DTO. The `GET /series` query joins `anime_cover_images WHERE is_default = TRUE` to populate this field.

---

## 2. Metadata Service

### Purpose

Single gateway for all external anime metadata. Initially implements Bangumi.tv; additional providers (AniList, MAL) can be added later without touching other services.

### Tech stack

Rust + Axum, stateless (no own database). Registers with Core on startup.

### Directory

```
/workspace/metadata/
├── Cargo.toml
├── Dockerfile.metadata
└── src/
    ├── main.rs           -- Axum server + service registration
    ├── handlers.rs       -- route handlers
    ├── bangumi_client.rs -- Bangumi.tv HTTP client
    └── models.rs         -- request/response DTOs
```

### Endpoints

```
POST /enrich/anime     -- query cover images and metadata by title
POST /enrich/episodes  -- query episode metadata by bangumi_id (for Viewer NFO)
GET  /health
```

**`POST /enrich/anime`**

```json
// Request
{ "title": "進擊の巨人", "title_ja": null }

// Response
{
  "bangumi_id": 12345,
  "cover_images": [
    { "url": "https://lain.bgm.tv/pic/cover/l/xx.jpg", "source": "bangumi" }
  ],
  "summary": "...",
  "air_date": "2013-04-06"
}
```

**`POST /enrich/episodes`**

```json
// Request
{ "bangumi_id": 12345, "episode_no": 5 }

// Response
{
  "episode_no": 5,
  "title": "First Battle",
  "title_cn": "初次戰鬥",
  "air_date": "2013-05-05",
  "summary": "..."
}
```

### Service registration

On startup, the Metadata Service registers itself with Core:

```json
POST /services/register
{
  "module_type": "metadata",
  "name": "metadata-bangumi",
  "version": "0.1.0",
  "base_url": "http://metadata-service:8003"
}
```

---

## 3. Data Flows

### Flow A: Anime creation → cover image

```
fetcher RSS
  → Core creates Anime record
      → spawn tokio background task
          → query service_modules WHERE module_type='metadata' AND is_enabled=true
          → POST /enrich/anime { title }
          → Metadata Service calls Bangumi.tv API
          → returns { bangumi_id, cover_images[] }
          → Core inserts into anime_cover_images
              (service_module_id = metadata service module_id,
               first image: is_default = TRUE)
```

### Flow B: Download complete → Viewer NFO generation

```
Core detects download completed
  → Core resolves bangumi_id from anime_cover_images (or animes table)
  → POST /sync to Viewer with ViewerSyncRequest + bangumi_id
      → Viewer calls Metadata Service: POST /enrich/episodes { bangumi_id, episode_no }
      → Metadata Service calls Bangumi.tv API
      → Viewer generates tvshow.nfo + episode.nfo + downloads poster.jpg
```

### Cover image default switching

```
User clicks arrow on cover image in AnimeDialog
  → Frontend cycles to next image in local list
  → POST /anime/:id/covers/:cover_id/set-default
  → Core transaction: UPDATE SET is_default=FALSE, then SET is_default=TRUE
  → Frontend invalidates AnimeSeriesPage query cache
  → AnimeSeriesPage re-fetches and shows new default thumbnail
```

---

## 4. Core Service Changes

- New API endpoints:
  - `GET  /anime/:anime_id/covers` — list all cover images for an anime
  - `POST /anime/:anime_id/covers/:cover_id/set-default` — set default
- Background task logic in anime creation handler to call Metadata Service
- `AnimeSeriesRich` query updated to LEFT JOIN `anime_cover_images`
- `ViewerSyncRequest` gains `bangumi_id: Option<i32>` field

---

## 5. Viewer Changes

Remove `bangumi_client.rs`. Replace with a lightweight `metadata_client.rs` that calls the Metadata Service's `/enrich/episodes` endpoint.

`ViewerSyncRequest` (shared model) gains `bangumi_id: Option<i32>`. When the field is present, Viewer calls Metadata Service; when absent, Viewer skips NFO metadata and only organises files.

---

## 6. Frontend Changes

### AnimeSeriesPage — view mode toggle

- Right-aligned toggle in page header: List / Grid (default: Grid)
- Selection persisted in `localStorage`

### `AnimeSeriesCard` component

Layout (poster ratio 2:3):

```
┌─────────────────────┐
│                     │
│    cover image      │
│                     │
│▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│
│ Anime Title         │  ← overlay with gradient
└─────────────────────┘
S3  ·  2023 Spring  ·  8/12
Mikanani
```

- No cover image: grey placeholder showing first character of title
- Below-image text: two lines, plain text, no decorative elements

### `AnimeDialog` — cover image switching

Arrows appear on image hover, no separate UI section:

```
┌────────────────────────┐
│  ←        1/3       →  │  ← visible on hover
│                        │
│       cover image      │
│                        │
│  · bangumi ·           │  ← small source label at bottom
└────────────────────────┘
```

Clicking an arrow:
1. Cycles the displayed image locally
2. Calls `POST /anime/:id/covers/:cover_id/set-default`
3. Invalidates AnimeSeriesPage query cache so thumbnail updates

---

## 7. New API Endpoints Summary

| Method | Path | Description |
|--------|------|-------------|
| GET | `/anime/:id/covers` | List all cover images |
| POST | `/anime/:id/covers/:cover_id/set-default` | Set default cover |

---

## 8. Out of Scope

- Viewer Bangumi.tv migration can be done as a follow-up; Viewer continues to work with its own `bangumi_client.rs` until migration is complete.
- Multiple metadata providers (AniList, MAL) are not in scope for this iteration.
- Manual cover image upload is not in scope.
