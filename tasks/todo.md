# Tag Badge Consolidation

- [x] Review current tag/badge usage points and existing translations.
- [x] Add regression coverage for the filtered-to-eliminated UI status semantics.
- [x] Extract a shared `TagBadge` component with consistent sizing and tone styles.
- [x] Rework status/download/filter/parser tags to use `TagBadge`.
- [x] Add translations for tag text in English, Traditional Chinese, and Japanese.
- [x] Rename user-facing filtered labels to eliminated/已淘汰 to avoid ambiguity.
- [x] Run frontend formatting, tests, typecheck, and build.
- [x] Document review results here.

## Proposed Design

- Keep the backend/API model unchanged: `filter_passed=false` still means the item was excluded by filtering.
- Rename the frontend display status kind to `eliminated`, so Latest Updates status priority reads as a user-facing elimination reason instead of a technical filter result.
- Add `frontend/src/components/shared/TagBadge.tsx` as the single visual primitive for small labels, with semantic tones such as neutral, info, success, warning, danger, and muted.
- Keep specialized wrappers like `StatusBadge` and `DownloadBadge`, but make them translate their labels and render through `TagBadge`.
- Apply `TagBadge` to page-level tags where the current UI uses inconsistent direct `Badge` styling.

## Verification Plan

- Frontend focused: run the raw item status test red/green.
- Frontend broad: run `bun run test`, `bun run typecheck`, and `bun run build`.
- Formatting: run the repository frontend formatter if available; otherwise run existing format-capable commands only and note if none exists.

## Review

- Added `TagBadge` as the shared small-label component and routed status, download, filter, parser, AI, source, count, season, subscription, provider, and service-health tags through it.
- Changed Latest Updates display status from `filtered` to `eliminated`, while leaving backend/API fields such as `filter_passed` and `filtered_items` unchanged.
- Added translated tag labels under `tags` in `en`, `zh-TW`, and `ja`, and changed user-facing filtered labels to Eliminated / 已淘汰 / 除外済み.
- Confirmed there are no remaining direct page/component `Badge` usages outside the `TagBadge` wrapper, and no remaining user-visible `已篩選` / `フィルター済み` / `Filtered` strings.
- Verified the raw item status test fails before the production change and passes after it.
- Verified `bun run typecheck`, `bun run test`, and `bun run build` exit 0. Build still reports the existing Vite large chunk warning.
- `bun run lint` still exits 1 due to existing unrelated lint debt across the app, including React hook lint rules and test unused-argument rules. I removed the new unused imports/props found in touched files.
- No frontend formatter script exists in `frontend/package.json`; no formatter command was available to run for these TS/JSON files.
