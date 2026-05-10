# UI and Retry Eligibility Adjustments

- [x] Review current project notes and confirm worktree state.
- [x] Locate title columns, Latest Updates table layout, and retry-download status logic.
- [x] Add a reusable title-cell presentation so titled table cells are wider and show full text in a tooltip.
- [x] Update titled pages/tables to use the wider tooltip title cell where titles may truncate.
- [x] Move Latest Updates retry action into its own leftmost column, followed by download status, then title.
- [x] Remove `cancelled` from manual retry eligibility in both frontend display logic and backend retry gates.
- [x] Add regression coverage proving cancelled downloads are not manually retryable.
- [x] Run focused frontend and backend verification.
- [x] Document review results here.

## Proposed Design

- Use the existing Radix tooltip wrapper in `frontend/src/components/ui/tooltip.tsx`; no new tooltip library.
- Add a compact shared cell helper for title text, likely under `frontend/src/components/shared/`, with a wider max width and full-title tooltip.
- Keep Latest Updates table behavior row-clickable; retry button will stop propagation as it does today.
- Backend retry semantics will treat manual retry as allowed only for unexpected dispatch/download failures: `failed`, `downloader_error`, and `no_downloader`. `cancelled` will remain visible as a status but will not create retry controls and will return 409 if called directly.

## Verification Plan

- Frontend: run the relevant test/typecheck command available in `frontend/package.json`.
- Backend: run the focused `core-service` test for retry partitioning, then a broader `core-service` lib test if feasible.

## Review

- Added shared `TitleCell` and wired `TooltipProvider`; page headers and title/name table columns now truncate at wider widths and expose the full title in a tooltip.
- Updated Latest Updates column order to `retry`, `download`, `title`, then existing status/details columns. Retry no longer shares the download-status column.
- Removed `cancelled` from frontend retry button visibility and backend `RETRYABLE_STATUSES`, so filter/parser cancellations cannot be manually retried through the UI or API retry gate.
- Split backend manual retry statuses from automatic dispatch terminal statuses, preserving `cancelled` as terminal for future automatic dispatch without making it manually retryable.
- Changed bulk retry with explicit `download_ids` to let `manual_retry` count cancelled records as `not_retryable` instead of silently dropping them during pre-filtering.
- Added regression coverage for cancelled downloads being excluded from retryable downloads.
- Verified `cargo test -p core-service partition_retryable_excludes_cancelled_downloads` exits 0 after first confirming it failed before the production-code change.
- Verified `cargo test -p core-service dispatch_terminal_statuses_include_cancelled_downloads` exits 0.
- Verified `cargo test -p core-service --bin core-service` exits 0 with 151 passed tests. Existing unused/deprecated warnings remain.
- Verified `bun run typecheck`, `bun run test`, and `bun run build` exit 0. Vite still reports the existing large chunk warning.
