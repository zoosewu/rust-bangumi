import { useState } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { AiResultPanel } from "@/components/shared/AiResultPanel"
import { Textarea } from "@/components/ui/textarea"
import { ChevronDown, ChevronRight } from "lucide-react"
import type { PendingAiResult } from "@/schemas/ai"

interface WizardPendingRowProps {
  result: PendingAiResult
  onAnyChange: () => void
}

function WizardPendingRow({ result, onAnyChange }: WizardPendingRowProps) {
  const [expanded, setExpanded] = useState(false)
  const [localResult, setLocalResult] = useState(result)

  const { mutate: updateData } = useEffectMutation(
    (generated_data: Record<string, unknown>) =>
      Effect.flatMap(CoreApi, (api) =>
        api.updatePendingAiResult(localResult.id, generated_data),
      ),
  )

  const handleDataChange = (jsonText: string) => {
    try {
      const parsed = JSON.parse(jsonText)
      setLocalResult((prev) => ({ ...prev, generated_data: parsed }))
      updateData(parsed)
    } catch {
      // invalid JSON, ignore
    }
  }

  const handleDone = () => {
    setExpanded(false)
    onAnyChange()
  }

  const handleRegenerated = (updated: PendingAiResult) => {
    setLocalResult(updated)
    onAnyChange()
  }

  return (
    <div className="border rounded-lg overflow-hidden">
      <button
        type="button"
        className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-muted/50 transition-colors"
        onClick={() => setExpanded((prev) => !prev)}
      >
        {expanded ? (
          <ChevronDown className="size-4 text-muted-foreground shrink-0" />
        ) : (
          <ChevronRight className="size-4 text-muted-foreground shrink-0" />
        )}
        <span className="text-xs px-2 py-0.5 rounded bg-muted font-mono uppercase">
          {result.result_type}
        </span>
        <span className="flex-1 text-sm">{result.source_title}</span>
        <span className="text-xs text-muted-foreground">
          {new Date(result.created_at).toLocaleDateString()}
        </span>
        <StatusDot status={result.status} />
      </button>

      {expanded && (
        <div className="border-t px-4 py-4 bg-muted/20">
          <AiResultPanel
            result={localResult}
            onConfirmed={handleDone}
            onRejected={handleDone}
            onRegenerated={handleRegenerated}
          >
            {localResult.generated_data && (
              <div className="space-y-2">
                <p className="text-xs text-muted-foreground font-mono">
                  生成的 {result.result_type} 資料（可直接編輯 JSON）
                </p>
                <Textarea
                  className="font-mono text-xs min-h-[200px]"
                  defaultValue={JSON.stringify(localResult.generated_data, null, 2)}
                  onBlur={(e) => handleDataChange(e.target.value)}
                />
              </div>
            )}
          </AiResultPanel>
        </div>
      )}
    </div>
  )
}

function StatusDot({ status }: { status: string }) {
  const colors: Record<string, string> = {
    generating: "bg-yellow-400 animate-pulse",
    pending: "bg-blue-500",
    confirmed: "bg-green-500",
    failed: "bg-red-500",
  }
  return (
    <span
      className={`size-2 rounded-full shrink-0 ${colors[status] ?? "bg-gray-400"}`}
    />
  )
}

interface WizardPendingListProps {
  results: PendingAiResult[]
  onAnyChange: () => void
}

export function WizardPendingList({ results, onAnyChange }: WizardPendingListProps) {
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
        />
      ))}
    </div>
  )
}
