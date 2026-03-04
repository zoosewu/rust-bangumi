# Batch Episode Dispatch Dedup & File Matching Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix two bugs in the batch episode pipeline: (1) dispatch sends duplicate torrent requests for each episode in a batch; (2) all episodes in a batch receive the same `video_file` path instead of their individual file.

**Architecture:** Add a Chain-of-Responsibility episode extractor to `shared`, use it in `download_scheduler` to match each `Download` record to its specific file when a batch torrent completes. Deduplicate `DownloadRequestItem`s by URL in `download_dispatch` before sending to qBittorrent. Fallback status `"batch_unmatched"` is terminal but retried on each polling cycle.

**Tech Stack:** Rust / Diesel / PostgreSQL (backend), React 19 + TypeScript (frontend `ParserForm.tsx` AI prompt only)

---

## Task 1: Add `regex` to `shared` and implement `EpisodeExtractHandler` chain

**Files:**
- Modify: `shared/Cargo.toml`
- Modify: `shared/src/file_classifier.rs`
- Modify: `shared/src/lib.rs`

**Step 1: Add `regex` workspace dependency to `shared/Cargo.toml`**

In `shared/Cargo.toml`, under `[dependencies]`, add:
```toml
regex.workspace = true
```

**Step 2: Write failing tests for episode extraction**

At the bottom of `shared/src/file_classifier.rs` (before the closing `}`), add a new test module:

```rust
#[cfg(test)]
mod episode_tests {
    use super::*;
    use std::collections::HashSet;

    fn expected(range: std::ops::RangeInclusive<i32>) -> HashSet<i32> {
        range.collect()
    }

    fn chain() -> Vec<Box<dyn EpisodeExtractHandler>> {
        build_default_chain()
    }

    // ExplicitMarkerHandler tests
    #[test]
    fn test_explicit_ep_prefix() {
        let ep = extract_episode_from_stem("Show EP07 [1080p]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    #[test]
    fn test_explicit_e_prefix_uppercase() {
        let ep = extract_episode_from_stem("Show E07 [1080p]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    #[test]
    fn test_explicit_cjk_marker() {
        let ep = extract_episode_from_stem("Show 第07話 [1080p]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    // DashSeparatorHandler tests
    #[test]
    fn test_dash_separator() {
        let ep = extract_episode_from_stem("[Group] Show - 07 [1080p][ABCD]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    #[test]
    fn test_dash_separator_version_suffix() {
        // "07v2" — the v2 should not block matching
        let ep = extract_episode_from_stem("[Group] Show - 07v2 [1080p]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    // IsolatedDigitHandler tests
    #[test]
    fn test_isolated_digit_fallback() {
        let ep = extract_episode_from_stem("show.07.mkv.stem", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    // Ambiguity → None
    #[test]
    fn test_ambiguous_returns_none() {
        // "02" and "07" both in expected range — ambiguous
        let ep = extract_episode_from_stem("S02E07", &expected(1..=12), &chain());
        assert_eq!(ep, None);
    }

    // Out of range → None
    #[test]
    fn test_out_of_range_ignored() {
        // Only number is 1080 which is not in expected range
        let ep = extract_episode_from_stem("show [1080p]", &expected(1..=12), &chain());
        assert_eq!(ep, None);
    }

    // Zero-padded parsing
    #[test]
    fn test_zero_padded_parsed_correctly() {
        let ep = extract_episode_from_stem("[Group] Show - 01 [720p]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(1));
    }

    // match_batch_files tests
    #[test]
    fn test_match_batch_files_video_and_subtitle() {
        let files = vec![
            "/dl/Show/[G] Show - 01 [1080p].mkv".to_string(),
            "/dl/Show/[G] Show - 01 [1080p].zh.ass".to_string(),
            "/dl/Show/[G] Show - 02 [1080p].mkv".to_string(),
            "/dl/Show/[G] Show - 02 [1080p].zh.ass".to_string(),
        ];
        let chain = build_default_chain();
        let result = match_batch_files(&files, &[1, 2], &chain);

        assert_eq!(result.get(&1).unwrap().0.as_deref(), Some("/dl/Show/[G] Show - 01 [1080p].mkv"));
        assert_eq!(result.get(&1).unwrap().1, vec!["/dl/Show/[G] Show - 01 [1080p].zh.ass"]);
        assert_eq!(result.get(&2).unwrap().0.as_deref(), Some("/dl/Show/[G] Show - 02 [1080p].mkv"));
    }

    #[test]
    fn test_match_batch_files_unmatched_episode_absent() {
        // ep 3 has no corresponding file
        let files = vec![
            "/dl/Show - 01.mkv".to_string(),
            "/dl/Show - 02.mkv".to_string(),
        ];
        let chain = build_default_chain();
        let result = match_batch_files(&files, &[1, 2, 3], &chain);

        assert!(result.contains_key(&1));
        assert!(result.contains_key(&2));
        assert!(!result.contains_key(&3));
    }

    #[test]
    fn test_match_batch_files_single_episode_not_confused() {
        // Only one expected episode → no ambiguity
        let files = vec![
            "/dl/Show - 05 [1080p].mkv".to_string(),
        ];
        let chain = build_default_chain();
        let result = match_batch_files(&files, &[5], &chain);
        assert_eq!(result.get(&5).unwrap().0.as_deref(), Some("/dl/Show - 05 [1080p].mkv"));
    }
}
```

**Step 3: Run tests to verify they fail**

```bash
cd /workspace
cargo test -p shared episode_tests 2>&1 | tail -20
```

Expected: compile errors — `EpisodeExtractHandler`, `build_default_chain`, `extract_episode_from_stem`, `match_batch_files` not defined.

**Step 4: Implement the chain of responsibility**

In `shared/src/file_classifier.rs`, add the following **before** the `#[cfg(test)]` block:

```rust
use regex::Regex;
use std::collections::{HashMap, HashSet};

/// Trait for episode number extraction strategies (Chain of Responsibility).
pub trait EpisodeExtractHandler: Send + Sync {
    /// Try to extract an episode number from `stem`.
    /// Returns `Some(n)` only if exactly one candidate is found in `expected`.
    /// Returns `None` to pass to the next handler.
    fn extract(&self, stem: &str, expected: &HashSet<i32>) -> Option<i32>;
}

fn unique_match(candidates: Vec<i32>, expected: &HashSet<i32>) -> Option<i32> {
    let matches: Vec<i32> = candidates
        .into_iter()
        .filter(|n| expected.contains(n))
        .collect();
    if matches.len() == 1 { Some(matches[0]) } else { None }
}

/// Handler 1: explicit markers — EP01, E01, 第01話
pub struct ExplicitMarkerHandler;

impl EpisodeExtractHandler for ExplicitMarkerHandler {
    fn extract(&self, stem: &str, expected: &HashSet<i32>) -> Option<i32> {
        let re = Regex::new(r"(?i)(?:EP?|第)(\d{1,3})").unwrap();
        let candidates = re
            .captures_iter(stem)
            .filter_map(|c| c[1].parse::<i32>().ok())
            .collect();
        unique_match(candidates, expected)
    }
}

/// Handler 2: separator-bounded numbers — "- 07 ", "_07_", "-07v2"
pub struct DashSeparatorHandler;

impl EpisodeExtractHandler for DashSeparatorHandler {
    fn extract(&self, stem: &str, expected: &HashSet<i32>) -> Option<i32> {
        let re = Regex::new(r"(?:[\s\-_\.])(\d{1,3})(?:[\s\-_\.v]|$)").unwrap();
        let candidates = re
            .captures_iter(stem)
            .filter_map(|c| c[1].parse::<i32>().ok())
            .collect();
        unique_match(candidates, expected)
    }
}

/// Handler 3: any isolated 1–3 digit sequence not adjacent to other digits
pub struct IsolatedDigitHandler;

impl EpisodeExtractHandler for IsolatedDigitHandler {
    fn extract(&self, stem: &str, expected: &HashSet<i32>) -> Option<i32> {
        let re = Regex::new(r"(?<!\d)(\d{1,3})(?!\d)").unwrap();
        let candidates = re
            .captures_iter(stem)
            .filter_map(|c| c[1].parse::<i32>().ok())
            .collect();
        unique_match(candidates, expected)
    }
}

/// Build the default three-handler chain (ExplicitMarker → DashSeparator → IsolatedDigit).
pub fn build_default_chain() -> Vec<Box<dyn EpisodeExtractHandler>> {
    vec![
        Box::new(ExplicitMarkerHandler),
        Box::new(DashSeparatorHandler),
        Box::new(IsolatedDigitHandler),
    ]
}

/// Walk the chain until a handler returns Some, otherwise return None.
pub fn extract_episode_from_stem(
    stem: &str,
    expected: &HashSet<i32>,
    chain: &[Box<dyn EpisodeExtractHandler>],
) -> Option<i32> {
    chain.iter().find_map(|h| h.extract(stem, expected))
}

/// Match all files in a completed batch torrent to their episode numbers.
///
/// Returns a map of `episode_no → (Option<video_path>, Vec<subtitle_paths>)`.
/// Episodes whose file cannot be uniquely identified are absent from the map.
pub fn match_batch_files(
    files: &[String],
    episode_nos: &[i32],
    chain: &[Box<dyn EpisodeExtractHandler>],
) -> HashMap<i32, (Option<String>, Vec<String>)> {
    let expected: HashSet<i32> = episode_nos.iter().copied().collect();
    let classified = classify_files(files.to_vec());
    let mut result: HashMap<i32, (Option<String>, Vec<String>)> = HashMap::new();

    for mf in classified.iter().filter(|f| f.file_type == FileType::Video) {
        let stem = std::path::Path::new(&mf.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if let Some(ep) = extract_episode_from_stem(stem, &expected, chain) {
            result.entry(ep).or_default().0 = Some(mf.path.clone());
        }
    }

    for mf in classified.iter().filter(|f| f.file_type == FileType::Subtitle) {
        let stem = std::path::Path::new(&mf.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if let Some(ep) = extract_episode_from_stem(stem, &expected, chain) {
            result.entry(ep).or_default().1.push(mf.path.clone());
        }
    }

    result
}
```

**Step 5: Export new symbols from `shared/src/lib.rs`**

Change the existing re-export line:
```rust
pub use file_classifier::{
    classify_files, collect_files_recursive, extract_language_tag, FileType, LanguageCodeMap,
};
```
To:
```rust
pub use file_classifier::{
    build_default_chain, classify_files, collect_files_recursive, extract_episode_from_stem,
    extract_language_tag, match_batch_files, EpisodeExtractHandler, FileType, LanguageCodeMap,
};
```

**Step 6: Run tests to verify they pass**

```bash
cd /workspace
cargo test -p shared episode_tests 2>&1 | tail -20
```

Expected: all episode_tests pass.

**Step 7: Commit**

```bash
git add shared/Cargo.toml shared/src/file_classifier.rs shared/src/lib.rs
git commit -m "feat(shared): add EpisodeExtractHandler chain and match_batch_files for batch torrent file matching"
```

---

## Task 2: Dispatch URL deduplication

**Files:**
- Modify: `core-service/src/services/download_dispatch.rs`

**Context:** Currently `dispatch_new_links` creates one `DownloadRequestItem` per `AnimeLink`. For a 12-episode batch torrent, 12 identical magnet URLs are sent. The fix: deduplicate by URL before sending, map results back to all links sharing a URL.

**Step 1: Write a unit test for the URL grouping helper**

Add this to the bottom of `download_dispatch.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::group_links_by_url;
    use crate::models::AnimeLink;
    use chrono::Utc;

    fn make_link(id: i32, url: &str) -> AnimeLink {
        let now = Utc::now().naive_utc();
        AnimeLink {
            link_id: id,
            anime_id: 1,
            group_id: 1,
            episode_no: id,
            title: None,
            url: url.to_string(),
            source_hash: format!("hash{}", id),
            filtered_flag: false,
            created_at: now,
            raw_item_id: None,
            download_type: Some("magnet".to_string()),
            conflict_flag: false,
            link_status: "active".to_string(),
        }
    }

    #[test]
    fn test_group_links_by_url_deduplicates() {
        let l1 = make_link(1, "magnet:?xt=urn:btih:ABC");
        let l2 = make_link(2, "magnet:?xt=urn:btih:ABC");
        let l3 = make_link(3, "magnet:?xt=urn:btih:DEF");
        let refs = vec![&l1, &l2, &l3];

        let groups = group_links_by_url(&refs);

        assert_eq!(groups.len(), 2, "should have 2 unique URLs");
        let abc_group = groups.iter().find(|(url, _)| url == "magnet:?xt=urn:btih:ABC").unwrap();
        assert_eq!(abc_group.1.len(), 2);
        let def_group = groups.iter().find(|(url, _)| url == "magnet:?xt=urn:btih:DEF").unwrap();
        assert_eq!(def_group.1.len(), 1);
    }

    #[test]
    fn test_group_links_by_url_preserves_order() {
        let l1 = make_link(1, "magnet:?xt=urn:btih:AAA");
        let l2 = make_link(2, "magnet:?xt=urn:btih:BBB");
        let refs = vec![&l1, &l2];

        let groups = group_links_by_url(&refs);
        assert_eq!(groups[0].0, "magnet:?xt=urn:btih:AAA");
        assert_eq!(groups[1].0, "magnet:?xt=urn:btih:BBB");
    }
}
```

**Step 2: Run to verify it fails**

```bash
cd /workspace/core-service
cargo test test_group_links_by_url 2>&1 | tail -10
```

Expected: compile error — `group_links_by_url` not defined.

**Step 3: Add `group_links_by_url` helper function**

Inside `impl DownloadDispatchService { ... }` (before the closing `}`), add:

```rust
/// Group a slice of links by URL, preserving insertion order.
/// Each unique URL maps to all links sharing that URL.
fn group_links_by_url<'a>(links: &[&'a AnimeLink]) -> Vec<(String, Vec<&'a AnimeLink>)> {
    let mut groups: Vec<(String, Vec<&'a AnimeLink>)> = Vec::new();
    for link in links {
        if let Some(g) = groups.iter_mut().find(|(url, _)| url == &link.url) {
            g.1.push(link);
        } else {
            groups.push((link.url.clone(), vec![link]));
        }
    }
    groups
}
```

**Step 4: Run to verify helper test passes**

```bash
cd /workspace/core-service
cargo test test_group_links_by_url 2>&1 | tail -10
```

Expected: both tests PASS.

**Step 5: Replace item-building in Phase 1 (preferred downloader)**

Locate lines ~175-204 (the `for (pref_id, pref_links) in &by_preferred { ... }` block).

Replace:
```rust
let items: Vec<DownloadRequestItem> = pref_links
    .iter()
    .map(|link| DownloadRequestItem {
        url: link.url.clone(),
        save_path: "/downloads".to_string(),
    })
    .collect();

let download_url = format!("{}/downloads", pref_dl.base_url);
match self.send_batch_to_downloader(&download_url, items).await {
    Ok(response) => {
        for (i, result) in response.results.iter().enumerate() {
            if i >= pref_links.len() {
                break;
            }
            let link = pref_links[i];
            if result.status == "accepted" {
                self.create_download_record(
                    &mut conn,
                    link.link_id,
                    &download_type,
                    "downloading",
                    Some(pref_dl.module_id),
                    result.hash.as_deref(),
                )?;
                total_dispatched += 1;
            } else {
                // rejected by preferred — fallback to cascade
                cascade_pending.push(link);
            }
        }
    }
    Err(e) => {
        tracing::error!(
            "Preferred downloader {} failed for {} links: {}",
            pref_dl.name,
            pref_links.len(),
            e
        );
        cascade_pending.extend(pref_links.iter().copied());
    }
}
```

With:
```rust
let url_groups = Self::group_links_by_url(pref_links);
let items: Vec<DownloadRequestItem> = url_groups
    .iter()
    .map(|(url, _)| DownloadRequestItem {
        url: url.clone(),
        save_path: "/downloads".to_string(),
    })
    .collect();

let download_url = format!("{}/downloads", pref_dl.base_url);
match self.send_batch_to_downloader(&download_url, items).await {
    Ok(response) => {
        for (i, result) in response.results.iter().enumerate() {
            if i >= url_groups.len() {
                break;
            }
            let (_, links_for_url) = &url_groups[i];
            for link in links_for_url {
                if result.status == "accepted" {
                    self.create_download_record(
                        &mut conn,
                        link.link_id,
                        &download_type,
                        "downloading",
                        Some(pref_dl.module_id),
                        result.hash.as_deref(),
                    )?;
                    total_dispatched += 1;
                } else {
                    cascade_pending.push(link);
                }
            }
        }
    }
    Err(e) => {
        tracing::error!(
            "Preferred downloader {} failed for {} links: {}",
            pref_dl.name,
            pref_links.len(),
            e
        );
        cascade_pending.extend(pref_links.iter().copied());
    }
}
```

**Step 6: Replace item-building in Phase 2 (cascade)**

Locate lines ~227-273 (the `for downloader in &downloaders { ... }` block).

Replace:
```rust
let items: Vec<DownloadRequestItem> = pending_links
    .iter()
    .map(|link| DownloadRequestItem {
        url: link.url.clone(),
        save_path: "/downloads".to_string(),
    })
    .collect();

let download_url = format!("{}/downloads", downloader.base_url);
match self.send_batch_to_downloader(&download_url, items).await {
    Ok(response) => {
        let mut rejected = Vec::new();

        for (i, result) in response.results.iter().enumerate() {
            if i >= pending_links.len() {
                break;
            }
            let link = pending_links[i];

            if result.status == "accepted" {
                self.create_download_record(
                    &mut conn,
                    link.link_id,
                    &download_type,
                    "downloading",
                    Some(downloader.module_id),
                    result.hash.as_deref(),
                )?;
                total_dispatched += 1;
            } else {
                rejected.push(link);
            }
        }

        pending_links = rejected;
    }
```

With:
```rust
let url_groups = Self::group_links_by_url(&pending_links);
let items: Vec<DownloadRequestItem> = url_groups
    .iter()
    .map(|(url, _)| DownloadRequestItem {
        url: url.clone(),
        save_path: "/downloads".to_string(),
    })
    .collect();

let download_url = format!("{}/downloads", downloader.base_url);
match self.send_batch_to_downloader(&download_url, items).await {
    Ok(response) => {
        let mut rejected = Vec::new();

        for (i, result) in response.results.iter().enumerate() {
            if i >= url_groups.len() {
                break;
            }
            let (_, links_for_url) = &url_groups[i];
            for link in links_for_url {
                if result.status == "accepted" {
                    self.create_download_record(
                        &mut conn,
                        link.link_id,
                        &download_type,
                        "downloading",
                        Some(downloader.module_id),
                        result.hash.as_deref(),
                    )?;
                    total_dispatched += 1;
                } else {
                    rejected.push(*link);
                }
            }
        }

        pending_links = rejected;
    }
```

**Step 7: Verify compilation**

```bash
cd /workspace/core-service
cargo check 2>&1 | grep "^error" | head -20
```

Expected: no errors.

**Step 8: Run all tests**

```bash
cd /workspace/core-service
cargo test 2>&1 | tail -15
```

Expected: all pass.

**Step 9: Commit**

```bash
git add core-service/src/services/download_dispatch.rs
git commit -m "feat(dispatch): deduplicate batch torrent URLs before sending to downloader"
```

---

## Task 3: Batch completion file matching in `download_scheduler`

**Files:**
- Modify: `core-service/src/services/download_scheduler.rs`

**Context:** When a batch torrent completes, `poll_downloader` and `check_recovery` both do a bulk `UPDATE ... WHERE torrent_hash = ?` that writes the same `video_file` (first video found) to all 12 episode records. Replace with per-record matching using `match_batch_files`.

**Step 1: Add imports at the top of `download_scheduler.rs`**

Change:
```rust
use crate::schema::{downloads, service_modules};
use shared::{classify_files, FileType, StatusQueryResponse};
```

To:
```rust
use crate::schema::{anime_links, downloads, service_modules};
use shared::{build_default_chain, classify_files, collect_files_recursive,
             match_batch_files, FileType, StatusQueryResponse};
```

**Step 2: Extract a helper method `apply_completed_files`**

Add this new private method inside `impl DownloadScheduler` (before the closing `}`):

```rust
/// Update download records for a completed torrent.
///
/// - Single record: bulk-set video_file to first video found (existing behaviour).
/// - Multiple records (batch torrent): use match_batch_files to assign
///   each record its specific video_file and subtitle_files.
///   Records that cannot be matched are set to status "batch_unmatched".
fn apply_completed_files(
    conn: &mut PgConnection,
    torrent_hash: &str,
    module_id: i32,
    content_path: Option<&str>,
    files: &[String],
    progress: f64,
    size: u64,
) {
    let now = chrono::Utc::now().naive_utc();

    // Load all Download records + their episode_no for this torrent
    let records: Vec<(i32, i32)> = downloads::table
        .inner_join(anime_links::table.on(anime_links::link_id.eq(downloads::link_id)))
        .filter(downloads::torrent_hash.eq(torrent_hash))
        .filter(downloads::module_id.eq(module_id))
        .filter(downloads::status.eq("downloading"))
        .select((downloads::download_id, anime_links::episode_no))
        .load::<(i32, i32)>(conn)
        .unwrap_or_default();

    if records.is_empty() {
        return;
    }

    if records.len() == 1 {
        // Single episode: original behaviour — pick first video
        let (video_file, subtitle_files_json) = Self::extract_media_files(files);
        diesel::update(
            downloads::table
                .filter(downloads::download_id.eq(records[0].0)),
        )
        .set((
            downloads::status.eq("completed"),
            downloads::progress.eq(progress as f32),
            downloads::total_bytes.eq(size as i64),
            downloads::file_path.eq(content_path),
            downloads::video_file.eq(video_file.as_deref()),
            downloads::subtitle_files.eq(subtitle_files_json.as_deref()),
            downloads::updated_at.eq(now),
        ))
        .execute(conn)
        .ok();
        return;
    }

    // Batch mode: match each episode to its specific file
    let episode_nos: Vec<i32> = records.iter().map(|(_, ep)| *ep).collect();
    let chain = build_default_chain();
    let matches = match_batch_files(files, &episode_nos, &chain);

    for (download_id, episode_no) in &records {
        if let Some((video, subs)) = matches.get(episode_no) {
            let subtitle_json = if subs.is_empty() {
                None
            } else {
                serde_json::to_string(subs).ok()
            };
            diesel::update(downloads::table.filter(downloads::download_id.eq(download_id)))
                .set((
                    downloads::status.eq("completed"),
                    downloads::progress.eq(progress as f32),
                    downloads::total_bytes.eq(size as i64),
                    downloads::file_path.eq(content_path),
                    downloads::video_file.eq(video.as_deref()),
                    downloads::subtitle_files.eq(subtitle_json.as_deref()),
                    downloads::updated_at.eq(now),
                ))
                .execute(conn)
                .ok();
        } else {
            tracing::warn!(
                "batch_unmatched: download_id={} episode_no={} torrent_hash={}",
                download_id,
                episode_no,
                torrent_hash
            );
            diesel::update(downloads::table.filter(downloads::download_id.eq(download_id)))
                .set((
                    downloads::status.eq("batch_unmatched"),
                    downloads::progress.eq(progress as f32),
                    downloads::total_bytes.eq(size as i64),
                    downloads::file_path.eq(content_path),
                    downloads::error_message.eq(Some(format!(
                        "Unable to match episode {} in {}",
                        episode_no,
                        content_path.unwrap_or("(unknown)")
                    ))),
                    downloads::updated_at.eq(now),
                ))
                .execute(conn)
                .ok();
        }
    }
}
```

**Step 3: Replace completion block in `poll_downloader`**

In `poll_downloader` (around line 126), replace the entire `if new_status == "completed" { ... } else { ... }` block with:

```rust
if new_status == "completed" {
    Self::apply_completed_files(
        conn,
        &status_item.hash,
        downloader.module_id,
        status_item.content_path.as_deref(),
        &status_item.files,
        status_item.progress,
        status_item.size,
    );
} else {
    diesel::update(
        downloads::table
            .filter(downloads::torrent_hash.eq(&status_item.hash))
            .filter(downloads::module_id.eq(downloader.module_id)),
    )
    .set((
        downloads::status.eq(new_status),
        downloads::progress.eq(status_item.progress as f32),
        downloads::total_bytes.eq(status_item.size as i64),
        downloads::updated_at.eq(now),
    ))
    .execute(conn)
    .ok();
}
```

**Step 4: Replace completion block in `check_recovery`**

In `check_recovery` (around line 241), replace the `if new_status == "completed" { ... }` block with the same call:

```rust
if new_status == "completed" {
    Self::apply_completed_files(
        conn,
        &status_item.hash,
        downloader.module_id,
        status_item.content_path.as_deref(),
        &status_item.files,
        status_item.progress,
        status_item.size,
    );
} else {
    diesel::update(
        downloads::table
            .filter(downloads::torrent_hash.eq(&status_item.hash))
            .filter(downloads::module_id.eq(downloader.module_id)),
    )
    .set((
        downloads::status.eq(new_status),
        downloads::progress.eq(status_item.progress as f32),
        downloads::total_bytes.eq(status_item.size as i64),
        downloads::updated_at.eq(now),
    ))
    .execute(conn)
    .ok();
}
```

**Step 5: Verify compilation**

```bash
cd /workspace/core-service
cargo check 2>&1 | grep "^error" | head -20
```

Expected: no errors.

**Step 6: Commit**

```bash
git add core-service/src/services/download_scheduler.rs
git commit -m "feat(scheduler): per-episode file matching for batch torrents; batch_unmatched status on failure"
```

---

## Task 4: Add `batch_unmatched` retry pass to scheduler

**Files:**
- Modify: `core-service/src/services/download_scheduler.rs`

**Context:** Records in `batch_unmatched` are terminal unless retried. The scheduler should attempt re-matching on each poll cycle by re-scanning the already-downloaded folder.

**Step 1: Add `retry_batch_unmatched` method**

Add this method to `impl DownloadScheduler` (before the closing `}`):

```rust
/// Retry file matching for downloads in "batch_unmatched" status.
/// The torrent is already downloaded; we re-scan the folder and attempt
/// to match again. On success the record transitions to "completed"
/// and will be picked up by trigger_sync_for_completed.
fn retry_batch_unmatched(&self, conn: &mut PgConnection) {
    use std::collections::HashMap;

    let unmatched: Vec<Download> = match downloads::table
        .filter(downloads::status.eq("batch_unmatched"))
        .filter(downloads::file_path.is_not_null())
        .load::<Download>(conn)
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to query batch_unmatched: {}", e);
            return;
        }
    };

    if unmatched.is_empty() {
        return;
    }

    // Group by (torrent_hash, module_id) → first available folder path
    let mut groups: HashMap<(String, i32), (String, Vec<&Download>)> = HashMap::new();
    for dl in &unmatched {
        if let (Some(hash), Some(mid), Some(fp)) =
            (&dl.torrent_hash, dl.module_id, &dl.file_path)
        {
            groups
                .entry((hash.clone(), mid))
                .or_insert_with(|| (fp.clone(), Vec::new()))
                .1
                .push(dl);
        }
    }

    for ((hash, module_id), (folder, group)) in &groups {
        // Re-scan filesystem for current file list
        let files = collect_files_recursive(std::path::Path::new(folder));
        if files.is_empty() {
            tracing::warn!("retry_batch_unmatched: no files found in {}", folder);
            continue;
        }

        // Get episode_nos via link_id → anime_links
        let link_ids: Vec<i32> = group.iter().map(|d| d.link_id).collect();
        let ep_map: HashMap<i32, i32> = anime_links::table
            .filter(anime_links::link_id.eq_any(&link_ids))
            .select((anime_links::link_id, anime_links::episode_no))
            .load::<(i32, i32)>(conn)
            .unwrap_or_default()
            .into_iter()
            .collect();

        let episode_nos: Vec<i32> = ep_map.values().copied().collect();
        let chain = build_default_chain();
        let matches = match_batch_files(&files, &episode_nos, &chain);

        let now = chrono::Utc::now().naive_utc();
        for dl in group {
            let ep = match ep_map.get(&dl.link_id) {
                Some(e) => *e,
                None => continue,
            };
            if let Some((video, subs)) = matches.get(&ep) {
                let subtitle_json = if subs.is_empty() {
                    None
                } else {
                    serde_json::to_string(subs).ok()
                };
                let updated = diesel::update(
                    downloads::table.filter(downloads::download_id.eq(dl.download_id)),
                )
                .set((
                    downloads::status.eq("completed"),
                    downloads::video_file.eq(video.as_deref()),
                    downloads::subtitle_files.eq(subtitle_json.as_deref()),
                    downloads::error_message.eq(None::<String>),
                    downloads::updated_at.eq(now),
                ))
                .execute(conn);

                match updated {
                    Ok(_) => tracing::info!(
                        "retry_batch_unmatched: recovered download_id={} ep={} hash={}",
                        dl.download_id, ep, hash
                    ),
                    Err(e) => tracing::error!(
                        "retry_batch_unmatched: failed to update download_id={}: {}",
                        dl.download_id, e
                    ),
                }
            }
        }
        let _ = module_id; // suppress unused warning if needed
    }
}
```

**Step 2: Call `retry_batch_unmatched` from `poll_all_downloaders`**

In `poll_all_downloaders`, after the `for downloader in &downloaders { ... }` loop (around line 75), add:

```rust
// Retry any batch_unmatched records
if let Ok(mut conn) = self.db_pool.get() {
    self.retry_batch_unmatched(&mut conn);
}
```

**Step 3: Verify compilation**

```bash
cd /workspace/core-service
cargo check 2>&1 | grep "^error" | head -20
```

Expected: no errors.

**Step 4: Run all backend tests**

```bash
cd /workspace/core-service
cargo test 2>&1 | tail -15
```

Expected: all pass.

**Step 5: Commit**

```bash
git add core-service/src/services/download_scheduler.rs
git commit -m "feat(scheduler): retry batch_unmatched records on each poll cycle"
```

---

## Task 5: Update frontend AI export prompt

**Files:**
- Modify: `frontend/src/components/shared/ParserForm.tsx:456-457`

**Context:** The `handleExportPrompt` function builds an AI prompt that includes a JSON schema for parser fields. `episode_end_source` and `episode_end_value` are missing from this template; the AI will not generate them when prompted.

**Step 1: Locate the prompt template in `ParserForm.tsx`**

Find the block (around lines 456-457):
```typescript
  "year_source": "'regex', 'static', or null - year (optional)",
  "year_value": "string or null"
}
```

**Step 2: Add `episode_end` field descriptions**

Replace:
```typescript
  "year_source": "'regex', 'static', or null - year (optional)",
  "year_value": "string or null"
}
\`\`\`
```

With:
```typescript
  "year_source": "'regex', 'static', or null - year (optional)",
  "year_value": "string or null",
  "episode_end_source": "'regex', 'static', or null - end episode for batch torrents covering a range e.g. 01-12 (optional)",
  "episode_end_value": "string or null - e.g. $3 if parse_regex has a 3rd capture group for the end episode number"
}
\`\`\`
```

**Step 3: Add instruction for batch detection**

Find the instructions block (around line 478-482):
```typescript
## Instructions
Analyze the titles above and generate a parser JSON that can:
1. Match these titles with `condition_regex` — make it as strict as possible
2. Extract anime_title, episode_no, and other fields using `parse_regex` with numbered capture groups
3. Set appropriate source/value pairs for each extracted field using `$N` notation
4. Use null for optional fields that cannot be reliably extracted
5. Determine priority based on the Priority Rules above
```

Change point 4 to:
```typescript
4. Use null for optional fields that cannot be reliably extracted. If titles show an episode range (e.g. \`01-12\`, \`EP01-EP12\`), set \`episode_end_source\` and \`episode_end_value\` to capture the upper bound.
```

**Step 4: Verify TypeScript builds**

```bash
cd /workspace/frontend
node_modules/.bin/tsc --noEmit 2>&1 | grep "error TS" | head -10
```

Expected: no TypeScript errors (this is a string template change only).

**Step 5: Commit**

```bash
git add frontend/src/components/shared/ParserForm.tsx
git commit -m "feat(frontend): add episode_end fields to AI export prompt template"
```

---

## Task 6: Final Verification

**Step 1: Run full backend test suite**

```bash
cd /workspace/core-service
cargo test 2>&1 | tail -20
```

Expected: all tests pass, 0 failures.

**Step 2: Run full workspace build**

```bash
cd /workspace
cargo build --workspace 2>&1 | tail -5
```

Expected: `Finished` with no errors.

**Step 3: Run shared crate tests in isolation**

```bash
cargo test -p shared 2>&1 | tail -10
```

Expected: all pass including the new `episode_tests`.

**Step 4: Final commit if any cleanup needed**

```bash
git status
# Only commit if there are actual cleanup changes
```
