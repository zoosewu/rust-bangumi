# Batch Episode (合輯) Handling Design

**Date**: 2026-03-03
**Status**: Approved

## Problem

When a fetcher RSS item represents a batch torrent (e.g., `[字幕組] 動畫名 01-12 [1080p]`), the current system
only creates one `AnimeLink` with a single `episode_no`. There is no mechanism to expand a batch torrent into
individual episode records.

## Goal

When a `RawAnimeItem` title indicates a range of episodes, the Core service should expand it into multiple
`AnimeLink` records — one per episode — all pointing to the same torrent URL. Existing conflict detection
(`conflict_flag`) handles cases where a single-episode torrent is later found for an already-covered episode.

## Design

### Approach: Extend Parser with `episode_end` Field

Follow the existing `{field}_source` / `{field}_value` pattern already used by all parser fields. Add an optional
`episode_end` field that, when set, triggers batch expansion.

This is a purely additive, opt-in change. Existing parsers require no modification.

### Backend Changes

#### 1. Database Migration

Add two nullable columns to `title_parsers`:

```sql
ALTER TABLE title_parsers
  ADD COLUMN episode_end_source VARCHAR(20),
  ADD COLUMN episode_end_value  VARCHAR(255);
```

#### 2. `TitleParser` Model (`core-service/src/models/db.rs`)

```rust
pub struct TitleParser {
    // ... existing fields ...
    pub episode_end_source: Option<String>,
    pub episode_end_value:  Option<String>,
}
```

#### 3. `ParsedResult` (`core-service/src/services/title_parser.rs`)

```rust
pub struct ParsedResult {
    pub anime_title:    String,
    pub episode_no:     i32,
    pub episode_end:    Option<i32>,   // None = single episode
    pub series_no:      i32,
    pub subtitle_group: Option<String>,
    pub resolution:     Option<String>,
    pub season:         Option<String>,
    pub year:           Option<String>,
    pub parser_id:      i32,
}
```

#### 4. Title Parser Service (`title_parser.rs`)

Extract `episode_end` using the same logic as `episode_no` (supports both `regex` and `static` sources).

#### 5. `process_parsed_result()` (`handlers/fetcher_results.rs`)

- Change return type from `Result<i32, String>` to `Result<Vec<i32>, String>`.
- If `parsed.episode_end.is_none()`: create one `AnimeLink` as before.
- If `parsed.episode_end.is_some()`: loop from `episode_no` to `episode_end` (inclusive), creating one
  `AnimeLink` per episode. Each link uses:
  - `episode_no`: the current loop value
  - `source_hash`: `{original_hash}#ep{n}` (ensures idempotency on re-processing)
  - `url`: unchanged (all episodes share the same torrent URL)
  - `raw_item_id`: same for all derived links
- Update all callers to handle `Vec<i32>`.

### Frontend Changes

#### 1. `ParserFormState` (`components/shared/ParserForm.tsx`)

```typescript
export type ParserFormState = {
  // ... existing fields ...
  episode_end_source: string | null
  episode_end_value:  string | null
}
```

#### 2. `ParserFormFields` Component

Add an "Episode End" `FieldSourceInput` in the same row as "Episode No".

#### 3. Preview Results

When `episode_end` is present in the parsed result, display the range:
- Before: `EP 1`
- After:  `EP 1–12`

#### 4. `schemas/parser.ts`

Add `episode_end_source` and `episode_end_value` as optional fields in the TypeScript schema.

## Example

Parser configuration for batch torrents:

```
condition_regex:  \d{2,3}-\d{2,3}
parse_regex:      ^.+?\s+(?P<ep_start>\d+)-(?P<ep_end>\d+).*$
episode_no:       source=regex, value=$ep_start
episode_end:      source=regex, value=$ep_end
```

Input title: `[字幕組] 動畫名 01-12 [1080p]`

Output: 12 `AnimeLink` records, `episode_no` = 1…12, all with the same `url`, `source_hash` = `{hash}#ep1` … `{hash}#ep12`.

## Conflict Handling

No changes needed. The existing `conflict_flag` mechanism (GROUP BY `anime_id`, `group_id`, `episode_no`
HAVING COUNT > 1) automatically detects when a single-episode torrent duplicates an episode already covered by
a batch record.

## Constraints

- `episode_end` must be ≥ `episode_no`; if not, treat as single-episode (log a warning).
- Maximum batch size: enforce a reasonable cap (e.g., 200 episodes) to guard against regex mismatches.
- `source_hash` uniqueness: the `#ep{n}` suffix ensures re-processing the same RSS feed is idempotent.
