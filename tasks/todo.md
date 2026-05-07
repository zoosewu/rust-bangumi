# Investigate duplicate anime create error

- [x] Review existing task notes and confirm worktree state.
- [x] Locate the database constraint behind `anime_series_anime_id_series_no_key`.
- [x] Trace every code path that creates an anime row.
- [x] Compare duplicate-handling behavior against nearby working patterns.
- [x] Reproduce or reason from tests/logs with enough evidence to identify root cause.
- [x] Implement the narrowest fix if code change is required.
- [x] Run focused verification.
- [x] Document review results.

## Review

- Root cause: application code treated `(work_id, series_no, season_id)` as anime identity, while the database unique key is effectively `(work_id, series_no)`.
- Updated direct create helpers and get-or-create flows to reuse existing anime rows by `(work_id, series_no)` before inserting.
- Added regression coverage for same work/series with a different season_id returning the existing row.
- Verified `cargo test -p core-service test_find_or_create_uses_work_and_series_unique_key` exits 0.
- Verified `cargo test -p core-service --lib` exits 0 with 54 passed tests. Existing unused/deprecated warnings remain.
