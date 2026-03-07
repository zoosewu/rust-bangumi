import { useState, useCallback, useRef, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Trash2, Plus, ChevronUp, Pencil } from "lucide-react"
import { ConfirmDialog } from "./ConfirmDialog"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import type { TitleParser, ParserPreviewResponse, ReparseStats } from "@/schemas/parser"
import { toast } from "sonner"
import {
  type ParserFormState,
  EMPTY_PARSER_FORM,
  buildParserRequest,
  ParserFormFields,
  ParserPreviewSection,
} from "./ParserForm"

interface ParserEditorProps {
  createdFromType: "global" | "anime_work" | "anime" | "subtitle_group" | "subscription"
  createdFromId: number | null
  onParsersChange?: () => void
}

export function ParserEditor({
  createdFromType,
  createdFromId,
  onParsersChange,
}: ParserEditorProps) {
  const { t } = useTranslation()
  const [showForm, setShowForm] = useState(false)
  const [editTarget, setEditTarget] = useState<TitleParser | null>(null)
  const [form, setForm] = useState<ParserFormState>(EMPTY_PARSER_FORM)
  const [preview, setPreview] = useState<ParserPreviewResponse | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<TitleParser | null>(null)
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
          ...buildParserRequest(form),
          is_enabled: true,
          created_from_type: createdFromType,
          created_from_id: createdFromId,
        }),
      ),
  )

  const { mutate: updateParser, isLoading: updating } = useEffectMutation(
    (req: { id: number; data: Record<string, unknown> }) =>
      Effect.flatMap(CoreApi, (api) => api.updateParser(req.id, req.data)),
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
      const req: Record<string, unknown> = {
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
        episode_end_source: form.episode_end_source,
        episode_end_value: form.episode_end_value,
        year_source: form.year_source,
        year_value: form.year_value,
      }
      if (editTarget) {
        req.exclude_parser_id = editTarget.parser_id
      }
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.previewParser(req)),
      ).then(setPreview).catch((e) => {
        console.error("Parser preview failed:", e)
        setPreview(null)
      })
    }, 300)

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [form, createdFromType, createdFromId, editTarget])

  const showReparseToast = useCallback((stats: ReparseStats) => {
    if (stats.total === 0) return
    toast.success(
      `Reparse: ${stats.parsed} parsed, ${stats.no_match} no match, ${stats.failed} failed (${stats.total} total)`,
    )
  }, [])

  const handleSave = useCallback(async () => {
    let stats: ReparseStats | undefined
    if (editTarget) {
      const result = await updateParser({ id: editTarget.parser_id, data: buildParserRequest(form) })
      stats = result.reparse
    } else {
      const result = await createParser()
      stats = result.reparse
    }
    setForm(EMPTY_PARSER_FORM)
    setShowForm(false)
    setEditTarget(null)
    setPreview(null)
    refetch()
    onParsersChange?.()
    if (stats) showReparseToast(stats)
  }, [editTarget, form, updateParser, createParser, refetch, onParsersChange, showReparseToast])

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
      episode_end_source: parser.episode_end_source ?? null,
      episode_end_value: parser.episode_end_value ?? null,
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

  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteTarget) return
    const result = await deleteParser(deleteTarget.parser_id)
    setDeleteTarget(null)
    refetch()
    onParsersChange?.()
    if (result.reparse.total > 0) showReparseToast(result.reparse)
  }, [deleteTarget, deleteParser, refetch, onParsersChange, showReparseToast])

  const updateForm = (key: string, value: string | number | null) =>
    setForm((prev) => ({ ...prev, [key]: value }))

  const handleImport = useCallback((imported: ParserFormState) => {
    setForm(imported)
    setShowForm(true)
  }, [])

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
                onClick={() => handleEdit(parser)}
              >
                <Pencil className="h-4 w-4" />
              </Button>
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
      <div className="flex gap-2">
        <Button
          variant="outline"
          size="sm"
          onClick={() => {
            if (showForm) {
              setShowForm(false)
              setEditTarget(null)
              setForm(EMPTY_PARSER_FORM)
              setPreview(null)
            } else {
              setEditTarget(null)
              setForm(EMPTY_PARSER_FORM)
              setPreview(null)
              setShowForm(true)
            }
          }}
        >
          {showForm ? (
            <ChevronUp className="h-4 w-4 mr-1" />
          ) : (
            <Plus className="h-4 w-4 mr-1" />
          )}
          {t("parser.addParser", "Add Parser")}
        </Button>
      </div>

      {/* Add/Edit form */}
      {showForm && (
        <div className="space-y-3 rounded-md border p-4">
          <ParserFormFields
            form={form}
            onChange={updateForm}
            onImport={handleImport}
            targetType={createdFromType}
            targetId={createdFromId}
          />

          {/* Save/Create button */}
          <Button
            size="sm"
            onClick={handleSave}
            disabled={(creating || updating) || !form.name || !form.condition_regex || !form.parse_regex}
          >
            {editTarget
              ? (updating ? t("parser.saving") : t("parser.save"))
              : (creating ? t("common.creating") : t("parser.create"))}
          </Button>

          {/* Live preview results */}
          <ParserPreviewSection preview={preview} />
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
        confirmLabel={t("common.delete", "Delete")}
        confirmLoadingLabel={t("common.deleting", "Deleting...")}
      />
    </div>
  )
}
