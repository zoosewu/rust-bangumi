# Bangumi Frontend Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a React 19 + Effect-TS SPA frontend with Shadcn/UI for managing Anime, subscriptions, filters, and parsers — including before/after preview functionality for filters and parsers.

**Architecture:** Three layers: (1) Backend preview APIs added to existing Rust core-service, (2) React 19 SPA with Effect-TS for all data fetching/validation/error handling, (3) Caddy-based production deployment with Vite dev proxy for development. The frontend communicates with core-service (:8000), downloader (:8002), and viewer (:8003) through `/api/{service}/*` prefixed routes.

**Tech Stack:** React 19, TypeScript, Vite, Effect-TS (`effect`, `@effect/schema`, `@effect/platform`), Shadcn/UI (Radix + Tailwind CSS), React Router v7, Caddy

---

## Task 1: Backend — Filter Preview API

**Files:**
- Modify: `core-service/src/handlers/filters.rs`
- Modify: `core-service/src/main.rs:124-129` (add route)

**Context:** The existing `filters.rs` has `create_filter_rule`, `get_filter_rules`, `delete_filter_rule` handlers. We need to add a `preview_filter` handler. The `FilterRule` model lives in `core-service/src/models/db.rs:264-275` with fields: `rule_id`, `rule_order`, `regex_pattern`, `is_positive`, `target_type`, `target_id`. Filter rules are fetched via `state.repos.filter_rule.find_by_target()` which returns rules sorted by `rule_order`.

The preview logic: given a regex pattern + is_positive flag, and optionally an `exclude_filter_id`, load all relevant filter rules (excluding the one being edited), apply them to raw_anime_items to get the "before" result, then also apply the new/edited filter to get the "after" result.

**Step 1: Add preview DTOs and handler to `filters.rs`**

Add at the end of `core-service/src/handlers/filters.rs`, before the closing:

```rust
use regex::Regex;

/// Preview request for filter rules
#[derive(Debug, Deserialize)]
pub struct FilterPreviewRequest {
    pub regex_pattern: String,
    pub is_positive: bool,
    pub subscription_id: Option<i32>,
    pub exclude_filter_id: Option<i32>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PreviewItem {
    pub item_id: i32,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct FilterPreviewPanel {
    pub passed_items: Vec<PreviewItem>,
    pub filtered_items: Vec<PreviewItem>,
}

#[derive(Debug, Serialize)]
pub struct FilterPreviewResponse {
    pub regex_valid: bool,
    pub regex_error: Option<String>,
    pub before: FilterPreviewPanel,
    pub after: FilterPreviewPanel,
}

/// POST /filters/preview
pub async fn preview_filter(
    State(state): State<AppState>,
    Json(req): Json<FilterPreviewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate regex
    let regex = match Regex::new(&req.regex_pattern) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::OK,
                Json(json!(FilterPreviewResponse {
                    regex_valid: false,
                    regex_error: Some(e.to_string()),
                    before: FilterPreviewPanel { passed_items: vec![], filtered_items: vec![] },
                    after: FilterPreviewPanel { passed_items: vec![], filtered_items: vec![] },
                })),
            );
        }
    };

    let limit = req.limit.unwrap_or(50).min(200);

    // Load raw items
    let items = match state.repos.raw_item.find_with_filters(
        crate::db::repository::raw_item::RawItemFilter {
            status: None,
            subscription_id: req.subscription_id,
            limit,
            offset: 0,
        }
    ).await {
        Ok(items) => items,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to load items: {}", e) })),
            );
        }
    };

    // Load existing filter rules (all global rules for now)
    let existing_rules = match state.repos.filter_rule
        .find_by_target(FilterTargetType::Global, None).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to load rules: {}", e) })),
            );
        }
    };

    // Build "before" rules (exclude current filter)
    let before_rules: Vec<&FilterRule> = existing_rules
        .iter()
        .filter(|r| Some(r.rule_id) != req.exclude_filter_id)
        .collect();

    // Apply before rules
    let (before_passed, before_filtered) = apply_filter_rules(&items, &before_rules);

    // Apply after rules (before rules + new rule)
    let (after_passed, after_filtered) = {
        let mut after_passed = vec![];
        let mut after_filtered = vec![];
        for item in &items {
            let mut passed_existing = true;
            // First check existing rules (excluding current)
            for rule in &before_rules {
                let r = Regex::new(&rule.regex_pattern).unwrap_or_else(|_| Regex::new("$^").unwrap());
                let matches = r.is_match(&item.title);
                if rule.is_positive && !matches {
                    passed_existing = false;
                    break;
                }
                if !rule.is_positive && matches {
                    passed_existing = false;
                    break;
                }
            }
            if !passed_existing {
                after_filtered.push(PreviewItem { item_id: item.item_id, title: item.title.clone() });
                continue;
            }

            // Then check the new rule
            let matches_new = regex.is_match(&item.title);
            let passed_new = if req.is_positive { matches_new } else { !matches_new };
            if passed_new {
                after_passed.push(PreviewItem { item_id: item.item_id, title: item.title.clone() });
            } else {
                after_filtered.push(PreviewItem { item_id: item.item_id, title: item.title.clone() });
            }
        }
        (after_passed, after_filtered)
    };

    (
        StatusCode::OK,
        Json(json!(FilterPreviewResponse {
            regex_valid: true,
            regex_error: None,
            before: FilterPreviewPanel { passed_items: before_passed, filtered_items: before_filtered },
            after: FilterPreviewPanel { passed_items: after_passed, filtered_items: after_filtered },
        })),
    )
}

fn apply_filter_rules(
    items: &[crate::models::RawAnimeItem],
    rules: &[&FilterRule],
) -> (Vec<PreviewItem>, Vec<PreviewItem>) {
    let mut passed = vec![];
    let mut filtered = vec![];
    for item in items {
        let mut item_passed = true;
        for rule in rules {
            let r = match Regex::new(&rule.regex_pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let matches = r.is_match(&item.title);
            if rule.is_positive && !matches {
                item_passed = false;
                break;
            }
            if !rule.is_positive && matches {
                item_passed = false;
                break;
            }
        }
        let preview = PreviewItem { item_id: item.item_id, title: item.title.clone() };
        if item_passed { passed.push(preview); } else { filtered.push(preview); }
    }
    (passed, filtered)
}
```

**Step 2: Add the route to `main.rs`**

In `core-service/src/main.rs`, after line 129 (the `/filters/:rule_id` delete route), add:

```rust
        .route("/filters/preview", post(handlers::filters::preview_filter))
```

Note: This route MUST come before the `/filters/:rule_id` route to avoid path conflicts, OR use a distinct path. Place it right after the existing filter routes block.

**Step 3: Verify it compiles**

Run: `cargo build -p core-service 2>&1 | tail -20`
Expected: Build succeeds (or warnings only)

**Step 4: Test manually**

Run: `cargo run -p core-service &` then:
```bash
curl -s -X POST http://localhost:8000/filters/preview \
  -H 'Content-Type: application/json' \
  -d '{"regex_pattern":"1080p","is_positive":true,"limit":10}' | jq .
```
Expected: JSON response with `regex_valid: true` and before/after panels

**Step 5: Commit**

```bash
git add core-service/src/handlers/filters.rs core-service/src/main.rs
git commit -m "feat(core): add POST /filters/preview endpoint with before/after comparison"
```

---

## Task 2: Backend — Parser Preview API

**Files:**
- Modify: `core-service/src/handlers/parsers.rs`
- Modify: `core-service/src/services/title_parser.rs:79` (make `try_parser` pub)
- Modify: `core-service/src/main.rs` (add route)

**Context:** `TitleParserService::try_parser()` at `title_parser.rs:79` is currently `fn try_parser` (private). We need it `pub` so the preview handler can call it. The service iterates parsers by priority desc — highest priority wins.

The preview logic: given a parser configuration + `exclude_parser_id`, load all enabled parsers (excluding the one being edited), run them against raw items to get "before" matching. Then add the new/edited parser (at its specified priority) to get "after" matching. Report which items changed assignment.

**Step 1: Make `try_parser` public in `title_parser.rs`**

In `core-service/src/services/title_parser.rs:79`, change:
```rust
    fn try_parser(parser: &TitleParser, title: &str) -> Result<Option<ParsedResult>, String> {
```
to:
```rust
    pub fn try_parser(parser: &TitleParser, title: &str) -> Result<Option<ParsedResult>, String> {
```

**Step 2: Add preview handler to `parsers.rs`**

Add at the end of `core-service/src/handlers/parsers.rs`:

```rust
use regex::Regex;

// ============ Preview DTOs ============

#[derive(Debug, Deserialize)]
pub struct ParserPreviewRequest {
    pub condition_regex: String,
    pub parse_regex: String,
    pub priority: i32,
    pub anime_title_source: String,
    pub anime_title_value: String,
    pub episode_no_source: String,
    pub episode_no_value: String,
    pub series_no_source: Option<String>,
    pub series_no_value: Option<String>,
    pub subtitle_group_source: Option<String>,
    pub subtitle_group_value: Option<String>,
    pub resolution_source: Option<String>,
    pub resolution_value: Option<String>,
    pub season_source: Option<String>,
    pub season_value: Option<String>,
    pub year_source: Option<String>,
    pub year_value: Option<String>,
    pub exclude_parser_id: Option<i32>,
    pub subscription_id: Option<i32>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ParsedFields {
    pub anime_title: String,
    pub episode_no: i32,
    pub series_no: i32,
    pub subtitle_group: Option<String>,
    pub resolution: Option<String>,
    pub season: Option<String>,
    pub year: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ParserPreviewResult {
    pub title: String,
    pub before_matched_by: Option<String>,
    pub after_matched_by: Option<String>,
    pub is_newly_matched: bool,
    pub is_override: bool,
    pub parse_result: Option<ParsedFields>,
}

#[derive(Debug, Serialize)]
pub struct ParserPreviewResponse {
    pub condition_regex_valid: bool,
    pub parse_regex_valid: bool,
    pub regex_error: Option<String>,
    pub results: Vec<ParserPreviewResult>,
}

/// POST /parsers/preview
pub async fn preview_parser(
    State(state): State<AppState>,
    Json(req): Json<ParserPreviewRequest>,
) -> Result<Json<ParserPreviewResponse>, (StatusCode, String)> {
    // Validate regexes
    if let Err(e) = Regex::new(&req.condition_regex) {
        return Ok(Json(ParserPreviewResponse {
            condition_regex_valid: false,
            parse_regex_valid: true,
            regex_error: Some(format!("condition_regex: {}", e)),
            results: vec![],
        }));
    }
    if let Err(e) = Regex::new(&req.parse_regex) {
        return Ok(Json(ParserPreviewResponse {
            condition_regex_valid: true,
            parse_regex_valid: false,
            regex_error: Some(format!("parse_regex: {}", e)),
            results: vec![],
        }));
    }

    let limit = req.limit.unwrap_or(20).min(200);

    // Load raw items
    let items = state.repos.raw_item.find_with_filters(
        crate::db::repository::raw_item::RawItemFilter {
            status: None,
            subscription_id: req.subscription_id,
            limit,
            offset: 0,
        }
    ).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Load existing enabled parsers
    let all_parsers = state.repos.title_parser
        .find_enabled_sorted_by_priority()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Build "before" parsers (exclude current)
    let before_parsers: Vec<&TitleParser> = all_parsers
        .iter()
        .filter(|p| Some(p.parser_id) != req.exclude_parser_id)
        .collect();

    // Build a temporary TitleParser for the "current" parser being previewed
    let now = Utc::now().naive_utc();
    let current_parser = TitleParser {
        parser_id: -1, // sentinel
        name: "(preview)".to_string(),
        description: None,
        priority: req.priority,
        is_enabled: true,
        condition_regex: req.condition_regex.clone(),
        parse_regex: req.parse_regex.clone(),
        anime_title_source: parse_source_type(&req.anime_title_source)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.1))?,
        anime_title_value: req.anime_title_value.clone(),
        episode_no_source: parse_source_type(&req.episode_no_source)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.1))?,
        episode_no_value: req.episode_no_value.clone(),
        series_no_source: req.series_no_source.as_ref()
            .map(|s| parse_source_type(s))
            .transpose()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.1))?,
        series_no_value: req.series_no_value.clone(),
        subtitle_group_source: req.subtitle_group_source.as_ref()
            .map(|s| parse_source_type(s))
            .transpose()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.1))?,
        subtitle_group_value: req.subtitle_group_value.clone(),
        resolution_source: req.resolution_source.as_ref()
            .map(|s| parse_source_type(s))
            .transpose()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.1))?,
        resolution_value: req.resolution_value.clone(),
        season_source: req.season_source.as_ref()
            .map(|s| parse_source_type(s))
            .transpose()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.1))?,
        season_value: req.season_value.clone(),
        year_source: req.year_source.as_ref()
            .map(|s| parse_source_type(s))
            .transpose()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.1))?,
        year_value: req.year_value.clone(),
        created_at: now,
        updated_at: now,
    };

    // Build "after" parsers list: before_parsers + current_parser, sorted by priority desc
    let mut after_parsers_owned: Vec<&TitleParser> = before_parsers.clone();
    after_parsers_owned.push(&current_parser);
    after_parsers_owned.sort_by(|a, b| b.priority.cmp(&a.priority));

    // Process each item
    let mut results = Vec::new();
    for item in &items {
        // "before" - which parser matches?
        let before_match = find_matching_parser(&before_parsers, &item.title);
        // "after" - which parser matches?
        let after_match = find_matching_parser(&after_parsers_owned, &item.title);

        let before_name = before_match.map(|p| p.name.clone());
        let after_name = after_match.map(|p| {
            if p.parser_id == -1 { "(current)".to_string() } else { p.name.clone() }
        });

        let is_current_after = after_match.map(|p| p.parser_id == -1).unwrap_or(false);
        let is_newly_matched = before_match.is_none() && is_current_after;
        let is_override = before_match.is_some() && is_current_after
            && before_match.map(|p| p.parser_id) != Some(-1);

        // Parse result only if current parser matched in "after"
        let parse_result = if is_current_after {
            TitleParserService::try_parser(&current_parser, &item.title)
                .ok()
                .flatten()
                .map(|r| ParsedFields {
                    anime_title: r.anime_title,
                    episode_no: r.episode_no,
                    series_no: r.series_no,
                    subtitle_group: r.subtitle_group,
                    resolution: r.resolution,
                    season: r.season,
                    year: r.year,
                })
        } else {
            None
        };

        results.push(ParserPreviewResult {
            title: item.title.clone(),
            before_matched_by: before_name,
            after_matched_by: after_name,
            is_newly_matched,
            is_override,
            parse_result,
        });
    }

    Ok(Json(ParserPreviewResponse {
        condition_regex_valid: true,
        parse_regex_valid: true,
        regex_error: None,
        results,
    }))
}

/// Find the first parser that matches a title (parsers must be pre-sorted by priority desc)
fn find_matching_parser<'a>(parsers: &[&'a TitleParser], title: &str) -> Option<&'a TitleParser> {
    for parser in parsers {
        if let Ok(Some(_)) = TitleParserService::try_parser(parser, title) {
            return Some(parser);
        }
    }
    None
}
```

**Step 3: Add the route to `main.rs`**

In `core-service/src/main.rs`, after the parsers routes block (around line 171), add:

```rust
        .route("/parsers/preview", post(handlers::parsers::preview_parser))
```

**Step 4: Verify it compiles**

Run: `cargo build -p core-service 2>&1 | tail -20`
Expected: Build succeeds

**Step 5: Test manually**

```bash
curl -s -X POST http://localhost:8000/parsers/preview \
  -H 'Content-Type: application/json' \
  -d '{
    "condition_regex": ".*",
    "parse_regex": "\\[(.+?)\\]\\s*(.+?)\\s*-\\s*(\\d+)",
    "priority": 10,
    "anime_title_source": "regex", "anime_title_value": "2",
    "episode_no_source": "regex", "episode_no_value": "3",
    "limit": 5
  }' | jq .
```
Expected: JSON with `condition_regex_valid: true` and results array

**Step 6: Commit**

```bash
git add core-service/src/handlers/parsers.rs core-service/src/services/title_parser.rs core-service/src/main.rs
git commit -m "feat(core): add POST /parsers/preview endpoint with priority-based match assignment"
```

---

## Task 3: Frontend — Project Scaffolding (Vite + React 19 + TypeScript + Tailwind)

**Files:**
- Create: `frontend/package.json`
- Create: `frontend/vite.config.ts`
- Create: `frontend/tsconfig.json`
- Create: `frontend/tsconfig.app.json`
- Create: `frontend/tsconfig.node.json`
- Create: `frontend/tailwind.config.ts`
- Create: `frontend/postcss.config.js`
- Create: `frontend/index.html`
- Create: `frontend/src/main.tsx`
- Create: `frontend/src/App.tsx`
- Create: `frontend/src/index.css`
- Create: `frontend/src/vite-env.d.ts`
- Create: `frontend/components.json`
- Create: `frontend/src/lib/utils.ts`

**Step 1: Initialize Vite project**

```bash
cd /workspace
npm create vite@latest frontend -- --template react-ts
```

**Step 2: Install dependencies**

```bash
cd /workspace/frontend
npm install effect @effect/schema @effect/platform @effect/platform-browser
npm install react-router-dom@7
npm install tailwindcss @tailwindcss/vite
npm install class-variance-authority clsx tailwind-merge lucide-react
npm install @radix-ui/react-slot @radix-ui/react-dialog @radix-ui/react-dropdown-menu @radix-ui/react-select @radix-ui/react-tabs @radix-ui/react-toast @radix-ui/react-tooltip @radix-ui/react-switch @radix-ui/react-separator @radix-ui/react-scroll-area
```

**Step 3: Configure `vite.config.ts`**

Replace `frontend/vite.config.ts` with:

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5173,
    proxy: {
      '/api/core': {
        target: 'http://localhost:8000',
        rewrite: (p) => p.replace(/^\/api\/core/, ''),
        changeOrigin: true,
      },
      '/api/downloader': {
        target: 'http://localhost:8002',
        rewrite: (p) => p.replace(/^\/api\/downloader/, ''),
        changeOrigin: true,
      },
      '/api/viewer': {
        target: 'http://localhost:8003',
        rewrite: (p) => p.replace(/^\/api\/viewer/, ''),
        changeOrigin: true,
      },
    },
  },
})
```

**Step 4: Configure `tailwind.config.ts`**

Note: With Tailwind v4 + @tailwindcss/vite, config is in CSS. Replace `frontend/src/index.css`:

```css
@import "tailwindcss";

@theme {
  --color-background: oklch(1 0 0);
  --color-foreground: oklch(0.145 0 0);
  --color-card: oklch(1 0 0);
  --color-card-foreground: oklch(0.145 0 0);
  --color-primary: oklch(0.205 0 0);
  --color-primary-foreground: oklch(0.985 0 0);
  --color-secondary: oklch(0.97 0 0);
  --color-secondary-foreground: oklch(0.205 0 0);
  --color-muted: oklch(0.97 0 0);
  --color-muted-foreground: oklch(0.556 0 0);
  --color-accent: oklch(0.97 0 0);
  --color-accent-foreground: oklch(0.205 0 0);
  --color-destructive: oklch(0.577 0.245 27.325);
  --color-destructive-foreground: oklch(0.577 0.245 27.325);
  --color-border: oklch(0.922 0 0);
  --color-input: oklch(0.922 0 0);
  --color-ring: oklch(0.708 0 0);
  --radius-sm: 0.25rem;
  --radius-md: 0.375rem;
  --radius-lg: 0.5rem;
  --radius-xl: 0.75rem;
}

@layer base {
  body {
    @apply bg-background text-foreground;
  }
}
```

**Step 5: Create `frontend/src/lib/utils.ts`**

```typescript
import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
```

**Step 6: Create minimal `App.tsx`**

```typescript
export default function App() {
  return (
    <div className="min-h-screen bg-background">
      <h1 className="text-2xl font-bold p-8">Bangumi</h1>
    </div>
  )
}
```

**Step 7: Update `main.tsx`**

```typescript
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import './index.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
```

**Step 8: Verify dev server starts**

Run: `cd /workspace/frontend && npm run dev -- --host 0.0.0.0`
Expected: Vite starts on :5173, page shows "Bangumi"

**Step 9: Verify build succeeds**

Run: `cd /workspace/frontend && npm run build`
Expected: `dist/` directory created successfully

**Step 10: Commit**

```bash
git add frontend/
git commit -m "feat(frontend): scaffold Vite + React 19 + TypeScript + Tailwind project"
```

---

## Task 4: Frontend — Effect-TS Runtime, Layers, and Core API Service

**Files:**
- Create: `frontend/src/runtime/AppRuntime.ts`
- Create: `frontend/src/layers/ApiLayer.ts`
- Create: `frontend/src/services/CoreApi.ts`
- Create: `frontend/src/schemas/common.ts`
- Create: `frontend/src/schemas/anime.ts`
- Create: `frontend/src/schemas/filter.ts`
- Create: `frontend/src/schemas/parser.ts`
- Create: `frontend/src/schemas/subscription.ts`
- Create: `frontend/src/schemas/download.ts`

**Context:** Check the latest Effect-TS docs (context7) for the correct API patterns for `HttpClient`, `Schema`, `Context.Tag`, `Layer`, and `ManagedRuntime`. The Effect-TS API has changed significantly — always verify imports and patterns against current docs.

**Step 1: Create Effect Schemas**

`frontend/src/schemas/common.ts`:
```typescript
import { Schema } from "effect"

export const PreviewItem = Schema.Struct({
  item_id: Schema.Number,
  title: Schema.String,
})
export type PreviewItem = typeof PreviewItem.Type
```

`frontend/src/schemas/anime.ts`:
```typescript
import { Schema } from "effect"

export const Anime = Schema.Struct({
  anime_id: Schema.Number,
  title: Schema.String,
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type Anime = typeof Anime.Type

export const AnimeSeries = Schema.Struct({
  series_id: Schema.Number,
  anime_id: Schema.Number,
  series_no: Schema.Number,
  season_id: Schema.Number,
  description: Schema.NullOr(Schema.String),
  aired_date: Schema.NullOr(Schema.String),
  end_date: Schema.NullOr(Schema.String),
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type AnimeSeries = typeof AnimeSeries.Type

export const Season = Schema.Struct({
  season_id: Schema.Number,
  year: Schema.Number,
  season: Schema.String,
  created_at: Schema.String,
})
export type Season = typeof Season.Type

export const SubtitleGroup = Schema.Struct({
  group_id: Schema.Number,
  group_name: Schema.String,
  created_at: Schema.String,
})
export type SubtitleGroup = typeof SubtitleGroup.Type
```

`frontend/src/schemas/filter.ts`:
```typescript
import { Schema } from "effect"
import { PreviewItem } from "./common"

export const FilterRule = Schema.Struct({
  rule_id: Schema.Number,
  target_type: Schema.String,
  target_id: Schema.NullOr(Schema.Number),
  rule_order: Schema.Number,
  is_positive: Schema.Boolean,
  regex_pattern: Schema.String,
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type FilterRule = typeof FilterRule.Type

export const FilterPreviewPanel = Schema.Struct({
  passed_items: Schema.Array(PreviewItem),
  filtered_items: Schema.Array(PreviewItem),
})

export const FilterPreviewResponse = Schema.Struct({
  regex_valid: Schema.Boolean,
  regex_error: Schema.NullOr(Schema.String),
  before: FilterPreviewPanel,
  after: FilterPreviewPanel,
})
export type FilterPreviewResponse = typeof FilterPreviewResponse.Type
```

`frontend/src/schemas/parser.ts`:
```typescript
import { Schema } from "effect"

export const TitleParser = Schema.Struct({
  parser_id: Schema.Number,
  name: Schema.String,
  description: Schema.NullOr(Schema.String),
  priority: Schema.Number,
  is_enabled: Schema.Boolean,
  condition_regex: Schema.String,
  parse_regex: Schema.String,
  anime_title_source: Schema.String,
  anime_title_value: Schema.String,
  episode_no_source: Schema.String,
  episode_no_value: Schema.String,
  series_no_source: Schema.NullOr(Schema.String),
  series_no_value: Schema.NullOr(Schema.String),
  subtitle_group_source: Schema.NullOr(Schema.String),
  subtitle_group_value: Schema.NullOr(Schema.String),
  resolution_source: Schema.NullOr(Schema.String),
  resolution_value: Schema.NullOr(Schema.String),
  season_source: Schema.NullOr(Schema.String),
  season_value: Schema.NullOr(Schema.String),
  year_source: Schema.NullOr(Schema.String),
  year_value: Schema.NullOr(Schema.String),
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type TitleParser = typeof TitleParser.Type

export const ParsedFields = Schema.Struct({
  anime_title: Schema.String,
  episode_no: Schema.Number,
  series_no: Schema.Number,
  subtitle_group: Schema.NullOr(Schema.String),
  resolution: Schema.NullOr(Schema.String),
  season: Schema.NullOr(Schema.String),
  year: Schema.NullOr(Schema.String),
})

export const ParserPreviewResult = Schema.Struct({
  title: Schema.String,
  before_matched_by: Schema.NullOr(Schema.String),
  after_matched_by: Schema.NullOr(Schema.String),
  is_newly_matched: Schema.Boolean,
  is_override: Schema.Boolean,
  parse_result: Schema.NullOr(ParsedFields),
})

export const ParserPreviewResponse = Schema.Struct({
  condition_regex_valid: Schema.Boolean,
  parse_regex_valid: Schema.Boolean,
  regex_error: Schema.NullOr(Schema.String),
  results: Schema.Array(ParserPreviewResult),
})
export type ParserPreviewResponse = typeof ParserPreviewResponse.Type
```

`frontend/src/schemas/subscription.ts`:
```typescript
import { Schema } from "effect"

export const Subscription = Schema.Struct({
  subscription_id: Schema.Number,
  fetcher_id: Schema.Number,
  source_url: Schema.String,
  name: Schema.NullOr(Schema.String),
  description: Schema.NullOr(Schema.String),
  last_fetched_at: Schema.NullOr(Schema.String),
  next_fetch_at: Schema.NullOr(Schema.String),
  fetch_interval_minutes: Schema.Number,
  is_active: Schema.Boolean,
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type Subscription = typeof Subscription.Type
```

`frontend/src/schemas/download.ts`:
```typescript
import { Schema } from "effect"

export const RawAnimeItem = Schema.Struct({
  item_id: Schema.Number,
  title: Schema.String,
  description: Schema.NullOr(Schema.String),
  download_url: Schema.String,
  pub_date: Schema.NullOr(Schema.String),
  subscription_id: Schema.Number,
  status: Schema.String,
  parser_id: Schema.NullOr(Schema.Number),
  error_message: Schema.NullOr(Schema.String),
  parsed_at: Schema.NullOr(Schema.String),
  created_at: Schema.String,
})
export type RawAnimeItem = typeof RawAnimeItem.Type
```

**Step 2: Create CoreApi Service**

`frontend/src/services/CoreApi.ts` — This must follow current Effect-TS patterns. Consult context7 for `@effect/platform` HttpClient usage. The service wraps all core-service API calls:

```typescript
import { Effect, Context, Layer } from "effect"
import { HttpClient, HttpClientRequest, HttpClientResponse } from "@effect/platform"
import { Schema } from "effect"
import { Anime } from "@/schemas/anime"
import { FilterRule, FilterPreviewResponse } from "@/schemas/filter"
import { TitleParser, ParserPreviewResponse } from "@/schemas/parser"
import { Subscription } from "@/schemas/subscription"
import { RawAnimeItem } from "@/schemas/download"

// Define the CoreApi service interface
export class CoreApi extends Context.Tag("CoreApi")<
  CoreApi,
  {
    readonly getAnimes: Effect.Effect<readonly Anime[]>
    readonly createAnime: (title: string) => Effect.Effect<Anime>
    readonly deleteAnime: (id: number) => Effect.Effect<void>
    readonly getSubscriptions: Effect.Effect<readonly Subscription[]>
    readonly getFilterRules: (targetType: string, targetId?: number) => Effect.Effect<readonly FilterRule[]>
    readonly previewFilter: (req: {
      regex_pattern: string
      is_positive: boolean
      subscription_id?: number
      exclude_filter_id?: number
      limit?: number
    }) => Effect.Effect<FilterPreviewResponse>
    readonly getParsers: Effect.Effect<readonly TitleParser[]>
    readonly previewParser: (req: Record<string, unknown>) => Effect.Effect<ParserPreviewResponse>
    readonly getRawItems: (params: { status?: string; subscription_id?: number; limit?: number; offset?: number }) => Effect.Effect<readonly RawAnimeItem[]>
    readonly getHealth: Effect.Effect<{ status: string }>
  }
>() {}
```

**Step 3: Create API Layer**

`frontend/src/layers/ApiLayer.ts` — implements CoreApi using HttpClient. Consult context7 for the correct `@effect/platform` browser HTTP client patterns:

```typescript
import { Effect, Layer } from "effect"
import { HttpClient, HttpClientRequest, HttpClientResponse } from "@effect/platform"
import { BrowserHttpClient } from "@effect/platform-browser"
import { Schema } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { Anime } from "@/schemas/anime"
import { FilterPreviewResponse } from "@/schemas/filter"
import { ParserPreviewResponse } from "@/schemas/parser"
import { Subscription } from "@/schemas/subscription"
import { RawAnimeItem } from "@/schemas/download"

// NOTE: Verify all @effect/platform imports against context7 docs.
// The API may use HttpClient.execute, HttpClientRequest.get, etc.

const makeCorApi = Effect.gen(function* () {
  const client = yield* HttpClient.HttpClient

  const fetchJson = <A>(request: HttpClientRequest.HttpClientRequest, schema: Schema.Schema<A>) =>
    client.execute(request).pipe(
      Effect.flatMap(HttpClientResponse.schemaBodyJson(schema)),
      Effect.scoped
    )

  return CoreApi.of({
    getAnimes: fetchJson(
      HttpClientRequest.get("/api/core/anime"),
      Schema.Array(Anime)
    ),

    createAnime: (title) =>
      client.execute(
        HttpClientRequest.post("/api/core/anime").pipe(
          HttpClientRequest.jsonBody({ title })
        )
      ).pipe(
        Effect.flatMap(Effect.flatMap(
          (req) => req,
          HttpClientResponse.schemaBodyJson(Anime)
        )),
        Effect.scoped
      ),

    deleteAnime: (id) =>
      client.execute(
        HttpClientRequest.del(`/api/core/anime/${id}`)
      ).pipe(
        Effect.asVoid,
        Effect.scoped
      ),

    getSubscriptions: fetchJson(
      HttpClientRequest.get("/api/core/subscriptions"),
      Schema.Array(Subscription)
    ),

    getFilterRules: (targetType, targetId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/filters?target_type=${targetType}${targetId ? `&target_id=${targetId}` : ""}`),
        Schema.Struct({ rules: Schema.Array(Schema.Any) })
      ).pipe(Effect.map((r) => r.rules)),

    previewFilter: (req) =>
      client.execute(
        HttpClientRequest.post("/api/core/filters/preview").pipe(
          HttpClientRequest.jsonBody(req)
        )
      ).pipe(
        Effect.flatMap(Effect.flatMap(
          (r) => r,
          HttpClientResponse.schemaBodyJson(FilterPreviewResponse)
        )),
        Effect.scoped
      ),

    getParsers: fetchJson(
      HttpClientRequest.get("/api/core/parsers"),
      Schema.Array(Schema.Any)
    ),

    previewParser: (req) =>
      client.execute(
        HttpClientRequest.post("/api/core/parsers/preview").pipe(
          HttpClientRequest.jsonBody(req)
        )
      ).pipe(
        Effect.flatMap(Effect.flatMap(
          (r) => r,
          HttpClientResponse.schemaBodyJson(ParserPreviewResponse)
        )),
        Effect.scoped
      ),

    getRawItems: (params) => {
      const qs = new URLSearchParams()
      if (params.status) qs.set("status", params.status)
      if (params.subscription_id) qs.set("subscription_id", String(params.subscription_id))
      if (params.limit) qs.set("limit", String(params.limit))
      if (params.offset) qs.set("offset", String(params.offset))
      return fetchJson(
        HttpClientRequest.get(`/api/core/raw-items?${qs.toString()}`),
        Schema.Array(RawAnimeItem)
      )
    },

    getHealth: fetchJson(
      HttpClientRequest.get("/api/core/health"),
      Schema.Struct({ status: Schema.String })
    ),
  })
})

export const CoreApiLive = Layer.effect(CoreApi, makeCorApi)
```

**Step 4: Create App Runtime**

`frontend/src/runtime/AppRuntime.ts`:
```typescript
import { ManagedRuntime, Layer } from "effect"
import { BrowserHttpClient } from "@effect/platform-browser"
import { CoreApiLive } from "@/layers/ApiLayer"

// NOTE: Check context7 for correct BrowserHttpClient layer composition
const AppLayer = CoreApiLive.pipe(
  Layer.provide(BrowserHttpClient.layerXMLHttpRequest)
)

export const AppRuntime = ManagedRuntime.make(AppLayer)
```

**Step 5: Verify build succeeds**

Run: `cd /workspace/frontend && npm run build`
Expected: Build succeeds. If there are type errors from `@effect/platform`, consult context7 for the latest API.

**Step 6: Commit**

```bash
git add frontend/src/schemas/ frontend/src/services/ frontend/src/layers/ frontend/src/runtime/
git commit -m "feat(frontend): add Effect-TS schemas, CoreApi service, layers, and runtime"
```

---

## Task 5: Frontend — React Hooks (Effect-to-React Bridge)

**Files:**
- Create: `frontend/src/hooks/useEffectQuery.ts`
- Create: `frontend/src/hooks/useEffectMutation.ts`

**Context:** These hooks bridge Effect-TS effects to React state. `useEffectQuery` runs an effect and manages loading/error/data state. `useEffectMutation` provides a trigger function for write operations.

**Step 1: Create `useEffectQuery.ts`**

```typescript
import { useState, useEffect, useCallback, useRef } from "react"
import { Effect } from "effect"
import { AppRuntime } from "@/runtime/AppRuntime"

export function useEffectQuery<A>(
  effectFn: () => Effect.Effect<A, unknown, never>,
  deps: unknown[] = []
) {
  const [data, setData] = useState<A | null>(null)
  const [error, setError] = useState<unknown>(null)
  const [isLoading, setIsLoading] = useState(true)
  const mountedRef = useRef(true)

  const execute = useCallback(() => {
    setIsLoading(true)
    setError(null)
    AppRuntime.runPromise(effectFn()).then(
      (result) => {
        if (mountedRef.current) {
          setData(result)
          setIsLoading(false)
        }
      },
      (err) => {
        if (mountedRef.current) {
          setError(err)
          setIsLoading(false)
        }
      }
    )
  }, deps)

  useEffect(() => {
    mountedRef.current = true
    execute()
    return () => { mountedRef.current = false }
  }, [execute])

  return { data, error, isLoading, refetch: execute }
}
```

**Step 2: Create `useEffectMutation.ts`**

```typescript
import { useState, useCallback, useRef } from "react"
import { Effect } from "effect"
import { AppRuntime } from "@/runtime/AppRuntime"

export function useEffectMutation<Args extends unknown[], A>(
  effectFn: (...args: Args) => Effect.Effect<A, unknown, never>
) {
  const [data, setData] = useState<A | null>(null)
  const [error, setError] = useState<unknown>(null)
  const [isLoading, setIsLoading] = useState(false)
  const mountedRef = useRef(true)

  const mutate = useCallback((...args: Args) => {
    setIsLoading(true)
    setError(null)
    return AppRuntime.runPromise(effectFn(...args)).then(
      (result) => {
        if (mountedRef.current) {
          setData(result)
          setIsLoading(false)
        }
        return result
      },
      (err) => {
        if (mountedRef.current) {
          setError(err)
          setIsLoading(false)
        }
        throw err
      }
    )
  }, [effectFn])

  return { mutate, data, error, isLoading }
}
```

**Step 3: Verify build**

Run: `cd /workspace/frontend && npm run build`

**Step 4: Commit**

```bash
git add frontend/src/hooks/
git commit -m "feat(frontend): add useEffectQuery and useEffectMutation React hooks"
```

---

## Task 6: Frontend — Layout + Routing (AppLayout, Sidebar, Header)

**Files:**
- Create: `frontend/src/components/layout/AppLayout.tsx`
- Create: `frontend/src/components/layout/Sidebar.tsx`
- Create: `frontend/src/components/layout/Header.tsx`
- Modify: `frontend/src/App.tsx` (add React Router routes)
- Create: `frontend/src/pages/Dashboard.tsx` (placeholder)

**Step 1: Install Shadcn/UI components needed for layout**

Initialize Shadcn with:
```bash
cd /workspace/frontend
npx shadcn@latest init
```

Then add components:
```bash
npx shadcn@latest add button badge separator scroll-area tooltip
```

**Step 2: Create Sidebar**

`frontend/src/components/layout/Sidebar.tsx`:
```typescript
import { NavLink } from "react-router-dom"
import { cn } from "@/lib/utils"
import {
  LayoutDashboard, Film, Rss, FileText, Download,
  Filter, FileCode, AlertTriangle
} from "lucide-react"

const navItems = [
  { to: "/", icon: LayoutDashboard, label: "Dashboard" },
  { to: "/anime", icon: Film, label: "Anime" },
  { to: "/subscriptions", icon: Rss, label: "Subscriptions" },
  { to: "/raw-items", icon: FileText, label: "Raw Items" },
  { to: "/downloads", icon: Download, label: "Downloads" },
  { to: "/filters", icon: Filter, label: "Filters" },
  { to: "/parsers", icon: FileCode, label: "Parsers" },
  { to: "/conflicts", icon: AlertTriangle, label: "Conflicts" },
]

export function Sidebar() {
  return (
    <aside className="w-60 border-r bg-card h-screen sticky top-0 flex flex-col">
      <div className="p-4 font-bold text-lg border-b">Bangumi</div>
      <nav className="flex-1 p-2 space-y-1">
        {navItems.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            end={to === "/"}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors",
                isActive
                  ? "bg-primary text-primary-foreground"
                  : "hover:bg-accent text-muted-foreground hover:text-foreground"
              )
            }
          >
            <Icon className="h-4 w-4" />
            {label}
          </NavLink>
        ))}
      </nav>
    </aside>
  )
}
```

**Step 3: Create Header**

`frontend/src/components/layout/Header.tsx`:
```typescript
export function Header() {
  return (
    <header className="h-14 border-b flex items-center px-6">
      <h2 className="text-sm text-muted-foreground">Bangumi Management</h2>
    </header>
  )
}
```

**Step 4: Create AppLayout**

`frontend/src/components/layout/AppLayout.tsx`:
```typescript
import { Outlet } from "react-router-dom"
import { Sidebar } from "./Sidebar"
import { Header } from "./Header"

export function AppLayout() {
  return (
    <div className="flex min-h-screen">
      <Sidebar />
      <div className="flex-1 flex flex-col">
        <Header />
        <main className="flex-1 p-6">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
```

**Step 5: Create Dashboard placeholder**

`frontend/src/pages/Dashboard.tsx`:
```typescript
export default function Dashboard() {
  return <div>Dashboard (TODO)</div>
}
```

**Step 6: Set up routing in `App.tsx`**

```typescript
import { BrowserRouter, Routes, Route } from "react-router-dom"
import { AppLayout } from "@/components/layout/AppLayout"
import Dashboard from "@/pages/Dashboard"

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route index element={<Dashboard />} />
          {/* More routes added in later tasks */}
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
```

**Step 7: Verify dev server**

Run: `cd /workspace/frontend && npm run dev`
Expected: Sidebar with nav links visible, Dashboard placeholder shown

**Step 8: Commit**

```bash
git add frontend/src/components/layout/ frontend/src/pages/Dashboard.tsx frontend/src/App.tsx
git commit -m "feat(frontend): add AppLayout with Sidebar, Header, and React Router setup"
```

---

## Task 7: Frontend — Shared Components (DataTable, StatusBadge, ConfirmDialog, RegexInput)

**Files:**
- Create: `frontend/src/components/shared/DataTable.tsx`
- Create: `frontend/src/components/shared/StatusBadge.tsx`
- Create: `frontend/src/components/shared/ConfirmDialog.tsx`
- Create: `frontend/src/components/shared/RegexInput.tsx`

**Step 1: Install additional Shadcn components**

```bash
cd /workspace/frontend
npx shadcn@latest add table dialog input label textarea select switch card
npx shadcn@latest add toast
```

**Step 2: Create DataTable**

A generic, reusable data table component. `frontend/src/components/shared/DataTable.tsx`:

```typescript
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table"

export interface Column<T> {
  key: string
  header: string
  render: (item: T) => React.ReactNode
}

interface DataTableProps<T> {
  columns: Column<T>[]
  data: T[]
  keyField: string
  onRowClick?: (item: T) => void
}

export function DataTable<T extends Record<string, unknown>>({
  columns, data, keyField, onRowClick,
}: DataTableProps<T>) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          {columns.map((col) => (
            <TableHead key={col.key}>{col.header}</TableHead>
          ))}
        </TableRow>
      </TableHeader>
      <TableBody>
        {data.map((item) => (
          <TableRow
            key={String(item[keyField])}
            onClick={() => onRowClick?.(item)}
            className={onRowClick ? "cursor-pointer" : ""}
          >
            {columns.map((col) => (
              <TableCell key={col.key}>{col.render(item)}</TableCell>
            ))}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
```

**Step 3: Create StatusBadge**

`frontend/src/components/shared/StatusBadge.tsx`:
```typescript
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

const statusColors: Record<string, string> = {
  pending: "bg-yellow-100 text-yellow-800",
  parsed: "bg-green-100 text-green-800",
  no_match: "bg-gray-100 text-gray-800",
  failed: "bg-red-100 text-red-800",
  skipped: "bg-blue-100 text-blue-800",
}

export function StatusBadge({ status }: { status: string }) {
  return (
    <Badge variant="outline" className={cn("text-xs", statusColors[status])}>
      {status}
    </Badge>
  )
}
```

**Step 4: Create ConfirmDialog**

`frontend/src/components/shared/ConfirmDialog.tsx`:
```typescript
import {
  Dialog, DialogContent, DialogDescription, DialogFooter,
  DialogHeader, DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"

interface ConfirmDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description: string
  onConfirm: () => void
  loading?: boolean
}

export function ConfirmDialog({
  open, onOpenChange, title, description, onConfirm, loading,
}: ConfirmDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={onConfirm} disabled={loading}>
            {loading ? "Deleting..." : "Delete"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

**Step 5: Create RegexInput**

`frontend/src/components/shared/RegexInput.tsx`:
```typescript
import { useState, useEffect } from "react"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

interface RegexInputProps {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  className?: string
}

export function RegexInput({ value, onChange, placeholder, className }: RegexInputProps) {
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!value) { setError(null); return }
    try {
      new RegExp(value)
      setError(null)
    } catch (e) {
      setError((e as Error).message)
    }
  }, [value])

  return (
    <div>
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={cn(
          "font-mono text-sm",
          error && "border-destructive",
          className
        )}
      />
      {error && (
        <p className="text-xs text-destructive mt-1">{error}</p>
      )}
    </div>
  )
}
```

**Step 6: Verify build**

Run: `cd /workspace/frontend && npm run build`

**Step 7: Commit**

```bash
git add frontend/src/components/shared/
git commit -m "feat(frontend): add shared components — DataTable, StatusBadge, ConfirmDialog, RegexInput"
```

---

## Task 8: Frontend — Dashboard Page

**Files:**
- Modify: `frontend/src/pages/Dashboard.tsx`
- Create: `frontend/src/hooks/useHealth.ts`

**Step 1: Create health hook**

`frontend/src/hooks/useHealth.ts`:
```typescript
import { useEffectQuery } from "./useEffectQuery"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"

export function useHealth() {
  return useEffectQuery(
    () => Effect.gen(function* () {
      const api = yield* CoreApi
      return yield* api.getHealth
    }).pipe(Effect.provide(/* provided by runtime */)),
    []
  )
}
```

Note: The exact pattern for providing the layer through `AppRuntime.runPromise` needs to be verified against context7. The `useEffectQuery` hook uses `AppRuntime.runPromise` which already provides the layers.

**Step 2: Implement Dashboard**

`frontend/src/pages/Dashboard.tsx`:
```typescript
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"

export default function Dashboard() {
  const health = useEffectQuery(
    () => Effect.gen(function* () {
      const api = yield* CoreApi
      return yield* api.getHealth
    }),
    []
  )

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Dashboard</h1>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Core Service
            </CardTitle>
          </CardHeader>
          <CardContent>
            {health.isLoading ? (
              <span className="text-muted-foreground">Checking...</span>
            ) : health.error ? (
              <Badge variant="destructive">Offline</Badge>
            ) : (
              <Badge className="bg-green-100 text-green-800">Online</Badge>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
```

**Step 3: Verify dev server**

Run: `cd /workspace/frontend && npm run dev`
Expected: Dashboard shows health status card

**Step 4: Commit**

```bash
git add frontend/src/pages/Dashboard.tsx frontend/src/hooks/useHealth.ts
git commit -m "feat(frontend): implement Dashboard page with service health status"
```

---

## Task 9: Frontend — Anime + Series Pages

**Files:**
- Create: `frontend/src/pages/anime/AnimePage.tsx`
- Create: `frontend/src/pages/anime/AnimeDetailPage.tsx`
- Modify: `frontend/src/App.tsx` (add routes)

**Step 1: Create AnimePage**

Anime list with DataTable + create/delete dialogs.

**Step 2: Create AnimeDetailPage**

Tabs for Series, Links, Filters.

**Step 3: Add routes**

In `App.tsx`:
```typescript
<Route path="anime" element={<AnimePage />} />
<Route path="anime/:animeId" element={<AnimeDetailPage />} />
```

**Step 4: Commit**

```bash
git add frontend/src/pages/anime/ frontend/src/App.tsx
git commit -m "feat(frontend): add Anime list and detail pages"
```

---

## Task 10: Frontend — Subscriptions + Raw Items Pages

**Files:**
- Create: `frontend/src/pages/subscriptions/SubscriptionsPage.tsx`
- Create: `frontend/src/pages/raw-items/RawItemsPage.tsx`
- Modify: `frontend/src/App.tsx` (add routes)

**Step 1: Create SubscriptionsPage**

DataTable with name, source_url, fetch interval, status badge. Create/delete subscription dialogs.

**Step 2: Create RawItemsPage**

Filter bar (status dropdown, subscription dropdown), DataTable with pagination, reparse/skip actions.

**Step 3: Add routes and commit**

```bash
git add frontend/src/pages/subscriptions/ frontend/src/pages/raw-items/ frontend/src/App.tsx
git commit -m "feat(frontend): add Subscriptions and Raw Items pages"
```

---

## Task 11: Frontend — Filters Page with Before/After Preview

**Files:**
- Create: `frontend/src/pages/filters/FiltersPage.tsx`
- Modify: `frontend/src/App.tsx` (add route)

**Context:** This is the key feature page. Layout:
- Top: Filter form (regex, include/exclude, subscription selector, target type/id)
- Bottom: Left-right split panel
  - Left: "Before" (without this filter) — passed/filtered items
  - Right: "After" (with this filter) — passed/filtered items
- Debounce 500ms before calling `POST /api/core/filters/preview`

**Step 1: Create FiltersPage**

`frontend/src/pages/filters/FiltersPage.tsx`:

```typescript
import { useState, useEffect, useCallback } from "react"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { RegexInput } from "@/components/shared/RegexInput"
import { Badge } from "@/components/ui/badge"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import type { FilterPreviewResponse } from "@/schemas/filter"

function useDebounce<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value)
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedValue(value), delay)
    return () => clearTimeout(timer)
  }, [value, delay])
  return debouncedValue
}

export default function FiltersPage() {
  const [regexPattern, setRegexPattern] = useState("")
  const [isPositive, setIsPositive] = useState(true)
  const [preview, setPreview] = useState<FilterPreviewResponse | null>(null)

  const debouncedRegex = useDebounce(regexPattern, 500)

  const { mutate: fetchPreview, isLoading } = useEffectMutation(
    (pattern: string, positive: boolean) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.previewFilter({
          regex_pattern: pattern,
          is_positive: positive,
        })
      })
  )

  useEffect(() => {
    if (!debouncedRegex) { setPreview(null); return }
    fetchPreview(debouncedRegex, isPositive)
      .then(setPreview)
      .catch(() => setPreview(null))
  }, [debouncedRegex, isPositive])

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Filter Rules</h1>

      {/* Filter Form */}
      <Card>
        <CardContent className="pt-6 space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <Label>Regex Pattern</Label>
              <RegexInput
                value={regexPattern}
                onChange={setRegexPattern}
                placeholder="e.g. 1080p"
              />
            </div>
            <div className="flex items-center gap-2 pt-6">
              <Switch checked={isPositive} onCheckedChange={setIsPositive} />
              <Label>{isPositive ? "Include (positive)" : "Exclude (negative)"}</Label>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Before / After Preview */}
      {preview && (
        <div className="grid grid-cols-2 gap-4">
          <PreviewPanel
            title="Before (without this filter)"
            passed={preview.before.passed_items}
            filtered={preview.before.filtered_items}
          />
          <PreviewPanel
            title="After (with this filter)"
            passed={preview.after.passed_items}
            filtered={preview.after.filtered_items}
            highlightDiff
          />
        </div>
      )}
    </div>
  )
}

function PreviewPanel({
  title, passed, filtered, highlightDiff,
}: {
  title: string
  passed: { item_id: number; title: string }[]
  filtered: { item_id: number; title: string }[]
  highlightDiff?: boolean
}) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm">{title}</CardTitle>
        <div className="flex gap-4 text-xs">
          <span className="text-green-600">Passed: {passed.length}</span>
          <span className="text-red-600">Filtered: {filtered.length}</span>
        </div>
      </CardHeader>
      <CardContent>
        <ScrollArea className="h-80">
          <div className="space-y-1">
            {passed.map((item) => (
              <div
                key={item.item_id}
                className={cn(
                  "text-xs px-2 py-1 rounded font-mono",
                  "bg-green-50 text-green-800"
                )}
              >
                {item.title}
              </div>
            ))}
            {filtered.map((item) => (
              <div
                key={item.item_id}
                className={cn(
                  "text-xs px-2 py-1 rounded font-mono",
                  highlightDiff ? "bg-red-50 text-red-600" : "bg-gray-50 text-gray-500"
                )}
              >
                {item.title}
              </div>
            ))}
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  )
}
```

**Step 2: Add route in `App.tsx`**

```typescript
<Route path="filters" element={<FiltersPage />} />
```

**Step 3: Verify with dev server**

Run frontend + core-service, navigate to /filters, type a regex, see preview

**Step 4: Commit**

```bash
git add frontend/src/pages/filters/ frontend/src/App.tsx
git commit -m "feat(frontend): add Filters page with before/after live preview"
```

---

## Task 12: Frontend — Parsers Page with Match Assignment + Parse Results

**Files:**
- Create: `frontend/src/pages/parsers/ParsersPage.tsx`
- Modify: `frontend/src/App.tsx` (add route)

**Context:** Two-part preview:
1. Match Assignment table: title | before_matched_by | after_matched_by (highlight newly_matched and overrides)
2. Parse Results: cards showing full parsed fields for items matched by current parser

**Step 1: Create ParsersPage**

`frontend/src/pages/parsers/ParsersPage.tsx` — similar pattern to FiltersPage but with:
- Parser form (name, priority, condition_regex, parse_regex, field source/value pairs)
- Match Assignment table
- Parse Results cards

This is the most complex page. Follow the same debounce + `useEffectMutation` pattern as FiltersPage.

**Step 2: Add route and commit**

```bash
git add frontend/src/pages/parsers/ frontend/src/App.tsx
git commit -m "feat(frontend): add Parsers page with match assignment table and parse results"
```

---

## Task 13: Frontend — Downloads Page

**Files:**
- Create: `frontend/src/pages/downloads/DownloadsPage.tsx`
- Create: `frontend/src/services/DownloaderApi.ts`
- Modify: `frontend/src/App.tsx` (add route)

**Step 1: Create DownloaderApi service and Downloads page**

DataTable with title, status, progress bar, size. Auto-refresh every 5 seconds. Pause/Resume/Cancel actions.

**Step 2: Add route and commit**

```bash
git add frontend/src/pages/downloads/ frontend/src/services/DownloaderApi.ts frontend/src/App.tsx
git commit -m "feat(frontend): add Downloads page with auto-refresh and actions"
```

---

## Task 14: Frontend — Conflicts Page + Toast Notifications

**Files:**
- Create: `frontend/src/pages/conflicts/ConflictsPage.tsx`
- Modify: `frontend/src/App.tsx` (add route + Toaster)

**Step 1: Create ConflictsPage**

List pending conflicts, resolve by selecting fetcher.

**Step 2: Add Toaster to App.tsx root**

```typescript
import { Toaster } from "@/components/ui/toaster"
// In JSX: <Toaster /> at the bottom
```

**Step 3: Commit**

```bash
git add frontend/src/pages/conflicts/ frontend/src/App.tsx
git commit -m "feat(frontend): add Conflicts page and toast notifications"
```

---

## Task 15: Deployment — Caddy Production + Docker Compose

**Files:**
- Create: `frontend/Dockerfile`
- Create: `frontend/Caddyfile`
- Create: `frontend/.dockerignore`
- Modify: `docker-compose.yaml` (add frontend service)

**Step 1: Create `frontend/.dockerignore`**

```
node_modules
dist
.git
```

**Step 2: Create `frontend/Dockerfile`**

```dockerfile
FROM node:22-alpine AS builder
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM caddy:alpine
COPY --from=builder /app/dist /srv
COPY Caddyfile /etc/caddy/Caddyfile
EXPOSE 80 443
```

**Step 3: Create `frontend/Caddyfile`**

```
:80 {
    encode gzip

    handle /api/core/* {
        uri strip_prefix /api/core
        reverse_proxy core-service:8000
    }
    handle /api/downloader/* {
        uri strip_prefix /api/downloader
        reverse_proxy downloader-qbittorrent:8002
    }
    handle /api/viewer/* {
        uri strip_prefix /api/viewer
        reverse_proxy viewer-jellyfin:8003
    }

    handle {
        root * /srv
        file_server
        try_files {path} /index.html
    }
}
```

**Step 4: Add to `docker-compose.yaml`**

Add before the `networks:` block:

```yaml
  frontend:
    build:
      context: ./frontend
      dockerfile: Dockerfile
    container_name: bangumi-frontend
    depends_on:
      core-service:
        condition: service_healthy
    ports:
      - "${FRONTEND_PORT:-3000}:80"
    networks:
      bangumi-network:
        ipv4_address: 172.25.0.20
    restart: always
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 128M
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```

**Step 5: Test Docker build**

```bash
cd /workspace
docker compose build frontend
```
Expected: Multi-stage build succeeds

**Step 6: Test full stack**

```bash
docker compose up -d
curl http://localhost:3000/
```
Expected: SPA HTML returned, API proxying works

**Step 7: Commit**

```bash
git add frontend/Dockerfile frontend/Caddyfile frontend/.dockerignore docker-compose.yaml
git commit -m "feat(deploy): add Caddy-based frontend Docker deployment"
```

---

## Task 16: Final — Add all remaining routes to App.tsx, verify E2E

**Files:**
- Modify: `frontend/src/App.tsx` (ensure all routes are wired)

**Step 1: Verify all routes exist in `App.tsx`**

```typescript
<Route index element={<Dashboard />} />
<Route path="anime" element={<AnimePage />} />
<Route path="anime/:animeId" element={<AnimeDetailPage />} />
<Route path="subscriptions" element={<SubscriptionsPage />} />
<Route path="raw-items" element={<RawItemsPage />} />
<Route path="filters" element={<FiltersPage />} />
<Route path="parsers" element={<ParsersPage />} />
<Route path="downloads" element={<DownloadsPage />} />
<Route path="conflicts" element={<ConflictsPage />} />
```

**Step 2: E2E verification**

1. `cargo build -p core-service` — compiles
2. `cd frontend && npm run build` — no errors
3. Start dev stack and verify:
   - Dashboard shows health
   - Navigate all sidebar links
   - Filter preview: type regex → see before/after
   - Parser preview: fill form → see match assignment + parse results
4. `docker compose up --build` — production deployment works

**Step 3: Final commit**

```bash
git add -A
git commit -m "feat(frontend): complete React 19 + Effect-TS frontend with all pages and deployment"
```
