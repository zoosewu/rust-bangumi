import { useState, useCallback, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { FilterPreviewPanel } from "./FilterPreviewPanel"
import { FilterRuleList } from "./FilterRuleList"
import { FilterAddForm } from "./FilterAddForm"
import { ConfirmDialog } from "./ConfirmDialog"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import type { FilterRule, FilterPreviewResponse } from "@/schemas/filter"

interface FilterRuleEditorProps {
  targetType: "global" | "anime_work" | "anime" | "subtitle_group" | "fetcher"
  targetId: number | null
  onRulesChange?: () => void
}

export function FilterRuleEditor({
  targetType,
  targetId,
  onRulesChange,
}: FilterRuleEditorProps) {
  const { t } = useTranslation()

  const [baseline, setBaseline] = useState<FilterPreviewResponse | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<FilterRule | null>(null)
  const [deletePreview, setDeletePreview] = useState<FilterPreviewResponse | null>(null)

  const { data: rules, refetch: refetchRules } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getFilterRules(targetType, targetId ?? undefined)),
    [targetType, targetId],
  )

  const loadBaseline = useCallback(() => {
    AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) =>
        api.previewFilter({
          target_type: targetType,
          target_id: targetId,
          regex_pattern: "^$",
          is_positive: false,
        }),
      ),
    ).then(setBaseline).catch(() => setBaseline(null))
  }, [targetType, targetId])

  useEffect(() => {
    loadBaseline()
  }, [loadBaseline])

  const { mutate: deleteRule, isLoading: deleting } = useEffectMutation(
    (ruleId: number) =>
      Effect.flatMap(CoreApi, (api) => api.deleteFilterRule(ruleId)),
  )

  const handleDeleteClick = useCallback(
    (rule: FilterRule) => {
      setDeleteTarget(rule)
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) =>
          api.previewFilter({
            target_type: targetType,
            target_id: targetId,
            regex_pattern: rule.regex_pattern,
            is_positive: rule.is_positive,
            exclude_filter_id: rule.rule_id,
          }),
        ),
      ).then(setDeletePreview).catch(() => setDeletePreview(null))
    },
    [targetType, targetId],
  )

  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteTarget) return
    await deleteRule(deleteTarget.rule_id)
    setDeleteTarget(null)
    setDeletePreview(null)
    refetchRules()
    loadBaseline()
    onRulesChange?.()
  }, [deleteTarget, deleteRule, refetchRules, loadBaseline, onRulesChange])

  const handleAddSuccess = useCallback(() => {
    refetchRules()
    loadBaseline()
    onRulesChange?.()
  }, [refetchRules, loadBaseline, onRulesChange])

  return (
    <div className="space-y-4">
      <FilterRuleList rules={rules ?? []} onDeleteClick={handleDeleteClick} />

      <div className="rounded-md border p-3">
        <FilterAddForm
          targetType={targetType}
          targetId={targetId}
          currentRuleCount={rules?.length ?? 0}
          onSuccess={handleAddSuccess}
          baseline={baseline}
        />
      </div>

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteTarget(null)
            setDeletePreview(null)
          }
        }}
        title={t("filter.confirmRemove", "Remove filter rule?")}
        description={
          deleteTarget
            ? `${deleteTarget.is_positive ? "Include" : "Exclude"}: ${deleteTarget.regex_pattern}`
            : ""
        }
        onConfirm={handleDeleteConfirm}
        loading={deleting}
      >
        {deletePreview?.regex_valid && (
          <FilterPreviewPanel
            before={deletePreview.after}
            after={deletePreview.before}
          />
        )}
      </ConfirmDialog>
    </div>
  )
}
