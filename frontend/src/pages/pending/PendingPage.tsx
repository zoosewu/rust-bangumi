import { useState } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { PageHeader } from "@/components/shared/PageHeader"
import { AiResultPanel } from "@/components/shared/AiResultPanel"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { Loader2 } from "lucide-react"
import type { PendingAiResult } from "@/schemas/ai"

export default function PendingPage() {
  const [activeTab, setActiveTab] = useState("all")
  const [expandedId, setExpandedId] = useState<number | null>(null)

  const { data: results, isLoading, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getPendingAiResults()),
    [],
  )

  const filtered = (results ?? []).filter((r) => {
    if (activeTab === "parser") return r.result_type === "parser"
    if (activeTab === "filter") return r.result_type === "filter"
    return true
  })

  const handleDone = () => {
    setExpandedId(null)
    refetch()
  }

  return (
    <div className="space-y-6">
      <PageHeader title="待確認" />

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList variant="line">
          <TabsTrigger value="all">
            全部 {results ? `(${results.length})` : ""}
          </TabsTrigger>
          <TabsTrigger value="parser">
            Parser{" "}
            {results ? `(${results.filter((r) => r.result_type === "parser").length})` : ""}
          </TabsTrigger>
          <TabsTrigger value="filter">
            Filter{" "}
            {results ? `(${results.filter((r) => r.result_type === "filter").length})` : ""}
          </TabsTrigger>
        </TabsList>

        <TabsContent value={activeTab} className="mt-4">
          {isLoading ? (
            <div className="flex justify-center py-8">
              <Loader2 className="size-6 animate-spin text-muted-foreground" />
            </div>
          ) : filtered.length === 0 ? (
            <p className="text-center text-muted-foreground py-8">目前沒有待確認的項目</p>
          ) : (
            <div className="space-y-3">
              {filtered.map((result) => (
                <PendingResultRow
                  key={result.id}
                  result={result}
                  expanded={expandedId === result.id}
                  onToggle={() =>
                    setExpandedId(expandedId === result.id ? null : result.id)
                  }
                  onDone={handleDone}
                />
              ))}
            </div>
          )}
        </TabsContent>
      </Tabs>
    </div>
  )
}

function PendingResultRow({
  result,
  expanded,
  onToggle,
  onDone,
}: {
  result: PendingAiResult
  expanded: boolean
  onToggle: () => void
  onDone: () => void
}) {
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

  return (
    <div className="border rounded-lg overflow-hidden">
      {/* 列表行 */}
      <button
        type="button"
        className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-muted/50 transition-colors"
        onClick={onToggle}
      >
        <span className="text-xs px-2 py-0.5 rounded bg-muted font-mono uppercase">
          {result.result_type}
        </span>
        <span className="flex-1 text-sm">{result.source_title}</span>
        <span className="text-xs text-muted-foreground">
          {new Date(result.created_at).toLocaleDateString()}
        </span>
        <StatusDot status={result.status} />
      </button>

      {/* 展開內容 */}
      {expanded && (
        <div className="border-t px-4 py-4 bg-muted/20">
          <AiResultPanel
            result={localResult}
            onConfirmed={onDone}
            onRejected={onDone}
            onRegenerated={(updated) => setLocalResult(updated)}
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
      className={`size-2 rounded-full ${colors[status] ?? "bg-gray-400"}`}
    />
  )
}
