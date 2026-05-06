# Fix frontend build readonly schema arrays

- [x] Inspect reported TypeScript errors and related schema definitions.
- [x] Confirm current local diffs before editing touched files.
- [x] Reproduce the frontend build/typecheck failure locally.
- [x] Fix schema/generated-type compatibility at the narrowest shared point.
- [x] Re-run frontend build verification.
- [x] Document review results.

## Review

- Reproduced failure with `bun run build` in `frontend`: TypeScript rejected readonly schema arrays against mutable generated OpenAPI arrays.
- Added shared schema assertion helper that readonly-normalizes generated API types before applying the compile-time compatibility constraint.
- Verified `bun run build` exits 0. Vite still reports the pre-existing large chunk warning for the main JS bundle.
