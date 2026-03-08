import { useCallback } from "react"
import { Effect } from "effect"
import { FilterRulePanel, type FilterRuleDraft } from "./FilterRulePanel"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { CoreApi } from "@/services/CoreApi"
import type { FilterRule } from "@/schemas/filter"

interface FilterRuleEditorProps {
  targetType: "global" | "anime_work" | "anime" | "subtitle_group" | "fetcher"
  targetId: number | null
  onRulesChange?: () => void
  readOnly?: boolean
}

export function FilterRuleEditor({
  targetType,
  targetId,
  onRulesChange,
  readOnly,
}: FilterRuleEditorProps) {
  const { data: rules, refetch: refetchRules } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getFilterRules(targetType, targetId ?? undefined)),
    [targetType, targetId],
  )

  const { mutate: deleteRule } = useEffectMutation(
    (ruleId: number) => Effect.flatMap(CoreApi, (api) => api.deleteFilterRule(ruleId)),
  )

  const drafts: FilterRuleDraft[] = (rules ?? []).map((r: FilterRule) => ({
    id: r.rule_id,
    is_positive: r.is_positive,
    regex_pattern: r.regex_pattern,
  }))

  const handleDelete = useCallback(async (idx: number) => {
    const rule = drafts[idx]
    if (!rule?.id) return
    await deleteRule(rule.id)
    refetchRules()
    onRulesChange?.()
  }, [drafts, deleteRule, refetchRules, onRulesChange])

  const handleAddSuccess = useCallback(() => {
    refetchRules()
    onRulesChange?.()
  }, [refetchRules, onRulesChange])

  return (
    <FilterRulePanel
      rules={drafts}
      targetType={targetType}
      targetId={targetId}
      onAddSuccess={handleAddSuccess}
      onDelete={handleDelete}
      requireDeleteConfirm
      readOnly={readOnly}
    />
  )
}
