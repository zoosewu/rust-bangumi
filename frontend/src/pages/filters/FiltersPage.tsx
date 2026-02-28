import { useState, useCallback } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { Plus } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { FilterRuleList } from "@/components/shared/FilterRuleList"
import { FilterAddForm } from "@/components/shared/FilterAddForm"
import { FilterPreviewPanel } from "@/components/shared/FilterPreviewPanel"
import { ConfirmDialog } from "@/components/shared/ConfirmDialog"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import type { FilterRule, FilterPreviewResponse } from "@/schemas/filter"

export default function FiltersPage() {
  const { t } = useTranslation()
  const [addOpen, setAddOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<FilterRule | null>(null)
  const [deletePreview, setDeletePreview] = useState<FilterPreviewResponse | null>(null)

  const { data: rules, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getFilterRules("global", undefined)),
    [],
  )

  const { mutate: deleteRule, isLoading: deleting } = useEffectMutation(
    (ruleId: number) =>
      Effect.flatMap(CoreApi, (api) => api.deleteFilterRule(ruleId)),
  )

  const handleDeleteClick = useCallback((rule: FilterRule) => {
    setDeleteTarget(rule)
    AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) =>
        api.previewFilter({
          target_type: "global",
          target_id: null,
          regex_pattern: rule.regex_pattern,
          is_positive: rule.is_positive,
          exclude_filter_id: rule.rule_id,
        }),
      ),
    ).then(setDeletePreview).catch(() => setDeletePreview(null))
  }, [])

  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteTarget) return
    await deleteRule(deleteTarget.rule_id)
    setDeleteTarget(null)
    setDeletePreview(null)
    refetch()
  }, [deleteTarget, deleteRule, refetch])

  const handleAddSuccess = useCallback(() => {
    setAddOpen(false)
    refetch()
  }, [refetch])

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("filters.title")}</h1>
        <Button onClick={() => setAddOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          {t("filters.addFilter")}
        </Button>
      </div>

      <FilterRuleList rules={rules ?? []} onDeleteClick={handleDeleteClick} />

      {rules?.length === 0 && (
        <p className="text-sm text-muted-foreground">{t("filters.noRules", "目前沒有全域篩選規則。")}</p>
      )}

      {/* Add Dialog */}
      <Dialog open={addOpen} onOpenChange={setAddOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>{t("filters.addFilter")}</DialogTitle>
          </DialogHeader>
          <FilterAddForm
            targetType="global"
            targetId={null}
            currentRuleCount={rules?.length ?? 0}
            onSuccess={handleAddSuccess}
          />
        </DialogContent>
      </Dialog>

      {/* Delete Confirm */}
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
