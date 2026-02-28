import { useState, useRef, useEffect, useCallback } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { Label } from "@/components/ui/label"
import { Plus } from "lucide-react"
import { FilterPreviewPanel } from "./FilterPreviewPanel"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import type { FilterPreviewResponse } from "@/schemas/filter"

interface FilterAddFormProps {
  targetType: "global" | "anime_work" | "anime" | "subtitle_group" | "fetcher"
  targetId: number | null
  currentRuleCount: number
  onSuccess: () => void
  /** 若不傳則由元件自動載入 baseline */
  baseline?: FilterPreviewResponse | null
}

export function FilterAddForm({
  targetType,
  targetId,
  currentRuleCount,
  onSuccess,
  baseline: baselineProp,
}: FilterAddFormProps) {
  const { t } = useTranslation()
  const [newPattern, setNewPattern] = useState("")
  const [isPositive, setIsPositive] = useState(true)
  const [preview, setPreview] = useState<FilterPreviewResponse | null>(null)
  const [selfBaseline, setSelfBaseline] = useState<FilterPreviewResponse | null>(null)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // 若父層未傳 baseline，自行載入
  useEffect(() => {
    if (baselineProp !== undefined) return
    AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) =>
        api.previewFilter({
          target_type: targetType,
          target_id: targetId,
          regex_pattern: "^$",
          is_positive: false,
        }),
      ),
    ).then(setSelfBaseline).catch(() => setSelfBaseline(null))
  }, [targetType, targetId, baselineProp])

  const baseline = baselineProp !== undefined ? baselineProp : selfBaseline

  const { mutate: createRule, isLoading: creating } = useEffectMutation(
    (pattern: string, positive: boolean) =>
      Effect.flatMap(CoreApi, (api) =>
        api.createFilterRule({
          target_type: targetType,
          target_id: targetId ?? undefined,
          rule_order: currentRuleCount + 1,
          is_positive: positive,
          regex_pattern: pattern,
        }),
      ),
  )

  // Debounced preview
  useEffect(() => {
    if (!newPattern.trim()) {
      setPreview(null)
      return
    }

    if (debounceRef.current) clearTimeout(debounceRef.current)

    debounceRef.current = setTimeout(() => {
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) =>
          api.previewFilter({
            target_type: targetType,
            target_id: targetId,
            regex_pattern: newPattern,
            is_positive: isPositive,
          }),
        ),
      ).then(setPreview).catch(() => setPreview(null))
    }, 300)

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [newPattern, isPositive, targetType, targetId])

  const handleAdd = useCallback(async () => {
    if (!newPattern.trim()) return
    await createRule(newPattern, isPositive)
    setNewPattern("")
    setPreview(null)
    onSuccess()
  }, [newPattern, isPositive, createRule, onSuccess])

  const showBefore = baseline?.before ?? null
  const showAfter = preview?.regex_valid ? preview.after : null

  return (
    <div className="space-y-3">
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

      {preview && !preview.regex_valid && preview.regex_error && (
        <p className="text-sm text-destructive">{preview.regex_error}</p>
      )}

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

      {showBefore && (
        <FilterPreviewPanel before={showBefore} after={showAfter} />
      )}
    </div>
  )
}
