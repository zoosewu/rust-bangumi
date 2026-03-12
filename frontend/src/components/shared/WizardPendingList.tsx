import { useState, useEffect, useRef, useCallback } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { AiResultPanel } from "@/components/shared/AiResultPanel"
import { FilterRulePanel, type FilterRuleDraft } from "./FilterRulePanel"
import { Badge } from "@/components/ui/badge"
import { ChevronDown, ChevronRight } from "lucide-react"
import {
  type ParserFormState,
  EMPTY_PARSER_FORM,
  buildParserRequest,
  ParserFormFields,
  ParserPreviewSection,
} from "./ParserForm"
import type { PendingAiResult } from "@/schemas/ai"
import type { ParserPreviewResponse } from "@/schemas/parser"

// --- Helpers ---

function toParserForm(data: Record<string, unknown> | null): ParserFormState {
  if (!data) return EMPTY_PARSER_FORM
  return {
    name: (data.name as string) ?? "",
    condition_regex: (data.condition_regex as string) ?? "",
    parse_regex: (data.parse_regex as string) ?? "",
    priority: (data.priority as number) ?? 50,
    anime_title_source: (data.anime_title_source as string) ?? "regex",
    anime_title_value: (data.anime_title_value as string) ?? "",
    episode_no_source: (data.episode_no_source as string) ?? "regex",
    episode_no_value: (data.episode_no_value as string) ?? "",
    episode_end_source: (data.episode_end_source as string | null) ?? null,
    episode_end_value: (data.episode_end_value as string | null) ?? null,
    series_no_source: (data.series_no_source as string | null) ?? null,
    series_no_value: (data.series_no_value as string | null) ?? null,
    subtitle_group_source: (data.subtitle_group_source as string | null) ?? null,
    subtitle_group_value: (data.subtitle_group_value as string | null) ?? null,
    resolution_source: (data.resolution_source as string | null) ?? null,
    resolution_value: (data.resolution_value as string | null) ?? null,
    season_source: (data.season_source as string | null) ?? null,
    season_value: (data.season_value as string | null) ?? null,
    year_source: (data.year_source as string | null) ?? null,
    year_value: (data.year_value as string | null) ?? null,
  }
}

interface GeneratedFilterRule {
  rule_order?: number
  is_positive: boolean
  regex_pattern: string
  reasoning?: string
}

function parseFilterRules(data: Record<string, unknown> | null): GeneratedFilterRule[] {
  if (!data) return []
  const rules = data.rules
  if (!Array.isArray(rules)) return []
  return rules as GeneratedFilterRule[]
}

// --- StatusBadge (local) ---

const STATUS_CONFIG: Record<string, { label: string; variant: "secondary" | "default" | "outline" | "destructive" }> = {
  generating: { label: "生成中", variant: "secondary" },
  pending: { label: "待確認", variant: "default" },
  confirmed: { label: "已確認", variant: "outline" },
  failed: { label: "失敗", variant: "destructive" },
}

// --- RowHeader ---

function RowHeader({
  expanded,
  result,
  onToggle,
  selected,
  onToggleSelect,
}: {
  expanded: boolean
  result: PendingAiResult
  onToggle: () => void
  selected?: boolean
  onToggleSelect?: () => void
}) {
  const statusCfg = STATUS_CONFIG[result.status] ?? { label: result.status, variant: "secondary" as const }
  return (
    <button
      type="button"
      className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-muted/50 transition-colors"
      onClick={onToggle}
    >
      {onToggleSelect && (
        <input
          type="checkbox"
          checked={selected ?? false}
          onChange={onToggleSelect}
          onClick={(e) => e.stopPropagation()}
          className="size-4 shrink-0 cursor-pointer accent-primary"
        />
      )}
      {expanded
        ? <ChevronDown className="size-4 text-muted-foreground shrink-0" />
        : <ChevronRight className="size-4 text-muted-foreground shrink-0" />}
      <span className="text-xs px-2 py-0.5 rounded bg-muted font-mono uppercase">
        {result.result_type}
      </span>
      <span className="flex-1 text-sm">{result.source_title}</span>
      <span className="text-xs text-muted-foreground">
        {new Date(result.created_at).toLocaleDateString()}
      </span>
      <Badge variant={statusCfg.variant} className="text-xs shrink-0">
        {statusCfg.label}
      </Badge>
    </button>
  )
}

// --- ParserPendingRow ---

function ParserPendingRow({
  result,
  onAnyChange,
  selected,
  onToggleSelect,
}: {
  result: PendingAiResult
  onAnyChange: () => void
  selected?: boolean
  onToggleSelect?: () => void
}) {
  const [expanded, setExpanded] = useState(false)
  const [localResult, setLocalResult] = useState(result)
  const [form, setForm] = useState<ParserFormState>(() => toParserForm(result.generated_data))
  const [preview, setPreview] = useState<ParserPreviewResponse | null>(null)
  const previewDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const saveDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const skipSaveRef = useRef(true)
  const userEditedRef = useRef(false)

  // Sync when backend updates (status change: generating → pending)
  useEffect(() => {
    skipSaveRef.current = true
    setLocalResult(result)
    setForm(toParserForm(result.generated_data))
    userEditedRef.current = result.status === "pending" && result.generated_data !== null
  }, [result.updated_at])

  const { mutate: updateData } = useEffectMutation(
    (generated_data: Record<string, unknown>) =>
      Effect.flatMap(CoreApi, (api) =>
        api.updatePendingAiResult(localResult.id, { generated_data }),
      ),
  )

  const updateForm = (key: string, value: string | number | null) => {
    userEditedRef.current = true
    setForm((prev) => ({ ...prev, [key]: value }))
  }

  // Auto-preview (300ms debounce)
  useEffect(() => {
    if (!userEditedRef.current || !form.condition_regex || !form.parse_regex) {
      setPreview(null)
      return
    }
    if (previewDebounceRef.current) clearTimeout(previewDebounceRef.current)
    previewDebounceRef.current = setTimeout(() => {
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) =>
          api.previewParser({
            target_type: localResult.subscription_id != null ? "subscription" : "global",
            target_id: localResult.subscription_id ?? null,
            ...buildParserRequest(form),
          }),
        ),
      ).then(setPreview).catch(() => setPreview(null))
    }, 300)
    return () => { if (previewDebounceRef.current) clearTimeout(previewDebounceRef.current) }
  }, [form, localResult.subscription_id])

  // Debounced save (1000ms)
  useEffect(() => {
    if (skipSaveRef.current) { skipSaveRef.current = false; return }
    if (saveDebounceRef.current) clearTimeout(saveDebounceRef.current)
    saveDebounceRef.current = setTimeout(() => {
      updateData(buildParserRequest(form) as Record<string, unknown>)
    }, 1000)
    return () => { if (saveDebounceRef.current) clearTimeout(saveDebounceRef.current) }
  }, [form])

  const handleDone = () => { setExpanded(false); onAnyChange() }
  const handleRegenerated = (updated: PendingAiResult) => {
    setLocalResult(updated)
    setForm(toParserForm(updated.generated_data))
    onAnyChange()
  }

  return (
    <div className="border rounded-lg overflow-hidden">
      <RowHeader
        expanded={expanded}
        result={localResult}
        onToggle={() => setExpanded((prev) => !prev)}
        selected={selected}
        onToggleSelect={onToggleSelect}
      />
      {expanded && (
        <div className="border-t px-4 py-4 bg-muted/20">
          <AiResultPanel
            result={localResult}
            onConfirmed={handleDone}
            onRejected={handleDone}
            onRegenerated={handleRegenerated}
            defaultLevel={localResult.subscription_id != null ? "subscription" : "global"}
            defaultTargetId={localResult.subscription_id ?? undefined}
          >
            <div className="space-y-4">
              <ParserFormFields
                form={form}
                onChange={updateForm}
                targetType="subscription"
                targetId={localResult.subscription_id}
              />
              <ParserPreviewSection preview={preview} />
            </div>
          </AiResultPanel>
        </div>
      )}
    </div>
  )
}

// --- FilterPendingRow ---

function FilterPendingRow({
  result,
  onAnyChange,
  selected,
  onToggleSelect,
}: {
  result: PendingAiResult
  onAnyChange: () => void
  selected?: boolean
  onToggleSelect?: () => void
}) {
  const [expanded, setExpanded] = useState(true)
  const [localResult, setLocalResult] = useState(result)
  const [localRules, setLocalRules] = useState<FilterRuleDraft[]>(() =>
    parseFilterRules(result.generated_data).map((r) => ({
      is_positive: r.is_positive,
      regex_pattern: r.regex_pattern,
      reasoning: r.reasoning,
    })),
  )
  const saveDebouncRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const skipSaveRef = useRef(true)

  // Sync when backend updates
  useEffect(() => {
    skipSaveRef.current = true
    setLocalResult(result)
    setLocalRules(
      parseFilterRules(result.generated_data).map((r) => ({
        is_positive: r.is_positive,
        regex_pattern: r.regex_pattern,
        reasoning: r.reasoning,
      })),
    )
  }, [result.updated_at])

  const { mutate: updateData } = useEffectMutation(
    (generated_data: Record<string, unknown>) =>
      Effect.flatMap(CoreApi, (api) =>
        api.updatePendingAiResult(localResult.id, { generated_data }),
      ),
  )

  // Debounced save (1000ms) whenever localRules changes
  useEffect(() => {
    if (skipSaveRef.current) { skipSaveRef.current = false; return }
    if (saveDebouncRef.current) clearTimeout(saveDebouncRef.current)
    saveDebouncRef.current = setTimeout(() => {
      updateData({ rules: localRules })
    }, 1000)
    return () => { if (saveDebouncRef.current) clearTimeout(saveDebouncRef.current) }
  }, [localRules])

  // confirm 前先強制 flush debounce，確保後端讀到最新的 generated_data
  const flushSave = useCallback(async () => {
    if (saveDebouncRef.current) {
      clearTimeout(saveDebouncRef.current)
      saveDebouncRef.current = null
    }
    await updateData({ rules: localRules })
  }, [localRules, updateData])

  const handleUpdate = (idx: number, changes: { is_positive?: boolean; regex_pattern?: string }) => {
    setLocalRules((prev) => prev.map((r, i) => i === idx ? { ...r, ...changes } : r))
  }

  const handleDelete = (idx: number) => {
    setLocalRules((prev) => prev.filter((_, i) => i !== idx))
  }

  const handleAdd = (rule: { is_positive: boolean; regex_pattern: string }) => {
    setLocalRules((prev) => [...prev, { is_positive: rule.is_positive, regex_pattern: rule.regex_pattern }])
  }

  const handleDone = () => { setExpanded(false); onAnyChange() }
  const handleRegenerated = (updated: PendingAiResult) => {
    skipSaveRef.current = true
    setLocalResult(updated)
    setLocalRules(
      parseFilterRules(updated.generated_data).map((r) => ({
        is_positive: r.is_positive,
        regex_pattern: r.regex_pattern,
        reasoning: r.reasoning,
      })),
    )
    onAnyChange()
  }

  return (
    <div className="border rounded-lg overflow-hidden">
      <RowHeader
        expanded={expanded}
        result={localResult}
        onToggle={() => setExpanded((prev) => !prev)}
        selected={selected}
        onToggleSelect={onToggleSelect}
      />
      {expanded && (
        <div className="border-t px-4 py-4 bg-muted/20">
          <AiResultPanel
            result={localResult}
            onConfirmed={handleDone}
            onRejected={handleDone}
            onRegenerated={handleRegenerated}
            onBeforeConfirm={flushSave}
            defaultLevel={localResult.subscription_id != null ? "subscription" : "global"}
            defaultTargetId={localResult.subscription_id ?? undefined}
          >
            <FilterRulePanel
              rules={localRules}
              targetType={localResult.subscription_id != null ? "subscription" : "global"}
              targetId={localResult.subscription_id ?? null}
              onAddSuccess={() => {/* rules already updated via handleAdd */}}
              addRuleOverride={handleAdd}
              onDelete={handleDelete}
              onUpdate={handleUpdate}
              requireDeleteConfirm={false}
            />
          </AiResultPanel>
        </div>
      )}
    </div>
  )
}

// --- WizardPendingRow (dispatcher) ---

interface WizardPendingRowProps {
  result: PendingAiResult
  onAnyChange: () => void
  selected?: boolean
  onToggleSelect?: () => void
}

function WizardPendingRow({ result, onAnyChange, selected, onToggleSelect }: WizardPendingRowProps) {
  if (result.result_type === "filter") {
    return <FilterPendingRow result={result} onAnyChange={onAnyChange} selected={selected} onToggleSelect={onToggleSelect} />
  }
  return <ParserPendingRow result={result} onAnyChange={onAnyChange} selected={selected} onToggleSelect={onToggleSelect} />
}

// --- WizardPendingList ---

interface WizardPendingListProps {
  results: readonly PendingAiResult[]
  onAnyChange: () => void
  selectedIds?: Set<number>
  onToggleSelect?: (id: number) => void
}

export function WizardPendingList({ results, onAnyChange, selectedIds, onToggleSelect }: WizardPendingListProps) {
  if (results.length === 0) {
    return (
      <p className="text-center text-muted-foreground py-8">沒有待確認項目</p>
    )
  }

  return (
    <div className="space-y-3">
      {results.map((result) => (
        <WizardPendingRow
          key={result.id}
          result={result}
          onAnyChange={onAnyChange}
          selected={selectedIds?.has(result.id)}
          onToggleSelect={result.status === "pending" && onToggleSelect
            ? () => onToggleSelect(result.id)
            : undefined}
        />
      ))}
    </div>
  )
}
