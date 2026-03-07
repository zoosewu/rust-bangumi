import { useState, useCallback } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { Badge } from "@/components/ui/badge"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Import, FileText, AlertTriangle } from "lucide-react"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import { cn } from "@/lib/utils"
import { CopyButton } from "@/components/shared/CopyButton"
import { RegexInput } from "@/components/shared/RegexInput"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import type { ParserPreviewResponse } from "@/schemas/parser"

// --- Types ---

export type ParserFormState = {
  name: string
  condition_regex: string
  parse_regex: string
  priority: number
  anime_title_source: string
  anime_title_value: string
  episode_no_source: string
  episode_no_value: string
  episode_end_source: string | null
  episode_end_value: string | null
  series_no_source: string | null
  series_no_value: string | null
  subtitle_group_source: string | null
  subtitle_group_value: string | null
  resolution_source: string | null
  resolution_value: string | null
  season_source: string | null
  season_value: string | null
  year_source: string | null
  year_value: string | null
}

export const EMPTY_PARSER_FORM: ParserFormState = {
  name: "",
  condition_regex: "",
  parse_regex: "",
  priority: 50,
  anime_title_source: "regex",
  anime_title_value: "",
  episode_no_source: "regex",
  episode_no_value: "",
  episode_end_source: null,
  episode_end_value: null,
  series_no_source: null,
  series_no_value: null,
  subtitle_group_source: null,
  subtitle_group_value: null,
  resolution_source: null,
  resolution_value: null,
  season_source: null,
  season_value: null,
  year_source: null,
  year_value: null,
}

// --- Helpers ---

export function buildParserRequest(form: ParserFormState): Record<string, unknown> {
  const req: Record<string, unknown> = {
    name: form.name,
    priority: form.priority,
    condition_regex: form.condition_regex,
    parse_regex: form.parse_regex,
    anime_title_source: form.anime_title_source,
    anime_title_value: form.anime_title_value,
    episode_no_source: form.episode_no_source,
    episode_no_value: form.episode_no_value,
  }
  const optionalFields = [
    ["episode_end", form.episode_end_source, form.episode_end_value],
    ["series_no", form.series_no_source, form.series_no_value],
    ["subtitle_group", form.subtitle_group_source, form.subtitle_group_value],
    ["resolution", form.resolution_source, form.resolution_value],
    ["season", form.season_source, form.season_value],
    ["year", form.year_source, form.year_value],
  ] as const
  for (const [field, source, value] of optionalFields) {
    if (source) {
      req[`${field}_source`] = source
      req[`${field}_value`] = value
    }
  }
  return req
}

// --- FieldSourceInput ---

function FieldSourceInput({
  label,
  source,
  value,
  onSourceChange,
  onValueChange,
  required,
}: {
  label: string
  source: string | null
  value: string
  onSourceChange: (v: string) => void
  onValueChange: (v: string) => void
  required?: boolean
}) {
  return (
    <div className="space-y-1">
      <Label className="text-xs">{label}</Label>
      <div className="flex gap-2">
        <Select value={source ?? "none"} onValueChange={(v) => onSourceChange(v === "none" ? "" : v)}>
          <SelectTrigger className="w-24">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {!required && <SelectItem value="none">—</SelectItem>}
            <SelectItem value="regex">regex</SelectItem>
            <SelectItem value="static">static</SelectItem>
          </SelectContent>
        </Select>
        <Input
          className="font-mono text-sm"
          value={value}
          onChange={(e) => onValueChange(e.target.value)}
          placeholder={source === "static" ? "Fixed value" : "Capture group (e.g. $1)"}
          disabled={!source || source === "none"}
        />
      </div>
    </div>
  )
}

// --- ParserFormFields ---

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
        <RegexInput
          value={form.condition_regex}
          onChange={(v) => onChange("condition_regex", v)}
          placeholder={t("parsers.conditionRegexPlaceholder", "Must match to activate this parser")}
        />
      </div>

      {/* Parse Regex */}
      <div>
        <Label className="text-xs">{t("parsers.parseRegex", "Parse Regex")}</Label>
        <RegexInput
          value={form.parse_regex}
          onChange={(v) => onChange("parse_regex", v)}
          placeholder={t("parsers.parseRegexPlaceholder", "Capture groups for field extraction")}
        />
      </div>

      {/* Field extraction — 3 per row */}
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
            label={t("parsers.episodeEnd", "Episode End")}
            source={form.episode_end_source}
            value={form.episode_end_value ?? ""}
            onSourceChange={(v) => onChange("episode_end_source", v === "none" ? null : v)}
            onValueChange={(v) => onChange("episode_end_value", v)}
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
}

// --- ParserAIButtons ---

export function ParserAIButtons({
  onImport,
  targetType,
  targetId,
}: {
  onImport: (form: ParserFormState) => void
  targetType: string
  targetId: number | null
}) {
  const { t } = useTranslation()

  // Import state
  const [importOpen, setImportOpen] = useState(false)
  const [importJson, setImportJson] = useState("")
  const [importError, setImportError] = useState("")

  // Export Prompt state
  const [promptOpen, setPromptOpen] = useState(false)
  const [promptText, setPromptText] = useState("")
  const [promptLoading, setPromptLoading] = useState(false)

  const handleImport = useCallback(() => {
    try {
      const parsed = JSON.parse(importJson)
      if (!parsed.name || !parsed.condition_regex || !parsed.parse_regex) {
        setImportError(t("parser.importError", "Missing required fields: name, condition_regex, parse_regex"))
        return
      }
      onImport({
        name: parsed.name ?? "",
        condition_regex: parsed.condition_regex ?? "",
        parse_regex: parsed.parse_regex ?? "",
        priority: parsed.priority ?? 50,
        anime_title_source: parsed.anime_title_source ?? "regex",
        anime_title_value: parsed.anime_title_value ?? "",
        episode_no_source: parsed.episode_no_source ?? "regex",
        episode_no_value: parsed.episode_no_value ?? "",
        episode_end_source: parsed.episode_end_source ?? null,
        episode_end_value: parsed.episode_end_value ?? null,
        series_no_source: parsed.series_no_source ?? null,
        series_no_value: parsed.series_no_value ?? null,
        subtitle_group_source: parsed.subtitle_group_source ?? null,
        subtitle_group_value: parsed.subtitle_group_value ?? null,
        resolution_source: parsed.resolution_source ?? null,
        resolution_value: parsed.resolution_value ?? null,
        season_source: parsed.season_source ?? null,
        season_value: parsed.season_value ?? null,
        year_source: parsed.year_source ?? null,
        year_value: parsed.year_value ?? null,
      })
      setImportOpen(false)
      setImportJson("")
      setImportError("")
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : "Unknown error"
      setImportError(
        `Invalid JSON: ${errorMsg}. Make sure backslashes in regex are DOUBLED (e.g., "\\\\[" not "\\[").`
      )
    }
  }, [importJson, t, onImport])

  const handleExportPrompt = useCallback(async () => {
    setPromptLoading(true)
    let titles: string[] = []
    try {
      const result = await AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) =>
          api.previewParser({
            target_type: targetType,
            target_id: targetId,
            condition_regex: ".*",
            parse_regex: "(?P<title>.*)",
            priority: 50,
            anime_title_source: "regex",
            anime_title_value: "$title",
            episode_no_source: "static",
            episode_no_value: "0",
            series_no_source: null,
            series_no_value: null,
            subtitle_group_source: null,
            subtitle_group_value: null,
            resolution_source: null,
            resolution_value: null,
            episode_end_source: null,
            episode_end_value: null,
            season_source: null,
            season_value: null,
            year_source: null,
            year_value: null,
          }),
        ),
      )
      titles = result.results.map((r) => r.title)
    } catch {
      // fallback to empty
    }

    const prompt = `I need you to create a parser configuration JSON for an anime RSS title parser.

## ⚠️ CRITICAL: JSON Escaping for Regex
In JSON strings, backslashes MUST be escaped with double backslashes (\\\\):
- Regex: \`\\[Group\\]\` → JSON: "condition_regex": "\\\\[Group\\\\]"
- Regex: \`\\d+\` → JSON: "parse_regex": "\\\\d+"
- Regex: \`\\s*\` → JSON: "condition_regex": "\\\\s*"

Every single backslash in your regex pattern MUST be doubled in JSON. This is non-negotiable.

## Parser JSON Format
\`\`\`json
{
  "name": "string - descriptive name for this parser",
  "condition_regex": "string - regex pattern (with ESCAPED BACKSLASHES) to match titles this parser should handle. Make this as strict and specific as possible.",
  "parse_regex": "string - regex with numbered capture groups (with ESCAPED BACKSLASHES) to extract fields",
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
  "year_value": "string or null",
  "episode_end_source": "'regex', 'static', or null - end episode for batch torrents covering a range e.g. 01-12 (optional)",
  "episode_end_value": "string or null - e.g. $3 if parse_regex has a 3rd capture group for the end episode number"
}
\`\`\`

## Capture Group Index Convention
When source is "regex", the value uses \`$N\` format where N is the capture group index:
- \`$1\` = 1st capture group in parse_regex
- \`$2\` = 2nd capture group in parse_regex
- etc.
The backend reads \`$1\` as index 1.

## Regex Escaping Examples
Remember to DOUBLE-escape all backslashes when generating JSON:
- \`[Group]\` patterns: use "\\\\[" and "\\\\]"
- \`\\d\` for digits: use "\\\\d"
- \`\\s\` for whitespace: use "\\\\s"
- \`\\w\` for word chars: use "\\\\w"

## Priority Rules
- If this parser targets a **single specific anime** (e.g. one show title), set priority to **9999**. The condition_regex should be very strict, matching only that specific anime's naming pattern.
- If this parser is **general purpose** (handles many different anime), set priority to **50**.
- Analyze the titles below to determine which case applies.

## Raw Item Titles
${titles.length > 0 ? titles.map((t) => `- ${t}`).join("\n") : "(no titles available)"}

## Instructions
Analyze the titles above and generate a parser JSON that can:
1. Match these titles with \`condition_regex\` — make it as strict as possible
2. Extract anime_title, episode_no, and other fields using \`parse_regex\` with numbered capture groups
3. Set appropriate source/value pairs for each extracted field using \`$N\` notation
4. Use null for optional fields that cannot be reliably extracted. If titles show an episode range (e.g. \`01-12\`, \`EP01-EP12\`), set \`episode_end_source\` and \`episode_end_value\` to capture the upper bound.
5. Determine priority based on the Priority Rules above

Return ONLY the JSON object, no extra text.`

    setPromptText(prompt)
    setPromptLoading(false)
    setPromptOpen(true)
  }, [targetType, targetId])

  return (
    <>
      <Button
        variant="outline"
        size="sm"
        onClick={() => {
          setImportJson("")
          setImportError("")
          setImportOpen(true)
        }}
      >
        <Import className="h-4 w-4 mr-1" />
        {t("parser.import", "Import")}
      </Button>
      <Button
        variant="outline"
        size="sm"
        onClick={handleExportPrompt}
        disabled={promptLoading}
      >
        <FileText className="h-4 w-4 mr-1" />
        {t("parser.exportPrompt", "Export Prompt")}
      </Button>

      {/* Import Dialog */}
      <FullScreenDialog
        open={importOpen}
        onOpenChange={setImportOpen}
        title={t("parser.importTitle", "Import Parser JSON")}
        description={t("parser.importDescription", "Paste AI-generated parser JSON to auto-fill the form fields.")}
        size="md"
        footer={
          <>
            <Button variant="outline" onClick={() => setImportOpen(false)}>
              {t("common.cancel", "Cancel")}
            </Button>
            <Button onClick={handleImport} disabled={!importJson.trim()}>
              {t("parser.import", "Import")}
            </Button>
          </>
        }
      >
        <div className="space-y-2">
          <Textarea
            className="font-mono text-sm min-h-[200px]"
            value={importJson}
            onChange={(e) => {
              setImportJson(e.target.value)
              setImportError("")
            }}
            placeholder='{"name": "...", "condition_regex": "...", ...}'
          />
          {importError && (
            <p className="text-sm text-destructive">{importError}</p>
          )}
        </div>
      </FullScreenDialog>

      {/* Export Prompt Dialog */}
      <FullScreenDialog
        open={promptOpen}
        onOpenChange={setPromptOpen}
        title={t("parser.exportPrompt", "Export Prompt")}
        size="md"
        footer={
          <>
            <Button variant="outline" onClick={() => setPromptOpen(false)}>
              {t("common.cancel", "Cancel")}
            </Button>
            <CopyButton text={promptText} label={t("parser.copyPrompt", "Copy")} copiedLabel={t("parser.copied", "Copied!")} />
          </>
        }
      >
        <Textarea
          className="font-mono text-xs resize-none h-full min-h-[400px]"
          value={promptText}
          readOnly
        />
      </FullScreenDialog>
    </>
  )
}

// --- ParserPreviewSection ---

/**
 * 顯示 previewParser API 回傳的結果，供 ParserEditor 及 WizardPendingList 共用。
 */
export function ParserPreviewSection({ preview }: { preview: ParserPreviewResponse | null }) {
  const { t } = useTranslation()
  const [searchQuery, setSearchQuery] = useState("")

  if (!preview) return null

  return (
    <div className="space-y-2">
      {(!preview.condition_regex_valid || !preview.parse_regex_valid) && (
        <p className="text-sm text-destructive">
          {t("parsers.regexError", "Regex error")}: {preview.regex_error}
        </p>
      )}
      {preview.condition_regex_valid && preview.parse_regex_valid && (
        preview.results.length > 0 ? (
          <>
            <Input
              placeholder={t("parsers.searchPlaceholder", "Search titles...")}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="text-sm"
            />
            <div className="rounded-md border divide-y">
              {preview.results
                .filter((r) => !searchQuery || r.title.toLowerCase().includes(searchQuery.toLowerCase()))
                .map((result, i) => (
                  <div
                    key={i}
                    className={cn(
                      "px-3 py-2 text-xs",
                      result.is_newly_matched && "bg-green-50 dark:bg-green-950/30",
                      result.is_override && "bg-yellow-50 dark:bg-yellow-950/30",
                    )}
                  >
                    <div className="flex items-start gap-2">
                      <span className="shrink-0 mt-0.5">
                        {result.is_newly_matched && (
                          <Badge variant="default" className="text-xs">
                            {t("parsers.newlyMatched", "new")}
                          </Badge>
                        )}
                        {result.is_override && (
                          <Badge variant="secondary" className="text-xs">
                            <AlertTriangle className="h-3 w-3 mr-1" />
                            {t("parsers.override", "override")}
                          </Badge>
                        )}
                        {!result.is_newly_matched && !result.is_override && (
                          <Badge variant="outline" className="text-xs text-muted-foreground">
                            {result.after_matched_by ? t("parsers.existing", "existing") : t("parsers.unmatched", "—")}
                          </Badge>
                        )}
                      </span>
                      <span className="font-mono break-all">{result.title}</span>
                    </div>
                    <div className="grid grid-cols-[auto_1fr_auto_auto_auto_auto_auto_auto] gap-x-3 mt-1 ml-1 text-xs text-muted-foreground">
                      <span className="truncate">{t("parsers.matchedBy", "Matched by")}: <span className="text-foreground">{result.after_matched_by ?? "—"}</span></span>
                      <span className="truncate">{t("parsers.animeTitle", "Anime")}: <span className={cn("text-foreground", !result.parse_result?.anime_title && "text-destructive")}>{result.parse_result?.anime_title || "—"}</span></span>
                      <span className="whitespace-nowrap">{t("parsers.episodeNo", "Ep")}: <span className={cn("text-foreground", result.parse_result?.episode_no == null && "text-destructive")}>{result.parse_result?.episode_end != null ? `${result.parse_result.episode_no}-${result.parse_result.episode_end}` : (result.parse_result?.episode_no ?? "—")}</span></span>
                      <span className="whitespace-nowrap">S: <span className="text-foreground">{result.parse_result?.series_no ?? "—"}</span></span>
                      <span className="whitespace-nowrap">{t("parsers.season", "Season")}: <span className="text-foreground">{result.parse_result?.season || "—"}</span></span>
                      <span className="whitespace-nowrap">{t("parsers.subtitleGroup", "Group")}: <span className="text-foreground">{result.parse_result?.subtitle_group || "—"}</span></span>
                      <span className="whitespace-nowrap">{t("parsers.resolution", "Res")}: <span className="text-foreground">{result.parse_result?.resolution || "—"}</span></span>
                      <span className="whitespace-nowrap">{t("parsers.year", "Year")}: <span className="text-foreground">{result.parse_result?.year || "—"}</span></span>
                    </div>
                    {result.parse_error && (
                      <div className="mt-1 ml-1 text-xs text-destructive">
                        {t("parsers.parseError", "Parse error")}: {result.parse_error}
                      </div>
                    )}
                  </div>
                ))}
            </div>
          </>
        ) : (
          <p className="text-sm text-muted-foreground">{t("common.noMatch", "No matching items")}</p>
        )
      )}
    </div>
  )
}
