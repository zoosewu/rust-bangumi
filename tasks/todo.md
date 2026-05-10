# Tag Badge Tone Adjustment

- [x] Review current `TagBadge` tone classes.
- [x] Reduce tag foreground saturation in both light and dark themes.
- [x] Keep semantic tone distinction through low-opacity background and borders.
- [x] Update lessons for the visual correction.
- [x] Run frontend verification.
- [x] Document review results here.

## Proposed Design

- Keep `TagBadge` as the single tag primitive.
- Stop using high-saturation foreground colors for semantic tones.
- Use neutral/slate text for every tone, especially in dark mode.
- Preserve status recognition with very low-opacity semantic backgrounds and subtle borders.

## Verification Plan

- Run `bun run typecheck`.
- Run `bun run build`.

## Review

- Updated `TagBadge` tones so all semantic tags use neutral slate foreground text instead of high-saturation colored text.
- Kept semantic distinction through low-opacity colored background and border classes.
- Added the dark-mode tag color lesson to `tasks/lessons.md`.
- Verified `bun run typecheck` exits 0.
- Verified `bun run build` exits 0. Vite still reports the existing large chunk warning.
