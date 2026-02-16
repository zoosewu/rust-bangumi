import { useState, useCallback, useRef, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import { Trash2, Plus } from "lucide-react"
import { FilterPreviewPanel } from "./FilterPreviewPanel"
import { ConfirmDialog } from "./ConfirmDialog"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import type { FilterRule, FilterPreviewResponse } from "@/schemas/filter"

interface FilterRuleEditorProps {
  targetType: "global" | "anime" | "anime_series" | "subtitle_group" | "fetcher"
  targetId: number | null
  onRulesChange?: () => void
}

export function FilterRuleEditor({
  targetType,
  targetId,
  onRulesChange,
}: FilterRuleEditorProps) {
  const { t } = useTranslation()

  // State
  const [newPattern, setNewPattern] = useState("")
  const [isPositive, setIsPositive] = useState(true)
  const [baseline, setBaseline] = useState<FilterPreviewResponse | null>(null)
  const [preview, setPreview] = useState<FilterPreviewResponse | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<FilterRule | null>(null)
  const [deletePreview, setDeletePreview] = useState<FilterPreviewResponse | null>(null)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Load current rules
  const {
    data: rules,
    refetch: refetchRules,
  } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getFilterRules(targetType, targetId ?? undefined)),
    [targetType, targetId],
  )

  // Load baseline (current filter state without any new rule)
  const loadBaseline = useCallback(() => {
    const req = {
      target_type: targetType,
      target_id: targetId,
      regex_pattern: "^$",
      is_positive: false,
    }
    AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) => api.previewFilter(req))
    ).then(setBaseline).catch(() => setBaseline(null))
  }, [targetType, targetId])

  useEffect(() => {
    loadBaseline()
  }, [loadBaseline])

  // Mutations
  const { mutate: createRule, isLoading: creating } = useEffectMutation(
    (pattern: string, positive: boolean) =>
      Effect.flatMap(CoreApi, (api) =>
        api.createFilterRule({
          target_type: targetType,
          target_id: targetId ?? undefined,
          rule_order: (rules?.length ?? 0) + 1,
          is_positive: positive,
          regex_pattern: pattern,
        }),
      ),
  )

  const { mutate: deleteRule, isLoading: deleting } = useEffectMutation(
    (ruleId: number) =>
      Effect.flatMap(CoreApi, (api) => api.deleteFilterRule(ruleId)),
  )

  // Debounced preview for new rule input
  useEffect(() => {
    if (!newPattern.trim()) {
      setPreview(null)
      return
    }

    if (debounceRef.current) clearTimeout(debounceRef.current)

    debounceRef.current = setTimeout(() => {
      const req = {
        target_type: targetType,
        target_id: targetId,
        regex_pattern: newPattern,
        is_positive: isPositive,
      }
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.previewFilter(req))
      ).then(setPreview).catch(() => setPreview(null))
    }, 300)

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [newPattern, isPositive, targetType, targetId])

  // Handle add rule
  const handleAdd = useCallback(async () => {
    if (!newPattern.trim()) return
    await createRule(newPattern, isPositive)
    setNewPattern("")
    setPreview(null)
    refetchRules()
    loadBaseline()
    onRulesChange?.()
  }, [newPattern, isPositive, createRule, refetchRules, loadBaseline, onRulesChange])

  // Handle delete with preview
  const handleDeleteClick = useCallback(
    (rule: FilterRule) => {
      setDeleteTarget(rule)
      const req = {
        target_type: targetType,
        target_id: targetId,
        regex_pattern: rule.regex_pattern,
        is_positive: rule.is_positive,
        exclude_filter_id: rule.rule_id,
      }
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.previewFilter(req))
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

  // Determine what to show in preview
  const showBefore = baseline?.before ?? null
  const showAfter = preview?.regex_valid ? preview.after : null

  return (
    <div className="space-y-4">
      {/* Current rules */}
      {rules && rules.length > 0 && (
        <div className="space-y-2">
          {rules.map((rule) => (
            <div
              key={rule.rule_id}
              className="flex items-center gap-2 rounded-md border px-3 py-2 text-sm"
            >
              <Badge variant={rule.is_positive ? "default" : "destructive"}>
                {rule.is_positive ? "include" : "exclude"}
              </Badge>
              <code className="flex-1 font-mono text-xs">{rule.regex_pattern}</code>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={() => handleDeleteClick(rule)}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          ))}
        </div>
      )}

      {/* Add new rule */}
      <div className="space-y-3 rounded-md border p-3">
        <div className="flex items-center gap-3">
          <Input
            placeholder={t("filter.regexPlaceholder", "Enter regex pattern...")}
            value={newPattern}
            onChange={(e) => setNewPattern(e.target.value)}
            className="flex-1 font-mono text-sm"
          />
          <div className="flex items-center gap-2">
            <Label className="text-xs whitespace-nowrap">
              {isPositive ? "Include" : "Exclude"}
            </Label>
            <Switch checked={isPositive} onCheckedChange={setIsPositive} />
          </div>
          <Button
            size="sm"
            onClick={handleAdd}
            disabled={!newPattern.trim() || creating}
          >
            <Plus className="h-4 w-4 mr-1" />
            {t("filter.addRule", "Add")}
          </Button>
        </div>

        {/* Regex error */}
        {preview && !preview.regex_valid && preview.regex_error && (
          <p className="text-sm text-destructive">{preview.regex_error}</p>
        )}

        {/* Stats summary when preview is active */}
        {preview?.regex_valid && showBefore && (
          <div className="flex gap-4 text-xs text-muted-foreground">
            <span>
              {t("filter.passed", "Passed")}: {showBefore.passed_items.length} → {preview.after.passed_items.length}
            </span>
            <span>
              {t("filter.filtered", "Filtered")}: {showBefore.filtered_items.length} → {preview.after.filtered_items.length}
            </span>
          </div>
        )}

        {/* Preview panel: baseline (left) + diff (right) */}
        {showBefore && (
          <FilterPreviewPanel
            before={showBefore}
            after={showAfter}
          />
        )}
      </div>

      {/* Delete confirmation dialog */}
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
