import { useState, useCallback, useRef, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Trash2, Plus, ChevronDown, ChevronUp, AlertTriangle } from "lucide-react"
import { ConfirmDialog } from "./ConfirmDialog"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import type { TitleParser, ParserPreviewResponse } from "@/schemas/parser"
import { cn } from "@/lib/utils"

interface ParserEditorProps {
  createdFromType: "global" | "anime" | "anime_series" | "subtitle_group" | "subscription"
  createdFromId: number | null
  onParsersChange?: () => void
}

const EMPTY_FORM = {
  name: "",
  condition_regex: "",
  parse_regex: "",
  priority: 50,
  anime_title_source: "regex" as string,
  anime_title_value: "",
  episode_no_source: "regex" as string,
  episode_no_value: "",
  series_no_source: null as string | null,
  series_no_value: null as string | null,
  subtitle_group_source: null as string | null,
  subtitle_group_value: null as string | null,
  resolution_source: null as string | null,
  resolution_value: null as string | null,
  season_source: null as string | null,
  season_value: null as string | null,
  year_source: null as string | null,
  year_value: null as string | null,
}

export function ParserEditor({
  createdFromType,
  createdFromId,
  onParsersChange,
}: ParserEditorProps) {
  const { t } = useTranslation()
  const [showForm, setShowForm] = useState(false)
  const [form, setForm] = useState(EMPTY_FORM)
  const [preview, setPreview] = useState<ParserPreviewResponse | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<TitleParser | null>(null)
  const [searchQuery, setSearchQuery] = useState("")
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Load parsers for this target
  const { data: parsers, refetch } = useEffectQuery(
    () =>
      Effect.flatMap(CoreApi, (api) =>
        api.getParsers({ created_from_type: createdFromType, created_from_id: createdFromId ?? undefined }),
      ),
    [createdFromType, createdFromId],
  )

  const { mutate: createParser, isLoading: creating } = useEffectMutation(
    () =>
      Effect.flatMap(CoreApi, (api) =>
        api.createParser({
          ...form,
          is_enabled: true,
          created_from_type: createdFromType,
          created_from_id: createdFromId,
        }),
      ),
  )

  const { mutate: deleteParser, isLoading: deleting } = useEffectMutation(
    (id: number) =>
      Effect.flatMap(CoreApi, (api) => api.deleteParser(id)),
  )

  // Debounced auto-preview (300ms)
  useEffect(() => {
    if (!form.condition_regex || !form.parse_regex) {
      setPreview(null)
      return
    }

    if (debounceRef.current) clearTimeout(debounceRef.current)

    debounceRef.current = setTimeout(() => {
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) =>
          api.previewParser({
            target_type: createdFromType,
            target_id: createdFromId,
            condition_regex: form.condition_regex,
            parse_regex: form.parse_regex,
            priority: form.priority,
            anime_title_source: form.anime_title_source,
            anime_title_value: form.anime_title_value,
            episode_no_source: form.episode_no_source,
            episode_no_value: form.episode_no_value,
            series_no_source: form.series_no_source,
            series_no_value: form.series_no_value,
            subtitle_group_source: form.subtitle_group_source,
            subtitle_group_value: form.subtitle_group_value,
            resolution_source: form.resolution_source,
            resolution_value: form.resolution_value,
            season_source: form.season_source,
            season_value: form.season_value,
            year_source: form.year_source,
            year_value: form.year_value,
          }),
        ),
      ).then(setPreview).catch((e) => {
        console.error("Parser preview failed:", e)
        setPreview(null)
      })
    }, 300)

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [form, createdFromType, createdFromId])

  const handleCreate = useCallback(async () => {
    await createParser()
    setForm(EMPTY_FORM)
    setShowForm(false)
    setPreview(null)
    refetch()
    onParsersChange?.()
  }, [createParser, refetch, onParsersChange])

  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteTarget) return
    await deleteParser(deleteTarget.parser_id)
    setDeleteTarget(null)
    refetch()
    onParsersChange?.()
  }, [deleteTarget, deleteParser, refetch, onParsersChange])

  const updateForm = (key: string, value: string | number | null) =>
    setForm((prev) => ({ ...prev, [key]: value }))

  return (
    <div className="space-y-4">
      {/* Existing parsers */}
      {parsers && parsers.length > 0 && (
        <div className="space-y-2">
          {parsers.map((parser) => (
            <div
              key={parser.parser_id}
              className="flex items-center gap-2 rounded-md border px-3 py-2 text-sm"
            >
              <Badge variant="secondary">P{parser.priority}</Badge>
              <span className="font-medium">{parser.name}</span>
              <code className="flex-1 text-xs text-muted-foreground font-mono truncate">
                {parser.condition_regex}
              </code>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={() => setDeleteTarget(parser)}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          ))}
        </div>
      )}

      {/* Toggle form */}
      <Button
        variant="outline"
        size="sm"
        onClick={() => setShowForm(!showForm)}
      >
        {showForm ? (
          <ChevronUp className="h-4 w-4 mr-1" />
        ) : (
          <Plus className="h-4 w-4 mr-1" />
        )}
        {t("parser.addParser", "Add Parser")}
      </Button>

      {/* Add form */}
      {showForm && (
        <div className="space-y-3 rounded-md border p-4">
          {/* Name + Priority */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <Label className="text-xs">{t("common.name", "Name")}</Label>
              <Input
                value={form.name}
                onChange={(e) => updateForm("name", e.target.value)}
                placeholder="Parser name"
              />
            </div>
            <div>
              <Label className="text-xs">{t("parsers.priority", "Priority")}</Label>
              <Input
                type="number"
                value={form.priority}
                onChange={(e) => updateForm("priority", parseInt(e.target.value) || 0)}
              />
            </div>
          </div>

          {/* Condition Regex */}
          <div>
            <Label className="text-xs">{t("parsers.conditionRegex", "Condition Regex")}</Label>
            <Input
              className="font-mono text-sm"
              value={form.condition_regex}
              onChange={(e) => updateForm("condition_regex", e.target.value)}
              placeholder={t("parsers.conditionRegexPlaceholder", "Must match to activate this parser")}
            />
          </div>

          {/* Parse Regex */}
          <div>
            <Label className="text-xs">{t("parsers.parseRegex", "Parse Regex")}</Label>
            <Input
              className="font-mono text-sm"
              value={form.parse_regex}
              onChange={(e) => updateForm("parse_regex", e.target.value)}
              placeholder={t("parsers.parseRegexPlaceholder", "Capture groups for field extraction")}
            />
          </div>

          {/* Field extraction */}
          <div className="space-y-1">
            <Label className="text-xs font-semibold">{t("parsers.fieldExtraction", "Field Extraction")}</Label>

            {/* Required fields: anime_title, episode_no */}
            <div className="grid grid-cols-2 gap-3">
              <FieldSourceInput
                label={t("parsers.animeTitle", "Anime Title")}
                source={form.anime_title_source}
                value={form.anime_title_value}
                onSourceChange={(v) => updateForm("anime_title_source", v)}
                onValueChange={(v) => updateForm("anime_title_value", v)}
                required
              />
              <FieldSourceInput
                label={t("parsers.episodeNo", "Episode No")}
                source={form.episode_no_source}
                value={form.episode_no_value}
                onSourceChange={(v) => updateForm("episode_no_source", v)}
                onValueChange={(v) => updateForm("episode_no_value", v)}
                required
              />
            </div>

            {/* Optional fields */}
            <div className="grid grid-cols-2 gap-3">
              <FieldSourceInput
                label={t("parsers.seriesNo", "Series No")}
                source={form.series_no_source}
                value={form.series_no_value ?? ""}
                onSourceChange={(v) => {
                  updateForm("series_no_source", v || null)
                  if (!v) updateForm("series_no_value", null)
                }}
                onValueChange={(v) => updateForm("series_no_value", v || null)}
              />
              <FieldSourceInput
                label={t("parsers.subtitleGroup", "Subtitle Group")}
                source={form.subtitle_group_source}
                value={form.subtitle_group_value ?? ""}
                onSourceChange={(v) => {
                  updateForm("subtitle_group_source", v || null)
                  if (!v) updateForm("subtitle_group_value", null)
                }}
                onValueChange={(v) => updateForm("subtitle_group_value", v || null)}
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <FieldSourceInput
                label={t("parsers.resolution", "Resolution")}
                source={form.resolution_source}
                value={form.resolution_value ?? ""}
                onSourceChange={(v) => {
                  updateForm("resolution_source", v || null)
                  if (!v) updateForm("resolution_value", null)
                }}
                onValueChange={(v) => updateForm("resolution_value", v || null)}
              />
              <FieldSourceInput
                label={t("parsers.season", "Season")}
                source={form.season_source}
                value={form.season_value ?? ""}
                onSourceChange={(v) => {
                  updateForm("season_source", v || null)
                  if (!v) updateForm("season_value", null)
                }}
                onValueChange={(v) => updateForm("season_value", v || null)}
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <FieldSourceInput
                label={t("parsers.year", "Year")}
                source={form.year_source}
                value={form.year_value ?? ""}
                onSourceChange={(v) => {
                  updateForm("year_source", v || null)
                  if (!v) updateForm("year_value", null)
                }}
                onValueChange={(v) => updateForm("year_value", v || null)}
              />
            </div>
          </div>

          {/* Create button */}
          <Button size="sm" onClick={handleCreate} disabled={creating || !form.name || !form.condition_regex || !form.parse_regex}>
            {t("parser.create", "Create")}
          </Button>

          {/* Live preview results */}
          {preview && (
            <div className="space-y-2">
              {!preview.condition_regex_valid && (
                <p className="text-sm text-destructive">
                  {t("parsers.regexError", "Regex error")}: {preview.regex_error}
                </p>
              )}
              {!preview.parse_regex_valid && (
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
                          {/* Row 1: status badge + full title */}
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
                          {/* Row 2: parsed details */}
                          <div className="flex flex-wrap gap-x-3 gap-y-0.5 mt-1 ml-1 text-muted-foreground">
                            <span>{t("parsers.matchedBy", "Matched by")}: <span className="text-foreground">{result.after_matched_by ?? "—"}</span></span>
                            {result.parse_result && (
                              <>
                                <span>{t("parsers.animeTitle", "Anime")}: <span className="text-foreground">{result.parse_result.anime_title}</span></span>
                                <span>Ep: <span className="text-foreground">{result.parse_result.episode_no}</span></span>
                                {result.parse_result.series_no != null && <span>S{result.parse_result.series_no}</span>}
                                {result.parse_result.season && <span>{t("parsers.season", "Season")}: <span className="text-foreground">{result.parse_result.season}</span></span>}
                                {result.parse_result.subtitle_group && <span>{t("parsers.subtitleGroup", "Group")}: <span className="text-foreground">{result.parse_result.subtitle_group}</span></span>}
                                {result.parse_result.resolution && <span>{result.parse_result.resolution}</span>}
                                {result.parse_result.year && <span>{result.parse_result.year}</span>}
                              </>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  </>
                ) : (
                  <p className="text-sm text-muted-foreground">{t("common.noMatch", "No matching items")}</p>
                )
              )}
            </div>
          )}
        </div>
      )}

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null)
        }}
        title={t("parser.confirmDelete", "Delete parser?")}
        description={deleteTarget ? `${deleteTarget.name} (priority: ${deleteTarget.priority})` : ""}
        onConfirm={handleDeleteConfirm}
        loading={deleting}
      />
    </div>
  )
}

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
