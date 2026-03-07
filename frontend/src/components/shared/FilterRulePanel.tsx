import { useState, useEffect, useCallback } from "react"
import { Effect } from "effect"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import { Trash2, ChevronDown, ChevronRight } from "lucide-react"
import { FilterPreviewPanel } from "./FilterPreviewPanel"
import { FilterAddForm } from "./FilterAddForm"
import { DeleteFilterRuleDialog } from "./DeleteFilterRuleDialog"
import { RegexInput } from "./RegexInput"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import type { FilterPreviewResponse } from "@/schemas/filter"

// --- Types ---

export interface FilterRuleDraft {
  id?: number
  is_positive: boolean
  regex_pattern: string
  reasoning?: string
}

interface FilterRulePanelProps {
  rules: FilterRuleDraft[]
  targetType: string
  targetId: number | null
  /**
   * 新增規則的回調。
   * - 持久化模式：FilterAddForm 自行呼叫 API，此函數在成功後被呼叫（用於 refetch）。
   * - Draft 模式：需傳入 `addRuleOverride`，此函數不再被呼叫。
   */
  onAddSuccess: () => void
  /**
   * 若提供，跳過 FilterAddForm 內部的 API 呼叫，改由此 callback 處理新規則（draft 模式）。
   */
  addRuleOverride?: (rule: { is_positive: boolean; regex_pattern: string }) => void | Promise<void>
  onDelete: (idx: number) => void | Promise<void>
  /** 提供時規則可行內編輯（draft 模式），不提供則顯示唯讀 badge（持久化模式） */
  onUpdate?: (idx: number, changes: { is_positive?: boolean; regex_pattern?: string }) => void
  requireDeleteConfirm?: boolean
}

// --- FilterRuleRow ---

function FilterRuleRow({
  rule,
  idx,
  selected,
  onSelect,
  onUpdate,
  requireDeleteConfirm,
  onDeleteRequest,
}: {
  rule: FilterRuleDraft
  idx: number
  selected: boolean
  onSelect: () => void
  onUpdate?: (idx: number, changes: { is_positive?: boolean; regex_pattern?: string }) => void
  requireDeleteConfirm: boolean
  onDeleteRequest: (idx: number) => void
}) {
  const handleDeleteClick = (e: React.MouseEvent) => {
    e.stopPropagation()
    onDeleteRequest(idx)
  }

  return (
    <div
      className={`rounded-md border text-xs transition-colors ${selected ? "border-primary bg-muted/30" : ""}`}
    >
      <div
        className="flex items-center gap-2 px-3 py-2 cursor-pointer hover:bg-muted/50"
        onClick={onSelect}
      >
        {/* Expand indicator */}
        <span className="shrink-0 text-muted-foreground">
          {selected
            ? <ChevronDown className="size-3.5" />
            : <ChevronRight className="size-3.5" />}
        </span>

        {/* include/exclude toggle or badge */}
        {onUpdate ? (
          <div className="flex items-center gap-1.5 shrink-0" onClick={(e) => e.stopPropagation()}>
            <Switch
              checked={rule.is_positive}
              onCheckedChange={(v) => onUpdate(idx, { is_positive: v })}
              className="scale-75"
            />
            <span className={`text-[10px] font-medium w-12 ${rule.is_positive ? "text-emerald-600" : "text-destructive"}`}>
              {rule.is_positive ? "Include" : "Exclude"}
            </span>
          </div>
        ) : (
          <Badge
            variant={rule.is_positive ? "default" : "destructive"}
            className="text-[10px] px-1.5 shrink-0"
          >
            {rule.is_positive ? "include" : "exclude"}
          </Badge>
        )}

        {/* Regex pattern (editable or read-only) */}
        {onUpdate ? (
          <div className="flex-1" onClick={(e) => e.stopPropagation()}>
            <RegexInput
              value={rule.regex_pattern}
              onChange={(v) => onUpdate(idx, { regex_pattern: v })}
              className="h-7 text-xs px-2"
            />
          </div>
        ) : (
          <code className="flex-1 font-mono text-xs truncate">{rule.regex_pattern}</code>
        )}

        {/* Delete */}
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 shrink-0"
          onClick={handleDeleteClick}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>

      {/* AI reasoning */}
      {rule.reasoning && (
        <p className="px-3 pb-2 text-muted-foreground leading-relaxed">{rule.reasoning}</p>
      )}
    </div>
  )
}

// --- FilterRulePanel ---

export function FilterRulePanel({
  rules,
  targetType,
  targetId,
  onAddSuccess,
  addRuleOverride,
  onDelete,
  onUpdate,
  requireDeleteConfirm = false,
}: FilterRulePanelProps) {
  const [selectedIdx, setSelectedIdx] = useState<number | null>(null)
  const [rulePreview, setRulePreview] = useState<FilterPreviewResponse | null>(null)
  const [baseline, setBaseline] = useState<FilterPreviewResponse | null>(null)
  const [deleteConfirmIdx, setDeleteConfirmIdx] = useState<number | null>(null)
  const [deleting, setDeleting] = useState(false)

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

  useEffect(() => { loadBaseline() }, [loadBaseline])

  const selectedRule = selectedIdx !== null ? (rules[selectedIdx] ?? null) : null

  // Preview selected rule
  useEffect(() => {
    if (!selectedRule?.regex_pattern) {
      setRulePreview(null)
      return
    }
    AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) =>
        api.previewFilter({
          target_type: targetType,
          target_id: targetId,
          regex_pattern: selectedRule.regex_pattern,
          is_positive: selectedRule.is_positive,
        }),
      ),
    ).then(setRulePreview).catch(() => setRulePreview(null))
  }, [selectedRule?.regex_pattern, selectedRule?.is_positive, targetType, targetId])

  const handleDeleteRequest = (idx: number) => {
    if (requireDeleteConfirm) {
      setDeleteConfirmIdx(idx)
    } else {
      onDelete(idx)
      if (selectedIdx === idx) setSelectedIdx(null)
    }
  }

  const handleDeleteConfirm = async () => {
    if (deleteConfirmIdx === null) return
    setDeleting(true)
    try {
      await onDelete(deleteConfirmIdx)
      if (selectedIdx === deleteConfirmIdx) setSelectedIdx(null)
      setDeleteConfirmIdx(null)
      loadBaseline()
    } finally {
      setDeleting(false)
    }
  }

  const deleteTarget = deleteConfirmIdx !== null ? (rules[deleteConfirmIdx] ?? null) : null

  return (
    <div className="space-y-4">
      {/* Rule list */}
      {rules.length > 0 && (
        <div className="space-y-2">
          {rules.map((rule, idx) => (
            <FilterRuleRow
              key={idx}
              rule={rule}
              idx={idx}
              selected={selectedIdx === idx}
              onSelect={() => setSelectedIdx(selectedIdx === idx ? null : idx)}
              onUpdate={onUpdate}
              requireDeleteConfirm={requireDeleteConfirm}
              onDeleteRequest={handleDeleteRequest}
            />
          ))}
        </div>
      )}

      {/* Preview for selected rule */}
      {selectedRule && baseline && (
        <FilterPreviewPanel
          before={baseline.before}
          after={rulePreview?.regex_valid ? rulePreview.after : null}
        />
      )}

      {/* Add rule form */}
      <div className="rounded-md border p-3">
        <FilterAddForm
          targetType={targetType as "global" | "anime_work" | "anime" | "subtitle_group" | "fetcher"}
          targetId={targetId}
          currentRuleCount={rules.length}
          baseline={baseline}
          onAddRule={addRuleOverride}
          onSuccess={() => {
            loadBaseline()
            onAddSuccess()
          }}
        />
      </div>

      {/* Delete confirm dialog (persistent mode) */}
      {requireDeleteConfirm && (
        <DeleteFilterRuleDialog
          open={deleteConfirmIdx !== null}
          onOpenChange={(open) => { if (!open) setDeleteConfirmIdx(null) }}
          rule={deleteTarget}
          targetType={targetType}
          targetId={targetId}
          onConfirm={handleDeleteConfirm}
          loading={deleting}
        />
      )}
    </div>
  )
}
