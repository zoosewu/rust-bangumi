# Manual Download Retry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add user-triggered download retry — a single-record endpoint and a bulk endpoint that re-dispatch retryable downloads through the existing `DownloadDispatchService::dispatch_new_links`, preserving history (each retry inserts a new download row).

**Architecture:** Thin service method `manual_retry(download_ids)` that loads downloads, partitions by retryable status (pure function), dedups link_ids, calls existing `dispatch_new_links`, and reports stats. Two Axum handlers translate this into REST: `POST /downloads/:download_id/retry` and `POST /downloads/retry`. No new download statuses; no DB schema changes.

**Tech Stack:** Rust, Axum, Diesel, Tokio, utoipa (OpenAPI), tracing.

**Spec:** [`docs/superpowers/specs/2026-04-29-manual-download-retry-design.md`](../specs/2026-04-29-manual-download-retry-design.md)

---

## File Map

- **Modify** `core-service/src/services/download_dispatch.rs` — add `RETRYABLE_STATUSES` const, `RetryResult` struct, `partition_retryable` pure fn (with tests), `manual_retry` async method. Refactor inline `redispatchable` to use the const.
- **Modify** `core-service/src/dto.rs` — add `RetryBulkRequest`, `RetryResultResponse`, `RetryOneResponse`.
- **Modify** `core-service/src/handlers/downloads.rs` — add `retry_one`, `retry_bulk` handlers.
- **Modify** `core-service/src/main.rs` — register two new routes.
- **Modify** `core-service/src/openapi.rs` — register new schemas.
- **Modify** `docs/api/openapi.yaml` — add path + component schemas.

---

## Background for the implementer

**The codebase**:
- Binary crate `core-service` (Axum). Library crate `shared` for cross-service types.
- Diesel 2.1 + r2d2 connection pool. Schema lives in `core-service/src/schema.rs`.
- The existing `DownloadDispatchService::dispatch_new_links(link_ids: Vec<i32>) -> Result<DispatchResult, String>` does the heavy lifting:
  - Filters out non-active / filtered / conflict-flagged / batch-conflict-propagated links
  - Inserts a NEW `downloads` row per accepted link (status="downloading")
  - Returns `DispatchResult { dispatched, no_downloader, failed }` (link-level counts)
- Internally, `dispatch_new_links` skips link_ids that have an existing download whose status is **not** in `redispatchable = ["cancelled", "failed", "no_downloader"]`. We must extend this to include `"downloader_error"` so manual retry of `downloader_error` rows actually re-dispatches.

**Test infrastructure**:
- `core-service` is a binary crate; tests in `src/**/tests` compile as binary tests (`cargo test -p core-service --bin core-service`).
- Existing dispatch service has no tests for its full HTTP flow. We only TDD the **pure function** `partition_retryable`.

**Conventions**:
- Use `tracing::info!` for happy path, `tracing::error!` for failures.
- Error responses use `{ "error": "<code>", "message": "<human readable>" }`.
- `cargo check -p core-service` is the minimal compile gate.
- `cargo test -p core-service` runs all tests.

---

## Task 1: Refactor — extract RETRYABLE_STATUSES constant

Carve out the existing inline `redispatchable` into a public const that both `dispatch_new_links` and the new manual-retry code will share, and extend it to include `"downloader_error"`.

**Files:**
- Modify: `core-service/src/services/download_dispatch.rs:118` (replace inline array with const)

- [ ] **Step 1: Add the constant near the top of the file**

After the imports (around line 8), add:

```rust
/// Download statuses that allow re-dispatching the link.
/// `dispatch_new_links` treats other statuses as "active" and skips the link.
/// Manual retry uses the same set as the input gate.
pub const RETRYABLE_STATUSES: &[&str] = &[
    "cancelled",
    "failed",
    "no_downloader",
    "downloader_error",
];
```

- [ ] **Step 2: Replace the inline `redispatchable` array**

In `dispatch_new_links` (around line 118), replace:

```rust
        let redispatchable = &["cancelled", "failed", "no_downloader"];
        let candidate_link_ids: Vec<i32> = links.iter().map(|l| l.link_id).collect();
        let links_with_active_downloads: Vec<i32> = downloads::table
            .filter(downloads::link_id.eq_any(&candidate_link_ids))
            .filter(downloads::status.ne_all(redispatchable))
```

with:

```rust
        let candidate_link_ids: Vec<i32> = links.iter().map(|l| l.link_id).collect();
        let links_with_active_downloads: Vec<i32> = downloads::table
            .filter(downloads::link_id.eq_any(&candidate_link_ids))
            .filter(downloads::status.ne_all(RETRYABLE_STATUSES))
```

- [ ] **Step 3: Compile check**

Run: `cargo check -p core-service`
Expected: compiles cleanly (existing warnings tolerated; no new errors).

- [ ] **Step 4: Run existing tests**

Run: `cargo test -p core-service`
Expected: all existing tests pass (we changed semantics only by extending the set; this is intentional and safe — `downloader_error` rows previously blocked re-dispatch which contradicted the auto-retry path that already includes it).

- [ ] **Step 5: Commit**

```bash
git add core-service/src/services/download_dispatch.rs
git commit -m "refactor(dispatch): extract RETRYABLE_STATUSES const

Pull the inline redispatchable array out of dispatch_new_links into a
shared const, and add downloader_error to the set so it does not block
re-dispatch (consistent with retry_failed_downloads behaviour).
"
```

---

## Task 2: Pure function `partition_retryable` (TDD)

Pure function that splits `&[Download]` into retryable downloads vs. count of non-retryable. Used by `manual_retry` to decide what to feed `dispatch_new_links`.

**Files:**
- Modify: `core-service/src/services/download_dispatch.rs` (add fn + tests at the bottom of `mod tests`)

- [ ] **Step 1: Find or add the test module**

At the bottom of `core-service/src/services/download_dispatch.rs`, look for `#[cfg(test)] mod tests`. If absent, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Download;
    use chrono::Utc;

    fn make_download(download_id: i32, link_id: i32, status: &str) -> Download {
        let now = Utc::now().naive_utc();
        Download {
            download_id,
            link_id,
            downloader_type: "magnet".to_string(),
            status: status.to_string(),
            progress: None,
            downloaded_bytes: None,
            total_bytes: None,
            error_message: None,
            created_at: now,
            updated_at: now,
            module_id: None,
            torrent_hash: None,
            file_path: None,
            sync_retry_count: 0,
            video_file: None,
            subtitle_files: None,
        }
    }
}
```

If the module already exists, just add `make_download` near the top of it (avoid duplicate imports).

- [ ] **Step 2: Write the failing tests**

Inside `mod tests`, add:

```rust
    #[test]
    fn partition_retryable_keeps_retryable_statuses() {
        let downloads = vec![
            make_download(1, 10, "failed"),
            make_download(2, 20, "cancelled"),
            make_download(3, 30, "no_downloader"),
            make_download(4, 40, "downloader_error"),
        ];
        let (retryable, not_retryable) = partition_retryable(&downloads);
        assert_eq!(retryable.len(), 4);
        assert_eq!(not_retryable, 0);
    }

    #[test]
    fn partition_retryable_excludes_active_statuses() {
        let downloads = vec![
            make_download(1, 10, "downloading"),
            make_download(2, 20, "completed"),
            make_download(3, 30, "syncing"),
            make_download(4, 40, "failed"),
        ];
        let (retryable, not_retryable) = partition_retryable(&downloads);
        assert_eq!(retryable.len(), 1);
        assert_eq!(retryable[0].download_id, 4);
        assert_eq!(not_retryable, 3);
    }

    #[test]
    fn partition_retryable_handles_empty_input() {
        let (retryable, not_retryable) = partition_retryable(&[]);
        assert!(retryable.is_empty());
        assert_eq!(not_retryable, 0);
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p core-service --bin core-service services::download_dispatch::tests::partition_retryable -- --nocapture 2>&1 | tail -20`
Expected: compile error `cannot find function partition_retryable`.

- [ ] **Step 4: Implement the function**

Just above `impl DownloadDispatchService` (or near the top of the file after the const), add:

```rust
/// Split a slice of downloads into (retryable references, count of non-retryable).
/// Pure function — no DB / IO.
pub fn partition_retryable(downloads: &[Download]) -> (Vec<&Download>, usize) {
    let mut retryable = Vec::with_capacity(downloads.len());
    let mut not_retryable = 0;
    for d in downloads {
        if RETRYABLE_STATUSES.contains(&d.status.as_str()) {
            retryable.push(d);
        } else {
            not_retryable += 1;
        }
    }
    (retryable, not_retryable)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p core-service --bin core-service services::download_dispatch::tests::partition_retryable 2>&1 | tail -15`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 6: Commit**

```bash
git add core-service/src/services/download_dispatch.rs
git commit -m "feat(dispatch): partition_retryable pure function

Split downloads by retryable status. Pure function, fully unit-tested.
Used by upcoming manual_retry service method.
"
```

---

## Task 3: Service method `manual_retry`

Loads downloads by id, partitions them, dedups link_ids, calls `dispatch_new_links`, reports counts.

**Files:**
- Modify: `core-service/src/services/download_dispatch.rs` (add `RetryResult` struct + `manual_retry` method on `impl DownloadDispatchService`)

- [ ] **Step 1: Add the result struct**

Below the existing `DispatchResult` struct, add:

```rust
#[derive(Debug, Clone)]
pub struct RetryResult {
    pub downloads_matched: usize,
    pub not_retryable: usize,
    pub unique_links: usize,
    pub dispatched: usize,
    pub no_downloader: usize,
    pub conflict_or_filtered: usize,
    pub failed: usize,
}
```

- [ ] **Step 2: Add the method**

Inside `impl DownloadDispatchService`, near the existing `retry_failed_downloads` method, add:

```rust
    /// Manually retry the given downloads.
    ///
    /// Loads each download, keeps only those in `RETRYABLE_STATUSES`, dedups
    /// link_ids, and calls `dispatch_new_links`. The existing dispatch logic
    /// inserts a NEW download row per accepted link, preserving history.
    ///
    /// Counts in `RetryResult`:
    /// - `downloads_matched`: downloads found by id (input length minus missing)
    /// - `not_retryable`: matched downloads whose status is not retryable
    /// - `unique_links`: deduplicated link_ids fed into dispatch
    /// - `dispatched / no_downloader / failed`: forwarded from `DispatchResult`
    /// - `conflict_or_filtered`: `unique_links - dispatched - no_downloader - failed`
    ///   (links dropped by dispatch's conflict / filter / link_status / batch-conflict gates)
    pub async fn manual_retry(
        &self,
        download_ids: Vec<i32>,
    ) -> Result<RetryResult, String> {
        if download_ids.is_empty() {
            return Ok(RetryResult {
                downloads_matched: 0,
                not_retryable: 0,
                unique_links: 0,
                dispatched: 0,
                no_downloader: 0,
                conflict_or_filtered: 0,
                failed: 0,
            });
        }

        let mut conn = self.db_pool.get().map_err(|e| e.to_string())?;

        let matched: Vec<Download> = downloads::table
            .filter(downloads::download_id.eq_any(&download_ids))
            .load::<Download>(&mut conn)
            .map_err(|e| format!("Failed to load downloads: {}", e))?;

        let downloads_matched = matched.len();
        let (retryable, not_retryable) = partition_retryable(&matched);

        let mut seen: HashSet<i32> = HashSet::new();
        let unique_link_ids: Vec<i32> = retryable
            .iter()
            .filter_map(|d| if seen.insert(d.link_id) { Some(d.link_id) } else { None })
            .collect();
        let unique_links = unique_link_ids.len();

        if unique_link_ids.is_empty() {
            return Ok(RetryResult {
                downloads_matched,
                not_retryable,
                unique_links: 0,
                dispatched: 0,
                no_downloader: 0,
                conflict_or_filtered: 0,
                failed: 0,
            });
        }

        // Drop the connection before awaiting an async call that may need its own conn.
        drop(conn);

        let dispatch_result = self.dispatch_new_links(unique_link_ids).await?;

        let DispatchResult {
            dispatched,
            no_downloader,
            failed,
        } = dispatch_result;

        let accounted = dispatched + no_downloader + failed;
        let conflict_or_filtered = unique_links.saturating_sub(accounted);

        tracing::info!(
            "manual_retry: matched={}, not_retryable={}, unique_links={}, dispatched={}, no_downloader={}, conflict_or_filtered={}, failed={}",
            downloads_matched,
            not_retryable,
            unique_links,
            dispatched,
            no_downloader,
            conflict_or_filtered,
            failed
        );

        Ok(RetryResult {
            downloads_matched,
            not_retryable,
            unique_links,
            dispatched,
            no_downloader,
            conflict_or_filtered,
            failed,
        })
    }
```

- [ ] **Step 3: Compile check**

Run: `cargo check -p core-service`
Expected: compiles cleanly. If the `Download` model is missing fields (e.g., `sync_retry_count`), adjust the `make_download` helper from Task 2 (the helper must match the real model).

- [ ] **Step 4: Run all tests**

Run: `cargo test -p core-service`
Expected: all tests pass. The new method has no direct unit test (it talks to the DB pool); it is exercised via manual verification in Task 8.

- [ ] **Step 5: Commit**

```bash
git add core-service/src/services/download_dispatch.rs
git commit -m "feat(dispatch): manual_retry service method

Load downloads, partition by retryable status, dedup link_ids, dispatch
through dispatch_new_links. Returns RetryResult with both
download-level (matched/not_retryable) and link-level
(unique_links/dispatched/no_downloader/conflict_or_filtered/failed) counts.
"
```

---

## Task 4: DTOs

Request and response shapes for the two new endpoints. Reuse `RetryResult` field names so the response is a direct serialization.

**Files:**
- Modify: `core-service/src/dto.rs` (add three structs near the existing download DTOs)

- [ ] **Step 1: Add the DTOs**

At the bottom of `core-service/src/dto.rs` (just before the `#[cfg(test)] mod tests` block, if present, otherwise at end), add:

```rust
// ============ Manual Download Retry DTOs ============

#[derive(Debug, Deserialize, ToSchema)]
pub struct RetryBulkRequest {
    /// 指定要重試的 download_ids；不傳則涵蓋所有 retryable downloads
    #[serde(default)]
    pub download_ids: Option<Vec<i32>>,
    /// 額外限定 status 子集（仍會卡在 RETRYABLE_STATUSES 之內）
    #[serde(default)]
    pub status: Option<Vec<String>>,
    /// 額外限定 downloader_type
    #[serde(default)]
    pub downloader_type: Option<String>,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct RetryResultResponse {
    pub downloads_matched: usize,
    pub not_retryable: usize,
    pub unique_links: usize,
    pub dispatched: usize,
    pub no_downloader: usize,
    pub conflict_or_filtered: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct RetryOneResponse {
    pub download_id: i32,
    pub link_id: i32,
    /// "dispatched" | "no_downloader"
    pub status: String,
}
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p core-service`
Expected: compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add core-service/src/dto.rs
git commit -m "feat(dto): retry request/response DTOs"
```

---

## Task 5: Handlers `retry_one` and `retry_bulk`

Two Axum handlers that translate `manual_retry` results into HTTP responses.

**Files:**
- Modify: `core-service/src/handlers/downloads.rs` (add handlers + helper)

- [ ] **Step 1: Add imports at the top of the file**

Find the existing imports in `core-service/src/handlers/downloads.rs`. Add (or extend) these:

```rust
use axum::extract::Path;
use serde_json::json;
use crate::dto::{RetryBulkRequest, RetryOneResponse, RetryResultResponse};
use crate::services::download_dispatch::{RetryResult, RETRYABLE_STATUSES};
```

(`State` and `Json` are already imported. `Query` too. `Download`, `downloads`, `anime_links` are already imported.)

- [ ] **Step 2: Add a result-to-response converter**

At the bottom of the file, add:

```rust
fn into_response(r: RetryResult) -> RetryResultResponse {
    RetryResultResponse {
        downloads_matched: r.downloads_matched,
        not_retryable: r.not_retryable,
        unique_links: r.unique_links,
        dispatched: r.dispatched,
        no_downloader: r.no_downloader,
        conflict_or_filtered: r.conflict_or_filtered,
        failed: r.failed,
    }
}
```

- [ ] **Step 3: Add `retry_one` handler**

Below `into_response`, add:

```rust
/// POST /downloads/:download_id/retry — manually retry a single download.
pub async fn retry_one(
    State(state): State<AppState>,
    Path(download_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = match state
        .dispatch_service
        .manual_retry(vec![download_id])
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("retry_one({}) failed: {}", download_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "dispatch_failed", "message": e })),
            );
        }
    };

    if result.downloads_matched == 0 {
        // Distinguish 404 (not found) from 409 (existed but wrong status).
        let mut conn = match state.db.get() {
            Ok(c) => c,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "db_error", "message": e.to_string() })),
                );
            }
        };
        let exists = downloads::table
            .filter(downloads::download_id.eq(download_id))
            .select(downloads::download_id)
            .first::<i32>(&mut conn)
            .optional()
            .unwrap_or(None)
            .is_some();
        if !exists {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "download_not_found", "download_id": download_id })),
            );
        }
        // Found but not retryable
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "not_retryable",
                "download_id": download_id,
                "message": "Download status is not in retryable set",
                "retryable_statuses": RETRYABLE_STATUSES,
            })),
        );
    }

    if result.not_retryable == 1 {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "not_retryable",
                "download_id": download_id,
                "retryable_statuses": RETRYABLE_STATUSES,
            })),
        );
    }

    // Resolve link_id for the response (one matched download means one link).
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "db_error", "message": e.to_string() })),
            );
        }
    };
    let link_id: i32 = match downloads::table
        .filter(downloads::download_id.eq(download_id))
        .select(downloads::link_id)
        .first::<i32>(&mut conn)
    {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "db_error", "message": e.to_string() })),
            );
        }
    };

    if result.dispatched == 1 {
        let resp = RetryOneResponse {
            download_id,
            link_id,
            status: "dispatched".to_string(),
        };
        return (StatusCode::OK, Json(serde_json::to_value(resp).unwrap_or(json!({}))));
    }

    if result.no_downloader == 1 {
        let resp = RetryOneResponse {
            download_id,
            link_id,
            status: "no_downloader".to_string(),
        };
        return (StatusCode::OK, Json(serde_json::to_value(resp).unwrap_or(json!({}))));
    }

    if result.conflict_or_filtered == 1 {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "link_not_dispatchable",
                "download_id": download_id,
                "link_id": link_id,
                "message": "Link is filtered, in conflict, or otherwise blocked from dispatch",
            })),
        );
    }

    // Catch-all — failed > 0 or unexpected combination
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "dispatch_failed", "result": into_response(result) })),
    )
}
```

- [ ] **Step 4: Add `retry_bulk` handler**

Below `retry_one`, add:

```rust
/// POST /downloads/retry — bulk retry, optional filters in the body.
pub async fn retry_bulk(
    State(state): State<AppState>,
    Json(payload): Json<RetryBulkRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "db_error", "message": e.to_string() })),
            );
        }
    };

    let mut q = downloads::table
        .filter(downloads::status.eq_any(RETRYABLE_STATUSES))
        .into_boxed();
    if let Some(ids) = &payload.download_ids {
        q = q.filter(downloads::download_id.eq_any(ids));
    }
    if let Some(s) = &payload.status {
        q = q.filter(downloads::status.eq_any(s));
    }
    if let Some(dt) = &payload.downloader_type {
        q = q.filter(downloads::downloader_type.eq(dt));
    }

    let candidate_ids: Vec<i32> = match q.select(downloads::download_id).load::<i32>(&mut conn) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "db_error", "message": e.to_string() })),
            );
        }
    };

    drop(conn);

    if candidate_ids.is_empty() {
        let empty = RetryResultResponse {
            downloads_matched: 0,
            not_retryable: 0,
            unique_links: 0,
            dispatched: 0,
            no_downloader: 0,
            conflict_or_filtered: 0,
            failed: 0,
        };
        return (StatusCode::OK, Json(serde_json::to_value(empty).unwrap_or(json!({}))));
    }

    let count = candidate_ids.len();
    match state.dispatch_service.manual_retry(candidate_ids).await {
        Ok(r) => {
            tracing::info!("retry_bulk: candidates={}, result={:?}", count, r);
            (
                StatusCode::OK,
                Json(serde_json::to_value(into_response(r)).unwrap_or(json!({}))),
            )
        }
        Err(e) => {
            tracing::error!("retry_bulk failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "dispatch_failed", "message": e })),
            )
        }
    }
}
```

- [ ] **Step 5: Compile check**

Run: `cargo check -p core-service`
Expected: compiles cleanly.

- [ ] **Step 6: Commit**

```bash
git add core-service/src/handlers/downloads.rs
git commit -m "feat(handlers): retry_one and retry_bulk download endpoints"
```

---

## Task 6: Register routes

**Files:**
- Modify: `core-service/src/main.rs` (add two `.route(...)` lines next to existing downloads route)

- [ ] **Step 1: Find the downloads route**

Search for `handlers::downloads::list_downloads` in `core-service/src/main.rs`. It should look like:

```rust
        .route("/downloads", get(handlers::downloads::list_downloads))
```

- [ ] **Step 2: Add the two retry routes immediately after**

Replace that single line with:

```rust
        .route("/downloads", get(handlers::downloads::list_downloads))
        .route("/downloads/retry", post(handlers::downloads::retry_bulk))
        .route(
            "/downloads/:download_id/retry",
            post(handlers::downloads::retry_one),
        )
```

> Note: The bulk route is registered *before* the single-id route. Axum's matching is path-segment based and these patterns don't conflict, but ordering by specificity is a defensive habit.

- [ ] **Step 3: Compile check**

Run: `cargo check -p core-service`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add core-service/src/main.rs
git commit -m "feat(routes): register manual download retry endpoints"
```

---

## Task 7: OpenAPI registration

Register the new schemas in utoipa, and write the path/schema entries in the hand-written `openapi.yaml`.

**Files:**
- Modify: `core-service/src/openapi.rs` (add three schemas to `components(schemas(...))`)
- Modify: `docs/api/openapi.yaml` (add path entries + component schemas)

- [ ] **Step 1: Register utoipa schemas**

In `core-service/src/openapi.rs`, find the `use crate::dto::{...}` block and add `RetryBulkRequest, RetryOneResponse, RetryResultResponse,` to the import list.

Then in the `components(schemas(...))` block inside `#[openapi(...)]`, add:

```rust
        RetryBulkRequest,
        RetryOneResponse,
        RetryResultResponse,
```

- [ ] **Step 2: Add OpenAPI yaml paths**

In `docs/api/openapi.yaml`, find the `/downloads` path block (search for `/downloads:` followed by `get:` near the existing list endpoint). Immediately after that block, add:

```yaml
  /downloads/{download_id}/retry:
    post:
      tags: [Downloads]
      summary: 手動重試單筆下載
      description: |
        對指定 download_id 重試。會沿用 dispatch_new_links 流程，每次重試 INSERT
        新的 download row，保留歷史。可重試 status：failed、downloader_error、
        no_downloader、cancelled。
      parameters:
        - name: download_id
          in: path
          required: true
          schema:
            type: integer
      responses:
        '200':
          description: 派送成功（status 為 dispatched 或 no_downloader）
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/RetryOneResponse'
        '404':
          description: download 不存在
        '409':
          description: download 狀態不可重試 / link 被 filter / conflict 擋住
        '500':
          description: dispatch 失敗

  /downloads/retry:
    post:
      tags: [Downloads]
      summary: 批次重試下載
      description: |
        批次重試符合 retryable status 的下載。可選的 download_ids / status /
        downloader_type 篩選欄位以 AND 結合，並一律加上 RETRYABLE_STATUSES gate。
        Response 同時報 download 級（downloads_matched / not_retryable）與
        link 級（unique_links / dispatched / no_downloader / conflict_or_filtered / failed）統計。
      requestBody:
        required: false
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/RetryBulkRequest'
      responses:
        '200':
          description: 處理成功（即使 0 筆命中也會回 200）
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/RetryResultResponse'
        '500':
          description: dispatch 失敗
```

- [ ] **Step 3: Add component schemas to openapi.yaml**

Find the existing `components: schemas:` section and add (alphabetical insertion is nice but not required):

```yaml
    RetryBulkRequest:
      type: object
      properties:
        download_ids:
          type: array
          items:
            type: integer
          nullable: true
        status:
          type: array
          items:
            type: string
          nullable: true
          description: 限定 status 子集（仍受 RETRYABLE_STATUSES 約束）
        downloader_type:
          type: string
          nullable: true

    RetryOneResponse:
      type: object
      required: [download_id, link_id, status]
      properties:
        download_id:
          type: integer
        link_id:
          type: integer
        status:
          type: string
          enum: [dispatched, no_downloader]

    RetryResultResponse:
      type: object
      required:
        - downloads_matched
        - not_retryable
        - unique_links
        - dispatched
        - no_downloader
        - conflict_or_filtered
        - failed
      properties:
        downloads_matched:
          type: integer
          description: 篩選命中的 download 筆數
        not_retryable:
          type: integer
          description: 命中後被擋掉的（status 不在 retryable）
        unique_links:
          type: integer
          description: dedup 後送進 dispatch 的 link 數
        dispatched:
          type: integer
        no_downloader:
          type: integer
        conflict_or_filtered:
          type: integer
          description: 被 conflict / filter / resolved / 批次傳播擋掉
        failed:
          type: integer
```

- [ ] **Step 4: Compile check**

Run: `cargo check -p core-service`
Expected: compiles cleanly.

- [ ] **Step 5: Commit**

```bash
git add core-service/src/openapi.rs docs/api/openapi.yaml
git commit -m "docs(openapi): document manual download retry endpoints"
```

---

## Task 8: Manual verification

Sanity-check the new endpoints against dev DB. Note: this requires the core-service running and PostgreSQL up.

- [ ] **Step 1: Bring up dev infra and run core-service**

```bash
docker compose -f docker-compose.dev.yaml up -d
cargo run -p core-service
```

Wait for the log line indicating the service bound to port 8000.

- [ ] **Step 2: Pick (or create) a failed download**

```bash
docker exec bangumi-postgres-dev psql -U bangumi -d bangumi -c \
  "SELECT download_id, link_id, status FROM downloads ORDER BY download_id DESC LIMIT 5;"
```

Pick one that is currently `downloading` or `completed` and force it into `failed` for testing:

```bash
docker exec bangumi-postgres-dev psql -U bangumi -d bangumi -c \
  "UPDATE downloads SET status='failed', error_message='manual test' WHERE download_id=<pick_one>;"
```

- [ ] **Step 3: Single retry — happy path**

```bash
xh POST localhost:8000/downloads/<that_download_id>/retry
```

Expected: HTTP 200, body `{ "download_id": ..., "link_id": ..., "status": "dispatched" }`. Check `downloads` table — there should be a new row with status `downloading` for the same link_id.

- [ ] **Step 4: Single retry — not retryable**

Pick a download in status `completed` (or `synced`):

```bash
xh POST localhost:8000/downloads/<completed_id>/retry
```

Expected: HTTP 409, body contains `"error": "not_retryable"`.

- [ ] **Step 5: Single retry — not found**

```bash
xh POST localhost:8000/downloads/999999/retry
```

Expected: HTTP 404, body contains `"error": "download_not_found"`.

- [ ] **Step 6: Bulk retry — no filter**

Force two downloads to `failed`, then:

```bash
xh POST localhost:8000/downloads/retry
```

Expected: HTTP 200, body has `downloads_matched >= 2`, `dispatched >= 2`.

- [ ] **Step 7: Bulk retry — with filters**

```bash
xh POST localhost:8000/downloads/retry status:='["cancelled"]' downloader_type=magnet
```

Expected: HTTP 200, only `cancelled` magnet downloads counted.

- [ ] **Step 8: No commit needed; this task is verification only.**

If any step fails, fix the underlying code in the relevant prior task and re-run.

---

## Self-Review Checklist (executor: skip — already done by author)

- [x] Spec coverage: §2.1, §2.2, §3.1, §3.2, §3.3, §3.4, §4 (edge cases handled in handler logic), §5 (Task 2 unit tests + Task 8 manual verification) all mapped.
- [x] No placeholders: every step has concrete code or commands.
- [x] Type consistency: `RetryResult` (service) ↔ `RetryResultResponse` (DTO) field names match exactly; `RETRYABLE_STATUSES` referenced consistently.
- [x] DB model field list in `make_download` matches `Download` struct in `models/db.rs` (verified against schema.rs at planning time).
