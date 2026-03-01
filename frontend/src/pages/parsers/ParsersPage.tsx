import { useState, useEffect, useCallback, useMemo } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { ConfirmDialog } from "@/components/shared/ConfirmDialog"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { PageHeader } from "@/components/shared/PageHeader"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"
import { Plus, Trash2 } from "lucide-react"
import type { ParserPreviewResponse, ReparseStats } from "@/schemas/parser"
import { toast } from "sonner"
import {
  type ParserFormState,
  EMPTY_PARSER_FORM,
  buildParserRequest,
  ParserFormFields,
} from "@/components/shared/ParserForm"
import { AppRuntime } from "@/runtime/AppRuntime"
import { SearchBar } from "@/components/shared/SearchBar"
import { useTableSearch } from "@/hooks/useTableSearch"
import { AnimeWorkDialog } from "@/pages/anime/AnimeDialog"
import { AnimeDialog } from "@/pages/anime-series/AnimeSeriesDialog"
import { SubtitleGroupDialog } from "@/pages/subtitle-groups/SubtitleGroupDialog"
import { SubscriptionDialog } from "@/pages/subscriptions/SubscriptionDialog"
import type { AnimeWork, AnimeRich } from "@/schemas/anime"
import type { Subscription } from "@/schemas/subscription"

type EntityDialog =
  | { type: "subtitle_group"; id: number; name: string }
  | { type: "anime_work"; data: AnimeWork }
  | { type: "anime"; data: AnimeRich }
  | { type: "subscription"; data: Subscription }

export default function ParsersPage() {
  const { t } = useTranslation()
  const [dialogOpen, setDialogOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")
  const [editTarget, setEditTarget] = useState<Record<string, unknown> | null>(null) // null = create mode
  const [form, setForm] = useState<ParserFormState>({ ...EMPTY_PARSER_FORM })
  const [deleteTarget, setDeleteTarget] = useState<{ id: number; name: string } | null>(null)

  const [entityDialog, setEntityDialog] = useState<EntityDialog | null>(null)
  const [preview, setPreview] = useState<ParserPreviewResponse | null>(null)
  const [previewDebounce, setPreviewDebounce] = useState<ReturnType<typeof setTimeout> | null>(null)

  const { data: rawParsers, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getParsers()
      }),
    [],
  )

  // Sort: global (no created_from_type) first, then by created_from_type
  const parsers = useMemo(() => {
    if (!rawParsers) return rawParsers
    return [...rawParsers].sort((a, b) => {
      const aIsGlobal = !a.created_from_type || a.created_from_type === "global"
      const bIsGlobal = !b.created_from_type || b.created_from_type === "global"
      if (aIsGlobal && !bIsGlobal) return -1
      if (!aIsGlobal && bIsGlobal) return 1
      return 0
    })
  }, [rawParsers])

  const filteredParsers = useTableSearch(parsers ?? [], searchQuery)

  const { mutate: createParser, isLoading: creating } = useEffectMutation(
    (req: Record<string, unknown>) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createParser(req)
      }),
  )

  const { mutate: updateParser, isLoading: updating } = useEffectMutation(
    (req: { id: number; data: Record<string, unknown> }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.updateParser(req.id, req.data)
      }),
  )

  const { mutate: deleteParser, isLoading: deleting } = useEffectMutation(
    (id: number) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.deleteParser(id)
      }),
  )

  // Debounced preview
  useEffect(() => {
    if (!dialogOpen) return
    if (!form.condition_regex || !form.parse_regex) {
      setPreview(null)
      return
    }
    if (previewDebounce) clearTimeout(previewDebounce)
    const timer = setTimeout(() => {
      const req: Record<string, unknown> = {
        ...buildParserRequest(form),
        target_type: "global",
        target_id: null,
      }
      if (editTarget) {
        req.exclude_parser_id = editTarget.parser_id
      }
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.previewParser(req)),
      ).then(setPreview).catch(() => setPreview(null))
    }, 600)
    setPreviewDebounce(timer)
    return () => clearTimeout(timer)
  }, [form, dialogOpen])

  const updateForm = (key: string, value: string | number | null) =>
    setForm((prev) => ({ ...prev, [key]: value }))

  const handleImport = useCallback((imported: ParserFormState) => {
    setForm(imported)
  }, [])

  const parserToPreviewForm = (p: Record<string, unknown>): ParserFormState => ({
    name: String(p.name ?? ""),
    priority: Number(p.priority) || 50,
    condition_regex: String(p.condition_regex ?? ""),
    parse_regex: String(p.parse_regex ?? ""),
    anime_title_source: String(p.anime_title_source ?? "regex"),
    anime_title_value: String(p.anime_title_value ?? ""),
    episode_no_source: String(p.episode_no_source ?? "regex"),
    episode_no_value: String(p.episode_no_value ?? ""),
    series_no_source: p.series_no_source ? String(p.series_no_source) : null,
    series_no_value: p.series_no_value ? String(p.series_no_value) : null,
    subtitle_group_source: p.subtitle_group_source ? String(p.subtitle_group_source) : null,
    subtitle_group_value: p.subtitle_group_value ? String(p.subtitle_group_value) : null,
    resolution_source: p.resolution_source ? String(p.resolution_source) : null,
    resolution_value: p.resolution_value ? String(p.resolution_value) : null,
    season_source: p.season_source ? String(p.season_source) : null,
    season_value: p.season_value ? String(p.season_value) : null,
    year_source: p.year_source ? String(p.year_source) : null,
    year_value: p.year_value ? String(p.year_value) : null,
  })

  const showReparseToast = useCallback((stats: ReparseStats) => {
    if (stats.total === 0) return
    toast.success(
      `Reparse: ${stats.parsed} parsed, ${stats.no_match} no match, ${stats.failed} failed (${stats.total} total)`,
    )
  }, [])

  const handleSave = useCallback(async () => {
    let result
    if (editTarget) {
      result = await updateParser({
        id: editTarget.parser_id as number,
        data: { ...buildParserRequest(form), created_from_type: String(editTarget.created_from_type ?? "global") },
      })
    } else {
      result = await createParser({ ...buildParserRequest(form), created_from_type: "global" })
    }
    setForm({ ...EMPTY_PARSER_FORM })
    setDialogOpen(false)
    setEditTarget(null)
    setPreview(null)
    refetch()
    if (result.reparse) showReparseToast(result.reparse)
  }, [editTarget, form, updateParser, createParser, refetch, showReparseToast])

  const handleEntityClick = useCallback(async (row: Record<string, unknown>) => {
    const type = row.created_from_type as string | null
    const id = row.created_from_id as number | null
    const name = row.created_from_name as string | null
    if (!type || !id) return

    if (type === "subtitle_group") {
      setEntityDialog({ type: "subtitle_group", id, name: name ?? `#${id}` })
    } else if (type === "anime_work") {
      const animes = await AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.getAnimeWorks),
      ).catch(() => { toast.error(t("common.loadFailed", "Load failed")); return null })
      const anime = animes?.find((a: AnimeWork) => a.anime_id === id)
      if (anime) setEntityDialog({ type: "anime_work", data: anime })
      else if (animes) toast.error(t("common.notFound", "Not found"))
    } else if (type === "anime") {
      const allSeries = await AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.getAllAnime({ excludeEmpty: true })),
      ).catch(() => { toast.error(t("common.loadFailed", "Load failed")); return null })
      const series = allSeries?.find((s: AnimeRich) => s.series_id === id)
      if (series) setEntityDialog({ type: "anime", data: series })
      else if (allSeries) toast.error(t("common.notFound", "Not found"))
    } else if (type === "subscription" || type === "fetcher") {
      const subs = await AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.getSubscriptions),
      ).catch(() => { toast.error(t("common.loadFailed", "Load failed")); return null })
      const sub = subs?.find((s: Subscription) => s.subscription_id === id)
      if (sub) setEntityDialog({ type: "subscription", data: sub })
      else if (subs) toast.error(t("common.notFound", "Not found"))
    }
  }, [t])

  const columns: Column<Record<string, unknown>>[] = [
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
      key: "created_from_name",
      header: t("parsers.entity", "Belongs To"),
      render: (item) => {
        const type = item.created_from_type as string | null
        const name = item.created_from_name as string | null
        const id = item.created_from_id as number | null
        if (!type || type === "global") {
          return <span className="text-muted-foreground text-xs">Global</span>
        }
        return (
          <button
            type="button"
            className="text-xs underline hover:opacity-70 text-left"
            onClick={(e) => {
              e.stopPropagation()
              handleEntityClick(item)
            }}
          >
            {name ?? `#${id}`}
          </button>
        )
      },
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
      <PageHeader
        title={t("parsers.title")}
        actions={
          <Button onClick={() => {
            setEditTarget(null)
            setForm({ ...EMPTY_PARSER_FORM })
            setPreview(null)
            setDialogOpen(true)
          }}>
            <Plus className="h-4 w-4 mr-2" />
            {t("parsers.addParser")}
          </Button>
        }
      />

      <SearchBar value={searchQuery} onChange={setSearchQuery} />

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <DataTable
          columns={columns}
          data={(filteredParsers ?? []) as unknown as Record<string, unknown>[]}
          keyField="parser_id"
          onRowClick={(item) => {
            setEditTarget(item)
            setForm(parserToPreviewForm(item))
            setPreview(null)
            setDialogOpen(true)
          }}
        />
      )}

      {/* Create/Edit FullScreenDialog */}
      <FullScreenDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        title={editTarget ? t("parser.editParser") : t("parsers.addParser")}
      >
        <div className="space-y-4">
          <ParserFormFields
            form={form}
            onChange={updateForm}
            onImport={handleImport}
            targetType="global"
            targetId={null}
          />

          {/* Save/Create button */}
          <Button
            onClick={handleSave}
            disabled={(creating || updating) || !form.name || !form.condition_regex || !form.parse_regex}
          >
            {editTarget
              ? (updating ? t("parser.saving") : t("parser.save"))
              : (creating ? t("common.creating") : t("common.create"))}
          </Button>

          {/* Preview results */}
          {preview && <PreviewResults preview={preview} />}
        </div>
      </FullScreenDialog>

      {/* Entity dialogs */}
      {entityDialog?.type === "subtitle_group" && (
        <SubtitleGroupDialog
          groupId={entityDialog.id}
          groupName={entityDialog.name}
          open={!!entityDialog}
          onOpenChange={(open) => { if (!open) setEntityDialog(null) }}
        />
      )}
      {entityDialog?.type === "anime_work" && (
        <AnimeWorkDialog
          anime={entityDialog.data}
          open={!!entityDialog}
          onOpenChange={(open) => { if (!open) setEntityDialog(null) }}
        />
      )}
      {entityDialog?.type === "anime" && (
        <AnimeDialog
          series={entityDialog.data}
          open={!!entityDialog}
          onOpenChange={(open) => { if (!open) setEntityDialog(null) }}
        />
      )}
      {entityDialog?.type === "subscription" && (
        <SubscriptionDialog
          subscription={entityDialog.data}
          open={!!entityDialog}
          onOpenChange={(open) => { if (!open) setEntityDialog(null) }}
        />
      )}

      {/* Delete Confirm */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title={t("parsers.deleteParser")}
        description={t("parsers.deleteConfirm", { name: deleteTarget?.name })}
        loading={deleting}
        onConfirm={() => {
          if (deleteTarget) {
            deleteParser(deleteTarget.id).then((result) => {
              setDeleteTarget(null)
              refetch()
              if (result.reparse.total > 0) showReparseToast(result.reparse)
            })
          }
        }}
      />
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
              {r.parse_error && (
                <div className="mt-1 text-[10px] text-destructive">
                  {t("parsers.parseError")}: {r.parse_error}
                </div>
              )}
            </div>
          ))}
        </div>
      </ScrollArea>
    </div>
  )
}
