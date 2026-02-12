import { useState, useCallback } from "react"
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

  // Preview
  const handlePreview = useCallback(() => {
    if (!form.condition_regex || !form.parse_regex) return
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
  }, [form])

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
          <div className="grid grid-cols-2 gap-3">
            <div>
              <Label className="text-xs">Name</Label>
              <Input
                value={form.name}
                onChange={(e) => updateForm("name", e.target.value)}
                placeholder="Parser name"
              />
            </div>
            <div>
              <Label className="text-xs">Priority</Label>
              <Input
                type="number"
                value={form.priority}
                onChange={(e) => updateForm("priority", parseInt(e.target.value) || 0)}
              />
            </div>
          </div>
          <div>
            <Label className="text-xs">Condition Regex</Label>
            <Input
              className="font-mono text-sm"
              value={form.condition_regex}
              onChange={(e) => updateForm("condition_regex", e.target.value)}
              placeholder="Must match to activate this parser"
            />
          </div>
          <div>
            <Label className="text-xs">Parse Regex</Label>
            <Input
              className="font-mono text-sm"
              value={form.parse_regex}
              onChange={(e) => updateForm("parse_regex", e.target.value)}
              placeholder="Capture groups for field extraction"
            />
          </div>

          {/* Field source/value pairs */}
          <div className="grid grid-cols-2 gap-3">
            <FieldSourceInput
              label="Anime Title"
              source={form.anime_title_source}
              value={form.anime_title_value}
              onSourceChange={(v) => updateForm("anime_title_source", v)}
              onValueChange={(v) => updateForm("anime_title_value", v)}
            />
            <FieldSourceInput
              label="Episode No"
              source={form.episode_no_source}
              value={form.episode_no_value}
              onSourceChange={(v) => updateForm("episode_no_source", v)}
              onValueChange={(v) => updateForm("episode_no_value", v)}
            />
          </div>

          <div className="flex gap-2">
            <Button size="sm" variant="outline" onClick={handlePreview}>
              {t("parser.preview", "Preview")}
            </Button>
            <Button size="sm" onClick={handleCreate} disabled={creating || !form.name || !form.condition_regex}>
              {t("parser.create", "Create")}
            </Button>
          </div>

          {/* Preview results */}
          {preview && (
            <div className="space-y-2">
              {!preview.condition_regex_valid && (
                <p className="text-sm text-destructive">
                  Condition regex error: {preview.regex_error}
                </p>
              )}
              {!preview.parse_regex_valid && (
                <p className="text-sm text-destructive">
                  Parse regex error: {preview.regex_error}
                </p>
              )}
              {preview.condition_regex_valid && preview.parse_regex_valid && (
                preview.results.length > 0 ? (
                  <div className="rounded-md border overflow-auto">
                    <table className="w-full text-xs">
                      <thead className="bg-muted sticky top-0">
                        <tr>
                          <th className="px-2 py-1 text-left">Status</th>
                          <th className="px-2 py-1 text-left">Title</th>
                          <th className="px-2 py-1 text-left">Anime</th>
                          <th className="px-2 py-1 text-left">Ep</th>
                          <th className="px-2 py-1 text-left">Season</th>
                          <th className="px-2 py-1 text-left">Group</th>
                          <th className="px-2 py-1 text-left">Res</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y">
                        {preview.results.map((result, i) => (
                          <tr
                            key={i}
                            className={cn(
                              result.is_newly_matched && "bg-green-50 dark:bg-green-950/30",
                              result.is_override && "bg-yellow-50 dark:bg-yellow-950/30",
                            )}
                          >
                            <td className="px-2 py-1">
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
                                <span className="text-muted-foreground">
                                  {result.after_matched_by ? t("parsers.existing", "existing") : t("parsers.unmatched", "—")}
                                </span>
                              )}
                            </td>
                            <td className="px-2 py-1 font-mono truncate max-w-48">{result.title}</td>
                            <td className="px-2 py-1">{result.parse_result?.anime_title ?? "—"}</td>
                            <td className="px-2 py-1">{result.parse_result?.episode_no ?? "—"}</td>
                            <td className="px-2 py-1">{result.parse_result?.season ?? "—"}</td>
                            <td className="px-2 py-1">{result.parse_result?.subtitle_group ?? "—"}</td>
                            <td className="px-2 py-1">{result.parse_result?.resolution ?? "—"}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
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
}: {
  label: string
  source: string
  value: string
  onSourceChange: (v: string) => void
  onValueChange: (v: string) => void
}) {
  return (
    <div className="space-y-1">
      <Label className="text-xs">{label}</Label>
      <div className="flex gap-2">
        <Select value={source} onValueChange={onSourceChange}>
          <SelectTrigger className="w-24">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="regex">regex</SelectItem>
            <SelectItem value="static">static</SelectItem>
          </SelectContent>
        </Select>
        <Input
          className="font-mono text-sm"
          value={value}
          onChange={(e) => onValueChange(e.target.value)}
          placeholder={source === "regex" ? "Group index (e.g. 1)" : "Fixed value"}
        />
      </div>
    </div>
  )
}
