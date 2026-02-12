import { useState, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { ConfirmDialog } from "@/components/shared/ConfirmDialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Card, CardContent } from "@/components/ui/card"
import { RegexInput } from "@/components/shared/RegexInput"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"
import {
  Dialog,
  DialogContent,
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
import { Plus, Trash2, Eye } from "lucide-react"
import type { ParserPreviewResponse } from "@/schemas/parser"

function useDebounce<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value)
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedValue(value), delay)
    return () => clearTimeout(timer)
  }, [value, delay])
  return debouncedValue
}

const DEFAULT_FORM = {
  name: "",
  priority: "50",
  condition_regex: "",
  parse_regex: "",
  anime_title_source: "regex" as string,
  anime_title_value: "",
  episode_no_source: "regex" as string,
  episode_no_value: "",
  series_no_source: "none" as string,
  series_no_value: "",
  subtitle_group_source: "none" as string,
  subtitle_group_value: "",
  resolution_source: "none" as string,
  resolution_value: "",
  season_source: "none" as string,
  season_value: "",
  year_source: "none" as string,
  year_value: "",
}

type FormState = typeof DEFAULT_FORM

export default function ParsersPage() {
  const { t } = useTranslation()
  const [createOpen, setCreateOpen] = useState(false)
  const [form, setForm] = useState<FormState>({ ...DEFAULT_FORM })
  const [deleteTarget, setDeleteTarget] = useState<{ id: number; name: string } | null>(null)
  const [previewOpen, setPreviewOpen] = useState(false)
  const [previewForm, setPreviewForm] = useState<FormState>({ ...DEFAULT_FORM })
  const [preview, setPreview] = useState<ParserPreviewResponse | null>(null)

  const { data: parsers, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getParsers()
      }),
    [],
  )

  const { mutate: createParser, isLoading: creating } = useEffectMutation(
    (req: Record<string, unknown>) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createParser(req)
      }),
  )

  const { mutate: deleteParser, isLoading: deleting } = useEffectMutation(
    (id: number) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.deleteParser(id)
      }),
  )

  const { mutate: fetchPreview } = useEffectMutation(
    (req: Record<string, unknown>) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.previewParser(req)
      }),
  )

  const debouncedPreviewForm = useDebounce(previewForm, 600)

  useEffect(() => {
    if (!previewOpen) return
    if (!debouncedPreviewForm.condition_regex || !debouncedPreviewForm.parse_regex) {
      setPreview(null)
      return
    }
    const req = buildRequest(debouncedPreviewForm)
    fetchPreview(req)
      .then(setPreview)
      .catch(() => setPreview(null))
  }, [debouncedPreviewForm, previewOpen])

  const setField = (setter: React.Dispatch<React.SetStateAction<FormState>>) =>
    (key: keyof FormState, value: string) =>
      setter((prev) => ({ ...prev, [key]: value }))

  const columns: Column<Record<string, unknown>>[] = [
    { key: "parser_id", header: t("common.id"), render: (item) => String(item.parser_id) },
    { key: "name", header: t("common.name"), render: (item) => String(item.name) },
    { key: "priority", header: t("parsers.priority"), render: (item) => String(item.priority) },
    {
      key: "condition_regex",
      header: t("parsers.condition"),
      render: (item) => <code className="text-xs font-mono">{String(item.condition_regex)}</code>,
    },
    {
      key: "is_enabled",
      header: t("parsers.enabled"),
      render: (item) => (item.is_enabled ? t("parsers.yes") : t("parsers.no")),
    },
    {
      key: "actions",
      header: "",
      render: (item) => (
        <div className="flex gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={(e) => {
              e.stopPropagation()
              const p = item as Record<string, unknown>
              const pf: FormState = {
                name: String(p.name ?? ""),
                priority: String(p.priority ?? "50"),
                condition_regex: String(p.condition_regex ?? ""),
                parse_regex: String(p.parse_regex ?? ""),
                anime_title_source: String(p.anime_title_source ?? "regex"),
                anime_title_value: String(p.anime_title_value ?? ""),
                episode_no_source: String(p.episode_no_source ?? "regex"),
                episode_no_value: String(p.episode_no_value ?? ""),
                series_no_source: String(p.series_no_source || "none"),
                series_no_value: String(p.series_no_value ?? ""),
                subtitle_group_source: String(p.subtitle_group_source || "none"),
                subtitle_group_value: String(p.subtitle_group_value ?? ""),
                resolution_source: String(p.resolution_source || "none"),
                resolution_value: String(p.resolution_value ?? ""),
                season_source: String(p.season_source || "none"),
                season_value: String(p.season_value ?? ""),
                year_source: String(p.year_source || "none"),
                year_value: String(p.year_value ?? ""),
              }
              setPreviewForm(pf)
              setPreview(null)
              setPreviewOpen(true)
            }}
          >
            <Eye className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={(e) => {
              e.stopPropagation()
              setDeleteTarget({ id: item.parser_id as number, name: item.name as string })
            }}
          >
            <Trash2 className="h-4 w-4 text-destructive" />
          </Button>
        </div>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("parsers.title")}</h1>
        <div className="flex gap-2">
          <Button
            variant="outline"
            onClick={() => {
              setPreviewForm({ ...DEFAULT_FORM })
              setPreview(null)
              setPreviewOpen(true)
            }}
          >
            <Eye className="h-4 w-4 mr-2" />
            {t("parsers.preview")}
          </Button>
          <Button onClick={() => { setForm({ ...DEFAULT_FORM }); setCreateOpen(true) }}>
            <Plus className="h-4 w-4 mr-2" />
            {t("parsers.addParser")}
          </Button>
        </div>
      </div>

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <DataTable
          columns={columns}
          data={(parsers ?? []) as unknown as Record<string, unknown>[]}
          keyField="parser_id"
        />
      )}

      {/* Create Dialog */}
      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>{t("parsers.addParser")}</DialogTitle>
          </DialogHeader>
          <ParserForm form={form} onChange={setField(setForm)} />
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              disabled={!form.name.trim() || !form.condition_regex.trim() || creating}
              onClick={() => {
                createParser(buildRequest(form)).then(() => {
                  setForm({ ...DEFAULT_FORM })
                  setCreateOpen(false)
                  refetch()
                })
              }}
            >
              {creating ? t("common.creating") : t("common.create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Preview Panel */}
      <Dialog open={previewOpen} onOpenChange={setPreviewOpen}>
        <DialogContent className="max-w-4xl max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>{t("parsers.parserPreview")}</DialogTitle>
          </DialogHeader>
          <ParserForm form={previewForm} onChange={setField(setPreviewForm)} />
          {preview && <PreviewResults preview={preview} />}
        </DialogContent>
      </Dialog>

      {/* Delete Confirm */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title={t("parsers.deleteParser")}
        description={t("parsers.deleteConfirm", { name: deleteTarget?.name })}
        loading={deleting}
        onConfirm={() => {
          if (deleteTarget) {
            deleteParser(deleteTarget.id).then(() => {
              setDeleteTarget(null)
              refetch()
            })
          }
        }}
      />
    </div>
  )
}

function buildRequest(form: FormState): Record<string, unknown> {
  const req: Record<string, unknown> = {
    name: form.name,
    priority: Number(form.priority) || 50,
    condition_regex: form.condition_regex,
    parse_regex: form.parse_regex,
    anime_title_source: form.anime_title_source,
    anime_title_value: form.anime_title_value,
    episode_no_source: form.episode_no_source,
    episode_no_value: form.episode_no_value,
  }
  if (form.series_no_source && form.series_no_source !== "none") {
    req.series_no_source = form.series_no_source
    req.series_no_value = form.series_no_value
  }
  if (form.subtitle_group_source && form.subtitle_group_source !== "none") {
    req.subtitle_group_source = form.subtitle_group_source
    req.subtitle_group_value = form.subtitle_group_value
  }
  if (form.resolution_source && form.resolution_source !== "none") {
    req.resolution_source = form.resolution_source
    req.resolution_value = form.resolution_value
  }
  if (form.season_source && form.season_source !== "none") {
    req.season_source = form.season_source
    req.season_value = form.season_value
  }
  if (form.year_source && form.year_source !== "none") {
    req.year_source = form.year_source
    req.year_value = form.year_value
  }
  return req
}

function SourceSelect({
  label,
  sourceKey,
  valueKey,
  form,
  onChange,
}: {
  label: string
  sourceKey: keyof FormState
  valueKey: keyof FormState
  form: FormState
  onChange: (key: keyof FormState, value: string) => void
}) {
  const { t } = useTranslation()
  return (
    <div className="grid grid-cols-3 gap-2 items-end">
      <div>
        <Label className="text-xs">{label} {t("parsers.source")}</Label>
        <Select value={form[sourceKey] || "none"} onValueChange={(v) => onChange(sourceKey, v)}>
          <SelectTrigger>
            <SelectValue placeholder={t("common.none")} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="none">{t("common.none")}</SelectItem>
            <SelectItem value="regex">{t("parsers.regex")}</SelectItem>
            <SelectItem value="static">{t("parsers.static")}</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div className="col-span-2">
        <Label className="text-xs">{label} {t("parsers.value")}</Label>
        <Input
          className="font-mono text-sm"
          value={form[valueKey]}
          onChange={(e) => onChange(valueKey, e.target.value)}
          disabled={!form[sourceKey] || form[sourceKey] === "none"}
          placeholder={form[sourceKey] === "regex" ? t("parsers.regexPlaceholder") : t("parsers.staticPlaceholder")}
        />
      </div>
    </div>
  )
}

function ParserForm({
  form,
  onChange,
}: {
  form: FormState
  onChange: (key: keyof FormState, value: string) => void
}) {
  const { t } = useTranslation()
  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-4">
        <div>
          <Label>{t("common.name")}</Label>
          <Input value={form.name} onChange={(e) => onChange("name", e.target.value)} />
        </div>
        <div>
          <Label>{t("parsers.priority")}</Label>
          <Input
            type="number"
            value={form.priority}
            onChange={(e) => onChange("priority", e.target.value)}
          />
        </div>
      </div>
      <div>
        <Label>{t("parsers.conditionRegex")}</Label>
        <RegexInput
          value={form.condition_regex}
          onChange={(v) => onChange("condition_regex", v)}
          placeholder={t("parsers.conditionRegexPlaceholder")}
        />
      </div>
      <div>
        <Label>{t("parsers.parseRegex")}</Label>
        <RegexInput
          value={form.parse_regex}
          onChange={(v) => onChange("parse_regex", v)}
          placeholder={t("parsers.parseRegexPlaceholder")}
        />
      </div>

      <div className="border-t pt-4 space-y-3">
        <p className="text-sm font-medium">{t("parsers.fieldExtraction")}</p>
        <SourceSelect
          label={t("parsers.animeTitle")}
          sourceKey="anime_title_source"
          valueKey="anime_title_value"
          form={form}
          onChange={onChange}
        />
        <SourceSelect
          label={t("parsers.episodeNo")}
          sourceKey="episode_no_source"
          valueKey="episode_no_value"
          form={form}
          onChange={onChange}
        />
        <SourceSelect
          label={t("parsers.seriesNo")}
          sourceKey="series_no_source"
          valueKey="series_no_value"
          form={form}
          onChange={onChange}
        />
        <SourceSelect
          label={t("parsers.subtitleGroup")}
          sourceKey="subtitle_group_source"
          valueKey="subtitle_group_value"
          form={form}
          onChange={onChange}
        />
        <SourceSelect
          label={t("parsers.resolution")}
          sourceKey="resolution_source"
          valueKey="resolution_value"
          form={form}
          onChange={onChange}
        />
        <SourceSelect
          label={t("parsers.season")}
          sourceKey="season_source"
          valueKey="season_value"
          form={form}
          onChange={onChange}
        />
        <SourceSelect
          label={t("parsers.year")}
          sourceKey="year_source"
          valueKey="year_value"
          form={form}
          onChange={onChange}
        />
      </div>
    </div>
  )
}

function PreviewResults({ preview }: { preview: ParserPreviewResponse }) {
  const { t } = useTranslation()

  if (!preview.condition_regex_valid || !preview.parse_regex_valid) {
    return (
      <Card className="border-destructive">
        <CardContent className="pt-4">
          <p className="text-sm text-destructive">{t("parsers.regexError")}: {preview.regex_error}</p>
        </CardContent>
      </Card>
    )
  }

  const newlyMatched = preview.results.filter((r) => r.is_newly_matched)
  const overridden = preview.results.filter((r) => r.is_override)
  const unmatched = preview.results.filter(
    (r) => !r.is_newly_matched && !r.is_override && !r.after_matched_by,
  )
  const existingMatch = preview.results.filter(
    (r) => !r.is_newly_matched && !r.is_override && !!r.after_matched_by,
  )

  return (
    <div className="space-y-4">
      <div className="flex gap-4 text-sm">
        <span className="text-green-600">{t("parsers.newlyMatched")}: {newlyMatched.length}</span>
        <span className="text-orange-600">{t("parsers.override")}: {overridden.length}</span>
        <span className="text-muted-foreground">{t("parsers.existing")}: {existingMatch.length}</span>
        <span className="text-muted-foreground">{t("parsers.unmatched")}: {unmatched.length}</span>
      </div>
      <ScrollArea className="h-80">
        <div className="space-y-1">
          {preview.results.map((r, i) => (
            <div
              key={i}
              className={cn(
                "text-xs px-2 py-1.5 rounded font-mono",
                r.is_newly_matched && "bg-green-50 text-green-800",
                r.is_override && "bg-orange-50 text-orange-800",
                !r.is_newly_matched && !r.is_override && r.after_matched_by && "bg-blue-50 text-blue-700",
                !r.after_matched_by && "bg-gray-50 text-gray-500",
              )}
            >
              <div className="flex justify-between items-start gap-2">
                <span className="truncate flex-1">{r.title}</span>
                <span className="text-[10px] shrink-0">
                  {r.after_matched_by ?? t("common.noMatch")}
                </span>
              </div>
              {r.parse_result && (
                <div className="mt-1 text-[10px] opacity-75 flex gap-3 flex-wrap">
                  <span>Title: {r.parse_result.anime_title}</span>
                  <span>EP: {r.parse_result.episode_no}</span>
                  {r.parse_result.subtitle_group && (
                    <span>Group: {r.parse_result.subtitle_group}</span>
                  )}
                  {r.parse_result.resolution && (
                    <span>Res: {r.parse_result.resolution}</span>
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      </ScrollArea>
    </div>
  )
}
