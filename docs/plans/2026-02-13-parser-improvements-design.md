# Parser Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve the parser system with better UI layout, edit support, smarter prompts, and `$N` regex index format.

**Architecture:** Six changes across frontend (ParserForm, ParserEditor, ParsersPage, CoreApi, i18n) and backend (title_parser service). All changes are independent enough to be committed separately.

**Tech Stack:** React + Effect (frontend), Rust + Axum + Diesel (backend), i18n (zh-TW, en, ja)

---

### Task 1: Backend — Support `$N` format in regex capture group index

**Files:**
- Modify: `core-service/src/services/title_parser.rs:156-173` (`extract_value` method)

**Step 1: Modify `extract_value` to strip `$` prefix**

In `core-service/src/services/title_parser.rs`, replace the `extract_value` method:

```rust
    /// 從捕獲組或靜態值提取欄位值
    fn extract_value(
        source: &ParserSourceType,
        value: &str,
        captures: &regex::Captures,
    ) -> Result<String, String> {
        match source {
            ParserSourceType::Regex => {
                // Support both "$1" and "1" formats
                let index_str = value.strip_prefix('$').unwrap_or(value);
                let index: usize = index_str
                    .parse()
                    .map_err(|_| format!("Invalid capture group index: {}", value))?;
                captures
                    .get(index)
                    .map(|m| m.as_str().trim().to_string())
                    .ok_or_else(|| format!("Capture group {} not found", index))
            }
            ParserSourceType::Static => Ok(value.to_string()),
        }
    }
```

**Step 2: Commit**

```bash
git add core-service/src/services/title_parser.rs
git commit -m "feat: support \$N format in parser regex capture group index"
```

---

### Task 2: Frontend — Add `updateParser` to CoreApi and ApiLayer

**Files:**
- Modify: `frontend/src/services/CoreApi.ts:36-37`
- Modify: `frontend/src/layers/ApiLayer.ts:88-94`

**Step 1: Add `updateParser` to CoreApi interface**

In `frontend/src/services/CoreApi.ts`, after `createParser` line (line 36), add:

```typescript
    readonly updateParser: (id: number, req: Record<string, unknown>) => Effect.Effect<TitleParser>
```

**Step 2: Add `updateParser` implementation to ApiLayer**

In `frontend/src/layers/ApiLayer.ts`, after `createParser` (line 89), add:

```typescript
    updateParser: (id, req) =>
      client
        .execute(
          HttpClientRequest.put(`/api/core/parsers/${id}`).pipe(
            HttpClientRequest.bodyUnsafeJson(req),
          ),
        )
        .pipe(
          Effect.flatMap((response) => response.json),
          Effect.flatMap(Schema.decodeUnknown(TitleParser)),
          Effect.scoped,
          Effect.orDie,
        ),
```

**Step 3: Commit**

```bash
git add frontend/src/services/CoreApi.ts frontend/src/layers/ApiLayer.ts
git commit -m "feat: add updateParser API method"
```

---

### Task 3: Frontend — Add i18n keys for edit parser

**Files:**
- Modify: `frontend/src/i18n/zh-TW.json`
- Modify: `frontend/src/i18n/en.json`
- Modify: `frontend/src/i18n/ja.json`

**Step 1: Add new i18n keys**

In each i18n file, add to the `"parser"` section:

zh-TW:
```json
    "editParser": "編輯解析器",
    "save": "儲存",
    "saving": "儲存中..."
```

en:
```json
    "editParser": "Edit Parser",
    "save": "Save",
    "saving": "Saving..."
```

ja:
```json
    "editParser": "パーサーを編集",
    "save": "保存",
    "saving": "保存中..."
```

**Step 2: Commit**

```bash
git add frontend/src/i18n/zh-TW.json frontend/src/i18n/en.json frontend/src/i18n/ja.json
git commit -m "feat: add i18n keys for parser editing"
```

---

### Task 4: Frontend — Restructure ParserFormFields layout and integrate AI buttons

**Files:**
- Modify: `frontend/src/components/shared/ParserForm.tsx:100-282`

**Step 1: Update `ParserFormFields` props to include AI button params**

Change the `ParserFormFields` component signature to accept optional AI button props:

```typescript
export function ParserFormFields({
  form,
  onChange,
  onImport,
  targetType,
  targetId,
}: {
  form: ParserFormState
  onChange: (key: string, value: string | number | null) => void
  onImport?: (form: ParserFormState) => void
  targetType?: string
  targetId?: number | null
}) {
```

**Step 2: Restructure the JSX**

Replace the entire return JSX of `ParserFormFields` with:

```tsx
  const { t } = useTranslation()

  return (
    <div className="space-y-3">
      {/* AI Import/Export buttons at top */}
      {onImport && targetType && (
        <div className="flex gap-2">
          <ParserAIButtons
            onImport={onImport}
            targetType={targetType}
            targetId={targetId ?? null}
          />
        </div>
      )}

      {/* Name + Priority */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <Label className="text-xs">{t("common.name", "Name")}</Label>
          <Input
            value={form.name}
            onChange={(e) => onChange("name", e.target.value)}
            placeholder="Parser name"
          />
        </div>
        <div>
          <Label className="text-xs">{t("parsers.priority", "Priority")}</Label>
          <Input
            type="number"
            value={form.priority}
            onChange={(e) => onChange("priority", parseInt(e.target.value) || 0)}
          />
        </div>
      </div>

      {/* Condition Regex */}
      <div>
        <Label className="text-xs">{t("parsers.conditionRegex", "Condition Regex")}</Label>
        <Input
          className="font-mono text-sm"
          value={form.condition_regex}
          onChange={(e) => onChange("condition_regex", e.target.value)}
          placeholder={t("parsers.conditionRegexPlaceholder", "Must match to activate this parser")}
        />
      </div>

      {/* Parse Regex */}
      <div>
        <Label className="text-xs">{t("parsers.parseRegex", "Parse Regex")}</Label>
        <Input
          className="font-mono text-sm"
          value={form.parse_regex}
          onChange={(e) => onChange("parse_regex", e.target.value)}
          placeholder={t("parsers.parseRegexPlaceholder", "Capture groups for field extraction")}
        />
      </div>

      {/* Field extraction — title on own line, 3 per row */}
      <div className="space-y-2">
        <Label className="text-sm font-semibold">{t("parsers.fieldExtraction", "Field Extraction")}</Label>

        <div className="grid grid-cols-3 gap-3">
          <FieldSourceInput
            label={t("parsers.animeTitle", "Anime Title")}
            source={form.anime_title_source}
            value={form.anime_title_value}
            onSourceChange={(v) => onChange("anime_title_source", v)}
            onValueChange={(v) => onChange("anime_title_value", v)}
            required
          />
          <FieldSourceInput
            label={t("parsers.episodeNo", "Episode No")}
            source={form.episode_no_source}
            value={form.episode_no_value}
            onSourceChange={(v) => onChange("episode_no_source", v)}
            onValueChange={(v) => onChange("episode_no_value", v)}
            required
          />
          <FieldSourceInput
            label={t("parsers.seriesNo", "Series No")}
            source={form.series_no_source}
            value={form.series_no_value ?? ""}
            onSourceChange={(v) => {
              onChange("series_no_source", v || null)
              if (!v) onChange("series_no_value", null)
            }}
            onValueChange={(v) => onChange("series_no_value", v || null)}
          />
        </div>

        <div className="grid grid-cols-3 gap-3">
          <FieldSourceInput
            label={t("parsers.subtitleGroup", "Subtitle Group")}
            source={form.subtitle_group_source}
            value={form.subtitle_group_value ?? ""}
            onSourceChange={(v) => {
              onChange("subtitle_group_source", v || null)
              if (!v) onChange("subtitle_group_value", null)
            }}
            onValueChange={(v) => onChange("subtitle_group_value", v || null)}
          />
          <FieldSourceInput
            label={t("parsers.resolution", "Resolution")}
            source={form.resolution_source}
            value={form.resolution_value ?? ""}
            onSourceChange={(v) => {
              onChange("resolution_source", v || null)
              if (!v) onChange("resolution_value", null)
            }}
            onValueChange={(v) => onChange("resolution_value", v || null)}
          />
          <FieldSourceInput
            label={t("parsers.season", "Season")}
            source={form.season_source}
            value={form.season_value ?? ""}
            onSourceChange={(v) => {
              onChange("season_source", v || null)
              if (!v) onChange("season_value", null)
            }}
            onValueChange={(v) => onChange("season_value", v || null)}
          />
        </div>

        <div className="grid grid-cols-3 gap-3">
          <FieldSourceInput
            label={t("parsers.year", "Year")}
            source={form.year_source}
            value={form.year_value ?? ""}
            onSourceChange={(v) => {
              onChange("year_source", v || null)
              if (!v) onChange("year_value", null)
            }}
            onValueChange={(v) => onChange("year_value", v || null)}
          />
        </div>
      </div>
    </div>
  )
```

**Step 3: Commit**

```bash
git add frontend/src/components/shared/ParserForm.tsx
git commit -m "feat: restructure parser form layout — 3 per row, AI buttons at top"
```

---

### Task 5: Frontend — Update ParsersPage to use FullScreenDialog for create/edit

**Files:**
- Modify: `frontend/src/pages/parsers/ParsersPage.tsx` (full rewrite)

**Step 1: Rewrite ParsersPage**

The page should:
1. Remove the old small `Dialog` for create
2. Remove the old preview `Dialog`
3. Add `FullScreenDialog` for both create and edit
4. Click table row → open edit dialog with pre-filled form
5. Keep the Add Parser button → open create dialog with empty form
6. Remove separate Preview button (preview is embedded in the form dialog)
7. Remove separate `ParserAIButtons` usage (now inside `ParserFormFields`)
8. Add `updateParser` mutation
9. Add `exclude_parser_id` to preview when editing (so the current parser isn't double-counted)

Key state changes:
```typescript
// Replace createOpen + previewOpen with:
const [editTarget, setEditTarget] = useState<Record<string, unknown> | null>(null) // null = create mode
const [dialogOpen, setDialogOpen] = useState(false)
```

Table row click handler:
```typescript
onClick={() => {
  setEditTarget(item)
  setForm(parserToPreviewForm(item))
  setPreview(null)
  setDialogOpen(true)
}}
```

Add Parser button:
```typescript
onClick={() => {
  setEditTarget(null)
  setForm({ ...EMPTY_PARSER_FORM })
  setPreview(null)
  setDialogOpen(true)
}}
```

Dialog content uses `FullScreenDialog` with embedded `ParserFormFields` (which now has AI buttons built in) + `PreviewResults` below + Save/Create button at bottom.

For edit mode, call `updateParser(editTarget.parser_id, buildParserRequest(form))`.
For create mode, call `createParser(buildParserRequest(form))`.

Pass `exclude_parser_id` to preview request when editing, so the existing parser is excluded from "before" list.

**Step 2: Commit**

```bash
git add frontend/src/pages/parsers/ParsersPage.tsx
git commit -m "feat: ParsersPage uses FullScreenDialog for create/edit parsers"
```

---

### Task 6: Frontend — Update ParserEditor to support editing existing parsers

**Files:**
- Modify: `frontend/src/components/shared/ParserEditor.tsx`

**Step 1: Add edit support**

Changes:
1. Add `editTarget` state: `useState<TitleParser | null>(null)`
2. Add `updateParser` mutation
3. Each parser item in the list gets an Edit button (Pencil icon) next to the Delete button
4. Clicking Edit: set `editTarget`, populate form with parser data, show form
5. When `editTarget` is set, the form Save button calls `updateParser` instead of `createParser`
6. Remove separate `ParserAIButtons` from the toggle area (now inside `ParserFormFields`)
7. Pass `onImport`, `targetType`, `targetId` to `ParserFormFields`
8. Pass `exclude_parser_id` to preview when editing

Key additions:
```typescript
const [editTarget, setEditTarget] = useState<TitleParser | null>(null)

const { mutate: updateParser, isLoading: updating } = useEffectMutation(
  (req: { id: number; data: Record<string, unknown> }) =>
    Effect.flatMap(CoreApi, (api) => api.updateParser(req.id, req.data)),
)

const handleEdit = useCallback((parser: TitleParser) => {
  setEditTarget(parser)
  setForm({
    name: parser.name,
    priority: parser.priority,
    condition_regex: parser.condition_regex,
    parse_regex: parser.parse_regex,
    anime_title_source: parser.anime_title_source,
    anime_title_value: parser.anime_title_value,
    episode_no_source: parser.episode_no_source,
    episode_no_value: parser.episode_no_value,
    series_no_source: parser.series_no_source,
    series_no_value: parser.series_no_value,
    subtitle_group_source: parser.subtitle_group_source,
    subtitle_group_value: parser.subtitle_group_value,
    resolution_source: parser.resolution_source,
    resolution_value: parser.resolution_value,
    season_source: parser.season_source,
    season_value: parser.season_value,
    year_source: parser.year_source,
    year_value: parser.year_value,
  })
  setShowForm(true)
}, [])

const handleSave = useCallback(async () => {
  if (editTarget) {
    await updateParser({ id: editTarget.parser_id, data: buildParserRequest(form) })
  } else {
    await createParser()
  }
  setForm(EMPTY_PARSER_FORM)
  setShowForm(false)
  setEditTarget(null)
  setPreview(null)
  refetch()
  onParsersChange?.()
}, [editTarget, form, updateParser, createParser, refetch, onParsersChange])
```

The form button label changes based on mode:
```tsx
<Button size="sm" onClick={handleSave} disabled={(creating || updating) || !form.name || !form.condition_regex || !form.parse_regex}>
  {editTarget
    ? (updating ? t("parser.saving") : t("parser.save"))
    : (creating ? t("common.creating") : t("parser.create"))}
</Button>
```

Each parser list item gets a Pencil edit button:
```tsx
<Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => handleEdit(parser)}>
  <Pencil className="h-4 w-4" />
</Button>
```

**Step 2: Commit**

```bash
git add frontend/src/components/shared/ParserEditor.tsx
git commit -m "feat: ParserEditor supports editing existing parsers"
```

---

### Task 7: Frontend — Update export prompt with `$N` explanation and smart priority

**Files:**
- Modify: `frontend/src/components/shared/ParserForm.tsx` (`handleExportPrompt` function, around line 372-442)

**Step 1: Update the prompt template**

Replace the prompt string in `handleExportPrompt` with:

```typescript
    const prompt = `I need you to create a parser configuration JSON for an anime RSS title parser.

## Parser JSON Format
\`\`\`json
{
  "name": "string - descriptive name for this parser",
  "condition_regex": "string - regex pattern to match titles this parser should handle. Make this as strict and specific as possible.",
  "parse_regex": "string - regex with numbered capture groups to extract fields",
  "priority": "number - see Priority Rules below",
  "anime_title_source": "'regex' or 'static' - how to determine the anime title",
  "anime_title_value": "string - capture group ref (e.g. $1) if regex, or fixed value if static",
  "episode_no_source": "'regex' or 'static' - how to determine the episode number",
  "episode_no_value": "string - capture group ref or fixed value",
  "series_no_source": "'regex', 'static', or null - season/series number (optional)",
  "series_no_value": "string or null",
  "subtitle_group_source": "'regex', 'static', or null - subtitle group (optional)",
  "subtitle_group_value": "string or null",
  "resolution_source": "'regex', 'static', or null - video resolution (optional)",
  "resolution_value": "string or null",
  "season_source": "'regex', 'static', or null - aired season (optional)",
  "season_value": "string or null",
  "year_source": "'regex', 'static', or null - year (optional)",
  "year_value": "string or null"
}
\`\`\`

## Capture Group Index Convention
When source is "regex", the value uses \`$N\` format where N is the capture group index:
- \`$1\` = 1st capture group in parse_regex
- \`$2\` = 2nd capture group in parse_regex
- etc.
The backend reads \`$1\` as index 1.

## Priority Rules
- If this parser targets a **single specific anime** (e.g. one show title), set priority to **9999**. The condition_regex should be very strict, matching only that specific anime's naming pattern.
- If this parser is **general purpose** (handles many different anime), set priority to **50**.
- Analyze the titles below to determine which case applies.

## Raw Item Titles
${titles.length > 0 ? titles.map((t) => \`- \${t}\`).join("\\n") : "(no titles available)"}

## Instructions
Analyze the titles above and generate a parser JSON that can:
1. Match these titles with \`condition_regex\` — make it as strict as possible
2. Extract anime_title, episode_no, and other fields using \`parse_regex\` with numbered capture groups
3. Set appropriate source/value pairs for each extracted field using \`$N\` notation
4. Use null for optional fields that cannot be reliably extracted
5. Determine priority based on the Priority Rules above

Return ONLY the JSON object, no extra text.`
```

**Step 2: Commit**

```bash
git add frontend/src/components/shared/ParserForm.tsx
git commit -m "feat: export prompt explains \$N index convention and smart priority"
```

---

### Task 8: Remove stale ParserAIButtons from callers

**Files:**
- Modify: `frontend/src/pages/parsers/ParsersPage.tsx` (already done in Task 5)
- Modify: `frontend/src/components/shared/ParserEditor.tsx` (already done in Task 6)

This is handled by Tasks 5 and 6. No separate commit needed.

---

## Execution Notes

- Tasks 1 (backend) and 2-7 (frontend) are independent and can be parallelized.
- Task 4 should be done before Tasks 5 and 6, as they depend on the new `ParserFormFields` props.
- Task 2 (API) should be done before Tasks 5 and 6.
- Task 3 (i18n) should be done before Tasks 5 and 6.
- Recommended order: 1 → 2 → 3 → 4 → 7 → 5 → 6
