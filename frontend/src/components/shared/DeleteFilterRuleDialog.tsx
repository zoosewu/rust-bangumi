import { useState, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { ConfirmDialog } from "./ConfirmDialog"
import { FilterPreviewPanel } from "./FilterPreviewPanel"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import type { FilterPreviewResponse } from "@/schemas/filter"

interface DeleteFilterRuleDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 被刪除的規則資訊，用於顯示描述與計算預覽 */
  rule: { is_positive: boolean; regex_pattern: string; id?: number } | null
  targetType: string
  targetId: number | null
  onConfirm: () => Promise<void>
  loading?: boolean
}

export function DeleteFilterRuleDialog({
  open,
  onOpenChange,
  rule,
  targetType,
  targetId,
  onConfirm,
  loading,
}: DeleteFilterRuleDialogProps) {
  const { t } = useTranslation()
  const [preview, setPreview] = useState<FilterPreviewResponse | null>(null)

  // 當 dialog 開啟且有規則時，計算「刪除後」的預覽
  useEffect(() => {
    if (!open || !rule?.regex_pattern) {
      setPreview(null)
      return
    }
    AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) =>
        api.previewFilter({
          target_type: targetType,
          target_id: targetId,
          regex_pattern: rule.regex_pattern,
          is_positive: rule.is_positive,
          ...(rule.id != null ? { exclude_filter_id: rule.id } : {}),
        }),
      ),
    ).then(setPreview).catch(() => setPreview(null))
  }, [open, rule?.regex_pattern, rule?.is_positive, rule?.id, targetType, targetId])

  const handleOpenChange = (v: boolean) => {
    if (!v) setPreview(null)
    onOpenChange(v)
  }

  return (
    <ConfirmDialog
      open={open}
      onOpenChange={handleOpenChange}
      title={t("filter.confirmRemove", "Remove filter rule?")}
      size="full"
      description={
        rule
          ? `${rule.is_positive ? "Include" : "Exclude"}: ${rule.regex_pattern}`
          : ""
      }
      onConfirm={onConfirm}
      loading={loading}
      confirmLabel={t("common.delete", "Delete")}
      confirmLoadingLabel={t("common.deleting", "Deleting...")}
    >
      {preview?.regex_valid && (
        <FilterPreviewPanel
          before={preview.after}
          after={preview.before}
        />
      )}
    </ConfirmDialog>
  )
}
