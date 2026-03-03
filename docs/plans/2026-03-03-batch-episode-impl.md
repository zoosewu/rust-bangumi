# Batch Episode (合輯) Handling Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow a single RSS torrent item that covers multiple episodes (e.g. `01-12`) to be expanded into individual `AnimeLink` records in the Core service, following the existing `{field}_source/{field}_value` parser pattern.

**Architecture:** Add an optional `episode_end` field to the `TitleParser` model and `ParsedResult`. When both `episode_no` and `episode_end` are present after parsing, `process_parsed_result()` creates one `AnimeLink` per episode in the range, each with a derived `source_hash` of `{original_hash}#ep{n}`. The frontend gains a matching `episode_end` input field in the parser form.

**Tech Stack:** Rust / Diesel / PostgreSQL (backend), React 19 + TypeScript + Effect-TS + Shadcn/UI (frontend)

---

## Task 1: DB Migration — Add `episode_end` columns to `title_parsers`

**Files:**
- Create: `core-service/migrations/2026-03-03-000000-add-episode-end-to-parsers/up.sql`
- Create: `core-service/migrations/2026-03-03-000000-add-episode-end-to-parsers/down.sql`

**Step 1: Create migration files**

`up.sql`:
```sql
ALTER TABLE title_parsers
  ADD COLUMN episode_end_source VARCHAR(20),
  ADD COLUMN episode_end_value  VARCHAR(255);
```

`down.sql`:
```sql
ALTER TABLE title_parsers
  DROP COLUMN IF EXISTS episode_end_source,
  DROP COLUMN IF EXISTS episode_end_value;
```

**Step 2: Run migration**

```bash
cd /workspace/core-service
diesel migration run
```

Expected: `Running migration 2026-03-03-000000-add-episode-end-to-parsers`

**Step 3: Commit**

```bash
git add core-service/migrations/2026-03-03-000000-add-episode-end-to-parsers/
git commit -m "feat(db): add episode_end_source/value columns to title_parsers"
```

---

## Task 2: Update Diesel Schema and Models

**Files:**
- Modify: `core-service/src/schema.rs:253-293` (title_parsers table definition)
- Modify: `core-service/src/models/db.rs:554-611` (TitleParser + NewTitleParser structs)

**Step 1: Regenerate or manually update `schema.rs`**

After running `diesel migration run`, regenerate schema:
```bash
cd /workspace/core-service
diesel print-schema > src/schema.rs
```

Verify the `title_parsers` table now contains (near the bottom of the table block, before `created_at`):
```rust
episode_end_source -> Nullable<Varchar>,
episode_end_value  -> Nullable<Varchar>,
```

**Step 2: Update `TitleParser` struct in `core-service/src/models/db.rs`**

After line `pub year_value: Option<String>,` (currently line ~575), add:
```rust
pub episode_end_source: Option<ParserSourceType>,
pub episode_end_value: Option<String>,
```

**Step 3: Update `NewTitleParser` struct in the same file**

After `pub year_value: Option<String>,` (currently line ~604), add:
```rust
pub episode_end_source: Option<ParserSourceType>,
pub episode_end_value: Option<String>,
```

**Step 4: Verify compilation**

```bash
cd /workspace/core-service
cargo check 2>&1 | head -50
```

Expected: no errors (or only errors from later tasks).

**Step 5: Commit**

```bash
git add core-service/src/schema.rs core-service/src/models/db.rs
git commit -m "feat(model): add episode_end_source/value to TitleParser model"
```

---

## Task 3: Update `ParsedResult` and Extraction Logic

**Files:**
- Modify: `core-service/src/services/title_parser.rs:12-23` (ParsedResult struct)
- Modify: `core-service/src/services/title_parser.rs:78-153` (try_parser function)

**Step 1: Write a failing test**

Add to the bottom of `core-service/src/services/title_parser.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::db::{TitleParser, ParserSourceType};
    use chrono::Utc;

    fn make_batch_parser() -> TitleParser {
        let now = Utc::now().naive_utc();
        TitleParser {
            parser_id: 1,
            name: "batch_test".to_string(),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: r"\d+-\d+".to_string(),
            parse_regex: r"^(?P<title>.+?)\s+(?P<ep_start>\d+)-(?P<ep_end>\d+)".to_string(),
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "$title".to_string(),
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "$ep_start".to_string(),
            episode_end_source: Some(ParserSourceType::Regex),
            episode_end_value: Some("$ep_end".to_string()),
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
        }
    }

    #[test]
    fn test_try_parser_extracts_episode_end() {
        let parser = make_batch_parser();
        let title = "動畫名 01-12 [1080p]";
        let result = TitleParserService::try_parser(&parser, title).unwrap().unwrap();

        assert_eq!(result.episode_no, 1);
        assert_eq!(result.episode_end, Some(12));
        assert_eq!(result.anime_title, "動畫名");
    }

    #[test]
    fn test_try_parser_episode_end_none_for_single_episode() {
        let now = chrono::Utc::now().naive_utc();
        let parser = TitleParser {
            parser_id: 2,
            name: "single_test".to_string(),
            description: None,
            priority: 50,
            is_enabled: true,
            condition_regex: ".*".to_string(),
            parse_regex: r"^(?P<title>.+?)\s+(?P<ep>\d+)".to_string(),
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "$title".to_string(),
            episode_no_source: ParserSourceType::Regex,
            episode_no_value: "$ep".to_string(),
            episode_end_source: None,
            episode_end_value: None,
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
        };
        let title = "動畫名 05 [1080p]";
        let result = TitleParserService::try_parser(&parser, title).unwrap().unwrap();

        assert_eq!(result.episode_no, 5);
        assert_eq!(result.episode_end, None);
    }
}
```

**Step 2: Run test to verify it fails**

```bash
cd /workspace/core-service
cargo test test_try_parser_extracts_episode_end 2>&1 | tail -20
```

Expected: compile error — `episode_end` field not found in `ParsedResult` / `TitleParser`.

**Step 3: Add `episode_end` to `ParsedResult`**

In `title_parser.rs`, modify the struct (lines 12-23):
```rust
pub struct ParsedResult {
    pub anime_title: String,
    pub episode_no: i32,
    pub episode_end: Option<i32>,  // None = single episode; Some(n) = batch end
    pub series_no: i32,
    pub subtitle_group: Option<String>,
    pub resolution: Option<String>,
    pub season: Option<String>,
    pub year: Option<String>,
    pub parser_id: i32,
}
```

**Step 4: Update `try_parser` to extract `episode_end`**

In `try_parser` (lines 78-153), after the block that extracts optional fields (`subtitle_group`, `resolution`, etc.), add:

```rust
// Extract episode_end (optional range end for batch torrents)
let episode_end = if parser.episode_end_source.is_some() {
    match Self::extract_optional_value(
        &parser.episode_end_source,
        &parser.episode_end_value,
        &captures,
    ) {
        Some(v) => v.parse::<i32>().ok(),
        None => None,
    }
} else {
    None
};
```

Then update the returned `ParsedResult` literal to include:
```rust
episode_end,
```

Also update the final `ParsedResult` construction block to include the field (it must be in the struct literal).

**Step 5: Run tests to verify they pass**

```bash
cd /workspace/core-service
cargo test test_try_parser 2>&1 | tail -20
```

Expected: both tests PASS.

**Step 6: Commit**

```bash
git add core-service/src/services/title_parser.rs
git commit -m "feat(parser): add episode_end extraction to ParsedResult and try_parser"
```

---

## Task 4: Update `process_parsed_result()` for Batch Expansion

**Files:**
- Modify: `core-service/src/handlers/fetcher_results.rs:651-727` (process_parsed_result)
- Modify: `core-service/src/handlers/fetcher_results.rs:520` (caller site)

**Step 1: Write a failing test**

> Note: `process_parsed_result` is an integration-level function requiring a DB connection. Write a unit test for the helper logic by extracting it into a small testable function.

Add a test that verifies the `source_hash` derivation logic:

```rust
#[cfg(test)]
mod batch_tests {
    #[test]
    fn test_batch_source_hash_format() {
        // Hash is SHA256 of the URL
        use sha2::{Digest, Sha256};
        let url = "magnet:?xt=urn:btih:abc123";
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let base_hash = format!("{:x}", hasher.finalize());

        let ep5_hash = format!("{}#ep5", base_hash);
        let ep12_hash = format!("{}#ep12", base_hash);

        assert!(ep5_hash.ends_with("#ep5"));
        assert!(ep12_hash.ends_with("#ep12"));
        assert_ne!(ep5_hash, ep12_hash);
    }
}
```

**Step 2: Run to verify it passes (pure logic, no DB needed)**

```bash
cd /workspace/core-service
cargo test test_batch_source_hash_format 2>&1 | tail -10
```

Expected: PASS.

**Step 3: Change `process_parsed_result` return type and implement expansion**

Replace the current function signature and body:

```rust
pub(crate) fn process_parsed_result(
    conn: &mut PgConnection,
    raw_item: &RawAnimeItem,
    parsed: &crate::services::title_parser::ParsedResult,
) -> Result<Vec<i32>, String> {
    use sha2::{Digest, Sha256};

    // 1-4: same as before — get work, season, anime, group
    let work = create_or_get_anime(conn, &parsed.anime_title)?;
    let year = parsed.year.as_ref().and_then(|y| y.parse::<i32>().ok()).unwrap_or(2025);
    let season_name = parsed.season.as_deref().unwrap_or("unknown");
    let season = create_or_get_season(conn, year, season_name)?;
    let anime = create_or_get_series(conn, work.work_id, parsed.series_no, season.season_id, "")?;
    let group_name = parsed.subtitle_group.as_deref().unwrap_or("未知字幕組");
    let group = create_or_get_subtitle_group(conn, group_name)?;

    // 5: compute base hash from URL
    let mut hasher = Sha256::new();
    hasher.update(raw_item.download_url.as_bytes());
    let base_hash = format!("{:x}", hasher.finalize());

    // Determine episode range
    let ep_start = parsed.episode_no;
    let ep_end = match parsed.episode_end {
        Some(end) if end >= ep_start && (end - ep_start) <= 200 => end,
        Some(bad) => {
            tracing::warn!(
                "episode_end ({}) is invalid relative to episode_no ({}), treating as single episode",
                bad, ep_start
            );
            ep_start
        }
        None => ep_start,
    };

    let is_batch = ep_end > ep_start;
    let now = Utc::now().naive_utc();
    let detected_type =
        crate::services::download_type_detector::detect_download_type(&raw_item.download_url);
    let mut link_ids = Vec::new();

    for ep in ep_start..=ep_end {
        let source_hash = if is_batch {
            format!("{}#ep{}", base_hash, ep)
        } else {
            base_hash.clone()
        };

        let new_link = NewAnimeLink {
            anime_id: anime.anime_id,
            group_id: group.group_id,
            episode_no: ep,
            title: Some(raw_item.title.clone()),
            url: raw_item.download_url.clone(),
            source_hash,
            filtered_flag: false,
            created_at: now,
            raw_item_id: Some(raw_item.item_id),
            download_type: detected_type.as_ref().map(|dt| dt.to_string()),
            conflict_flag: false,
            link_status: "active".to_string(),
        };

        let created_link: AnimeLink = diesel::insert_into(anime_links::table)
            .values(&new_link)
            .get_result(conn)
            .map_err(|e| format!("Failed to create anime link ep {}: {}", ep, e))?;

        match crate::services::filter_recalc::compute_filtered_flag_for_link(conn, &created_link) {
            Ok(flag) if flag != created_link.filtered_flag => {
                diesel::update(anime_links::table.filter(anime_links::link_id.eq(created_link.link_id)))
                    .set(anime_links::filtered_flag.eq(flag))
                    .execute(conn)
                    .map_err(|e| format!("Failed to update filtered_flag: {}", e))?;
            }
            Err(e) => {
                tracing::warn!("Failed to compute filtered_flag for link {}: {}", created_link.link_id, e);
            }
            _ => {}
        }

        link_ids.push(created_link.link_id);
    }

    Ok(link_ids)
}
```

**Step 4: Update the caller at line 520**

Find the call site in `receive_raw_fetcher_results`. Currently:
```rust
match process_parsed_result(&mut conn, &saved_item, &parsed) {
    Ok(link_id) => {
        new_link_ids.push(link_id);
```

Change to:
```rust
match process_parsed_result(&mut conn, &saved_item, &parsed) {
    Ok(ids) => {
        new_link_ids.extend(ids);
```

**Step 5: Verify compilation**

```bash
cd /workspace/core-service
cargo check 2>&1 | head -50
```

Expected: no errors.

**Step 6: Commit**

```bash
git add core-service/src/handlers/fetcher_results.rs
git commit -m "feat(core): expand batch episode torrents into multiple AnimeLinks"
```

---

## Task 5: Update Handlers for Parser Create/Update — Pass New Fields

**Files:**
- Modify: handler that creates/updates `TitleParser` (find with `grep -n "NewTitleParser" core-service/src/ -r`)

**Step 1: Find the create/update parser handler**

```bash
grep -rn "NewTitleParser {" /workspace/core-service/src/
```

**Step 2: Add `episode_end_source` and `episode_end_value` fields**

In the `NewTitleParser { ... }` construction block, add:
```rust
episode_end_source: body.get("episode_end_source")
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse().ok()),
episode_end_value: body.get("episode_end_value")
    .and_then(|v| v.as_str())
    .map(|s| s.to_string()),
```

For the update handler, similarly extract these fields from the request body.

**Step 3: Verify compilation**

```bash
cd /workspace/core-service
cargo check 2>&1 | head -50
```

Expected: no errors.

**Step 4: Commit**

```bash
git add core-service/src/handlers/
git commit -m "feat(api): pass episode_end fields through parser create/update handlers"
```

---

## Task 6: Update Frontend Schema (`parser.ts`)

**Files:**
- Modify: `frontend/src/schemas/parser.ts`

**Step 1: Add `episode_end_source` and `episode_end_value` to `TitleParser` schema**

In the `TitleParser` Schema.Struct, after `episode_no_value`:
```typescript
episode_end_source: Schema.NullOr(Schema.String),
episode_end_value: Schema.NullOr(Schema.String),
```

**Step 2: Add `episode_end` to `ParsedFields`**

In the `ParsedFields` Schema.Struct, after `episode_no`:
```typescript
episode_end: Schema.NullOr(Schema.Number),
```

**Step 3: Verify TypeScript compiles**

```bash
cd /workspace/frontend
npm run build 2>&1 | grep -E "error|Error" | head -20
```

Expected: no type errors related to parser schema.

**Step 4: Commit**

```bash
git add frontend/src/schemas/parser.ts
git commit -m "feat(frontend): add episode_end fields to TitleParser and ParsedFields schemas"
```

---

## Task 7: Update `ParserFormState` and Default Values

**Files:**
- Modify: `frontend/src/components/shared/ParserForm.tsx:29-69`

**Step 1: Add fields to `ParserFormState` type (line 29-48)**

After `episode_no_value: string`, add:
```typescript
episode_end_source: string | null
episode_end_value: string | null
```

**Step 2: Add fields to `EMPTY_PARSER_FORM` (line 50-69)**

After `episode_no_value: ""`, add:
```typescript
episode_end_source: null,
episode_end_value: null,
```

**Step 3: Verify TypeScript compiles**

```bash
cd /workspace/frontend
npm run build 2>&1 | grep -E "error|Error" | head -20
```

Expected: errors will appear at any place that constructs a `ParserFormState` without the new fields — fix those as needed (usually just the parser-to-form mappers).

**Step 4: Fix any `ParserFormState` construction sites**

Search for places that build a `ParserFormState` from an existing `TitleParser`:
```bash
grep -rn "episode_no_value" /workspace/frontend/src/
```

In each mapper function (e.g. `parserToForm` or similar), add:
```typescript
episode_end_source: parser.episode_end_source ?? null,
episode_end_value: parser.episode_end_value ?? null,
```

**Step 5: Commit**

```bash
git add frontend/src/components/shared/ParserForm.tsx
git commit -m "feat(frontend): add episode_end to ParserFormState"
```

---

## Task 8: Add Episode End UI Field to `ParserFormFields`

**Files:**
- Modify: `frontend/src/components/shared/ParserForm.tsx` (ParserFormFields component, lines ~219-292)

**Step 1: Find the Episode No FieldSourceInput block (line ~228-235)**

Current:
```tsx
<FieldSourceInput
  label={t("parsers.episodeNo", "Episode No")}
  source={form.episode_no_source}
  value={form.episode_no_value}
  onSourceChange={(v) => onChange("episode_no_source", v)}
  onValueChange={(v) => onChange("episode_no_value", v)}
  required
/>
```

**Step 2: Add Episode End field directly after Episode No**

```tsx
<FieldSourceInput
  label={t("parsers.episodeEnd", "Episode End")}
  source={form.episode_end_source}
  value={form.episode_end_value ?? ""}
  onSourceChange={(v) => onChange("episode_end_source", v === "none" ? null : v)}
  onValueChange={(v) => onChange("episode_end_value", v)}
/>
```

**Step 3: Verify in browser (or build)**

```bash
cd /workspace/frontend
npm run build 2>&1 | grep -E "error|Error" | head -20
```

Expected: no type errors.

**Step 4: Commit**

```bash
git add frontend/src/components/shared/ParserForm.tsx
git commit -m "feat(frontend): add Episode End field to parser form UI"
```

---

## Task 9: Update Preview Display for Episode Range

**Files:**
- Modify: `frontend/src/pages/parsers/ParsersPage.tsx` (PreviewResults component)
- Possibly: `frontend/src/components/shared/ParserEditor.tsx`

**Step 1: Find where `episode_no` is displayed in preview results**

```bash
grep -n "episode_no" /workspace/frontend/src/pages/parsers/ParsersPage.tsx
grep -n "episode_no" /workspace/frontend/src/components/shared/ParserEditor.tsx
```

**Step 2: Update episode display to show range**

Find the code like:
```tsx
<span>{result.parse_result?.episode_no}</span>
```
or
```tsx
EP {result.parse_result.episode_no}
```

Change to:
```tsx
{result.parse_result?.episode_end != null
  ? `EP ${result.parse_result.episode_no}–${result.parse_result.episode_end}`
  : `EP ${result.parse_result?.episode_no}`}
```

**Step 3: Verify build**

```bash
cd /workspace/frontend
npm run build 2>&1 | grep -E "error|Error" | head -20
```

Expected: no errors.

**Step 4: Commit**

```bash
git add frontend/src/pages/parsers/ParsersPage.tsx frontend/src/components/shared/ParserEditor.tsx
git commit -m "feat(frontend): show episode range in parser preview when episode_end is set"
```

---

## Task 10: Final Verification

**Step 1: Run all backend tests**

```bash
cd /workspace/core-service
cargo test 2>&1 | tail -30
```

Expected: all tests pass.

**Step 2: Run full backend build**

```bash
cd /workspace/core-service
cargo build 2>&1 | tail -10
```

Expected: `Finished` with no errors.

**Step 3: Run frontend build**

```bash
cd /workspace/frontend
npm run build 2>&1 | tail -10
```

Expected: `build complete` with no errors.

**Step 4: Run workspace-level check**

```bash
cd /workspace
cargo check --workspace 2>&1 | tail -20
```

Expected: no errors.

**Step 5: Final commit (if any cleanup needed)**

```bash
git add -p
git commit -m "chore: final cleanup for batch episode feature"
```
