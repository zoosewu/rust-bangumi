import { useState, useCallback } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Import, FileText, Copy, Check } from "lucide-react"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"

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

// --- CopyPromptButton ---

function CopyPromptButton({ text, label }: { text: string; label: string }) {
  const { t } = useTranslation()
  const [copied, setCopied] = useState(false)

  return (
    <Button
      onClick={() => {
        navigator.clipboard.writeText(text).then(() => {
          setCopied(true)
          setTimeout(() => setCopied(false), 1500)
        })
      }}
    >
      {copied ? (
        <>
          <Check className="h-4 w-4 mr-1" />
          {label}
        </>
      ) : (
        <>
          <Copy className="h-4 w-4 mr-1" />
          {t("parser.copyPrompt", "Copy")}
        </>
      )}
    </Button>
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
        setImportError(t("parser.importError"))
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
    } catch {
      setImportError(t("parser.importError"))
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
${titles.length > 0 ? titles.map((t) => `- ${t}`).join("\n") : "(no titles available)"}

## Instructions
Analyze the titles above and generate a parser JSON that can:
1. Match these titles with \`condition_regex\` — make it as strict as possible
2. Extract anime_title, episode_no, and other fields using \`parse_regex\` with numbered capture groups
3. Set appropriate source/value pairs for each extracted field using \`$N\` notation
4. Use null for optional fields that cannot be reliably extracted
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
      <Dialog open={importOpen} onOpenChange={setImportOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("parser.importTitle", "Import Parser JSON")}</DialogTitle>
            <DialogDescription>{t("parser.importDescription", "Paste AI-generated parser JSON to auto-fill the form fields.")}</DialogDescription>
          </DialogHeader>
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
          <DialogFooter>
            <Button variant="outline" onClick={() => setImportOpen(false)}>
              {t("common.cancel", "Cancel")}
            </Button>
            <Button onClick={handleImport} disabled={!importJson.trim()}>
              {t("parser.import", "Import")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Export Prompt Dialog */}
      <Dialog open={promptOpen} onOpenChange={setPromptOpen}>
        <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle>{t("parser.exportPrompt", "Export Prompt")}</DialogTitle>
          </DialogHeader>
          <Textarea
            className="font-mono text-xs flex-1 min-h-0 resize-none"
            value={promptText}
            readOnly
          />
          <DialogFooter>
            <Button variant="outline" onClick={() => setPromptOpen(false)}>
              {t("common.cancel", "Cancel")}
            </Button>
            <CopyPromptButton text={promptText} label={t("parser.copied", "Copied!")} />
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
