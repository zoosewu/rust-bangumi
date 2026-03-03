# Batch Episode Downstream Impact — Design

**Date:** 2026-03-03
**Scope:** Downloader dispatch deduplication, per-episode file matching, batch_unmatched recovery, frontend export prompt update.

---

## Background

The batch episode (合輯) feature creates one `AnimeLink` per episode from a single RSS torrent item (e.g. ep 01–12 all in one `.torrent`). All 12 links share the same `url` (magnet / .torrent URL).

Two bugs were identified in the downstream pipeline:

1. **Dispatch sends 12 identical requests** to qBittorrent for the same torrent.
2. **`video_file` is set identically for all 12 Download records** (first video file wins), so the viewer receives the wrong file path for every episode except the first.

---

## Problem 1 — Dispatch Deduplication

### Root Cause

`dispatch_new_links` builds one `DownloadRequestItem { url }` per `AnimeLink`. For a 12-episode batch, 12 identical magnets are sent to the downloader. qBittorrent deduplicates internally but Core creates 12 Download records with the same `torrent_hash` anyway.

### Fix

In `download_dispatch.rs`, before building the items list:

1. Group links by `url` → `HashMap<String, Vec<&AnimeLink>>`.
2. Send **one** `DownloadRequestItem` per unique URL.
3. Build a `url → DownloadResultItem` map from the response.
4. For all links sharing a URL, look up the hash and create a `Download` record each with that same `torrent_hash`.

Result: one qBittorrent add per unique torrent, N Download records with the same hash (existing scheduler logic handles N→1 correctly).

---

## Problem 2 — Per-Episode File Matching

### Root Cause

`download_scheduler.rs` completion handler:
```rust
let (video_file, subtitle_files_json) = Self::extract_media_files(&status_item.files);
diesel::update(downloads where torrent_hash = hash)
    .set(video_file = ...)   // same value written to ALL records
```

`extract_media_files` calls `.find(first video)` — one value, bulk-written.

### Fix

Detect batch mode: if more than one Download record shares `(torrent_hash, module_id)`.

**Single-episode mode** (count == 1): unchanged behaviour.

**Batch mode** (count > 1):
1. Join Download records with `anime_links` to get `link_id → episode_no`.
2. Classify `status_item.files` into Video and Subtitle groups.
3. For each file group, extract episode numbers using the **Chain of Responsibility** (see below).
4. Build map: `episode_no → (video_path, subtitle_paths[])`.
5. Update each Download record **individually** with its matched files.
6. If an episode cannot be matched → set `status = "batch_unmatched"`, set `error_message = "unable to match episode file from: {filename}"`, skip viewer sync.

---

## Episode Number Extraction — Chain of Responsibility

Defined in `shared/src/file_classifier.rs`.

```rust
pub trait EpisodeExtractHandler: Send + Sync {
    /// Returns Some(episode_no) or None to pass to the next handler.
    fn extract(&self, stem: &str, expected: &HashSet<i32>) -> Option<i32>;
}
```

Chain (highest priority first):

| # | Handler | Pattern | Example match |
|---|---------|---------|---------------|
| 1 | `ExplicitMarkerHandler` | `(?i)(?:EP?\|第)(\d{1,3})` | `EP01`, `E7`, `第01話` |
| 2 | `DashSeparatorHandler` | `(?:[\s\-_\.])(\d{1,3})(?:[\s\-_\.v])` | `- 07 `, `_07_`, `-07v2` |
| 3 | `IsolatedDigitHandler` | `(?<!\d)(\d{1,3})(?!\d)` ∩ expected | generic fallback |

Each handler:
- Extracts all candidate numbers matching its pattern.
- Parses each as `i32` (strips zero-padding: `"07"` → `7`).
- Intersects with the `expected` set of known episode numbers.
- Returns the value only if the intersection contains **exactly one** element.

Adding new strategies: implement the trait and insert at any position in the chain.

### Public API

```rust
/// Match all files in a completed batch torrent to their episode numbers.
/// Returns: episode_no → (Option<video_path>, Vec<subtitle_paths>)
/// Episodes that cannot be matched are absent from the map.
pub fn match_batch_files(
    files: &[String],
    episode_nos: &[i32],
    chain: &[Box<dyn EpisodeExtractHandler>],
) -> HashMap<i32, (Option<String>, Vec<String>)>
```

Flow:
1. `classify_files` → separate Video and Subtitle groups.
2. For each group, run the chain on each file's stem.
3. Merge into the result map: `episode_no → (video, subs[])`.

---

## Failure State — `batch_unmatched`

A terminal Download status for episodes whose file could not be matched.

- `status = "batch_unmatched"`
- `file_path = content_path` (folder still recorded)
- `video_file = NULL`
- `error_message` = human-readable reason
- No sync triggered.
- Scheduler does **not** retry indefinitely.

### Recovery

**Automatic (scheduler):** A secondary pass in the polling loop queries records where `status = 'batch_unmatched' AND file_path IS NOT NULL`, re-runs `match_batch_files`, and on success transitions to `completed` → triggers viewer sync.

**Manual (future):** API endpoint `POST /downloads/retry-batch-match?torrent_hash={hash}` — re-runs matching for all unmatched records under a given hash. UI button to be added later.

---

## Frontend — Export Prompt Update

`ParserForm.tsx` `handleExportPrompt` generates an AI prompt with a JSON schema description. Add `episode_end` fields:

```
"episode_end_source": "'regex', 'static', or null - end episode for batch torrents (optional)",
"episode_end_value": "string or null - e.g. $3 if parse_regex captures end episode"
```

Add instruction: *"If titles show an episode range (e.g. `01-12`), set `episode_end_source` and `episode_end_value` to capture the upper bound."*

Note: Import (`handleImport`) already handles `episode_end_source/value` — no change needed.

---

## File Change Summary

| File | Change |
|------|--------|
| `shared/src/file_classifier.rs` | Add `EpisodeExtractHandler` trait, 3 handlers, `match_batch_files()` |
| `core-service/src/services/download_dispatch.rs` | URL deduplication before building items |
| `core-service/src/services/download_scheduler.rs` | Batch completion matching; `batch_unmatched` retry pass |
| `frontend/src/components/shared/ParserForm.tsx` | Add `episode_end` to AI export prompt template |

No database schema changes required. `"batch_unmatched"` is a string status value.

---

## Out of Scope

- Manual file assignment UI (future).
- PikPak or other downloader adaptors (same `DownloadStatusItem.files` contract applies).
