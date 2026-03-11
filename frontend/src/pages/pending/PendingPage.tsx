import { useState, useEffect, useRef, useCallback } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { PageHeader } from "@/components/shared/PageHeader"
import { WizardPendingList } from "@/components/shared/WizardPendingList"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Loader2 } from "lucide-react"

export default function PendingPage() {
  const [activeTab, setActiveTab] = useState("all")
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set())
  const [batchProcessing, setBatchProcessing] = useState(false)
  const selectAllRef = useRef<HTMLInputElement>(null)

  const { data: results, isLoading, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getPendingAiResults()),
    [],
  )

  // 當有 generating 項目時，每 3 秒自動輪詢直到全部完成（成功或失敗）
  const hasGenerating = (results ?? []).some((r) => r.status === "generating")
  useEffect(() => {
    if (!hasGenerating) return
    const id = setInterval(() => refetch(), 3000)
    return () => clearInterval(id)
  }, [hasGenerating, refetch])

  const filtered = (results ?? []).filter((r) => {
    if (activeTab === "parser") return r.result_type === "parser"
    if (activeTab === "filter") return r.result_type === "filter"
    return true
  })

  // 當前頁面中可選取的項目（只有 pending 狀態才能批次操作）
  const selectableIds = filtered.filter((r) => r.status === "pending").map((r) => r.id)
  const selectedInView = selectableIds.filter((id) => selectedIds.has(id))
  const allSelected = selectableIds.length > 0 && selectedInView.length === selectableIds.length
  const someSelected = selectedInView.length > 0 && !allSelected

  // 同步全選 checkbox 的 indeterminate 狀態
  useEffect(() => {
    if (selectAllRef.current) {
      selectAllRef.current.indeterminate = someSelected
    }
  }, [someSelected])

  // 切換分頁時清除選取
  useEffect(() => {
    setSelectedIds(new Set())
  }, [activeTab])

  const handleToggleSelect = useCallback((id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }, [])

  const handleSelectAll = () => {
    if (allSelected) {
      setSelectedIds((prev) => {
        const next = new Set(prev)
        selectableIds.forEach((id) => next.delete(id))
        return next
      })
    } else {
      setSelectedIds((prev) => new Set([...prev, ...selectableIds]))
    }
  }

  const handleBatchConfirm = async () => {
    const targets = (results ?? []).filter(
      (r) => selectedIds.has(r.id) && r.status === "pending",
    )
    setBatchProcessing(true)
    await Promise.all(
      targets.map((r) => {
        const level = (r.confirm_level ?? "subscription") as "global" | "subscription" | "anime_work"
        const targetId = level === "global" ? undefined : (r.confirm_target_id ?? undefined)
        return AppRuntime.runPromise(
          Effect.flatMap(CoreApi, (api) =>
            api.confirmPendingAiResult(r.id, { level, target_id: targetId }),
          ),
        ).catch(() => {})
      }),
    )
    setSelectedIds(new Set())
    setBatchProcessing(false)
    refetch()
  }

  const handleBatchReject = async () => {
    const targets = (results ?? []).filter(
      (r) => selectedIds.has(r.id) && r.status === "pending",
    )
    setBatchProcessing(true)
    await Promise.all(
      targets.map((r) =>
        AppRuntime.runPromise(
          Effect.flatMap(CoreApi, (api) => api.rejectPendingAiResult(r.id)),
        ).catch(() => {}),
      ),
    )
    setSelectedIds(new Set())
    setBatchProcessing(false)
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
          {isLoading && results === null ? (
            <div className="flex justify-center py-8">
              <Loader2 className="size-6 animate-spin text-muted-foreground" />
            </div>
          ) : (
            <div className="space-y-3">
              {/* 全選 + 批次操作列 */}
              {selectableIds.length > 0 && (
                <div className="flex items-center gap-3 px-1">
                  <label className="flex items-center gap-2 cursor-pointer select-none text-sm text-muted-foreground">
                    <input
                      ref={selectAllRef}
                      type="checkbox"
                      checked={allSelected}
                      onChange={handleSelectAll}
                      className="size-4 cursor-pointer accent-primary"
                    />
                    全選
                  </label>
                  {selectedInView.length > 0 && (
                    <>
                      <span className="text-sm text-muted-foreground">
                        已選 {selectedInView.length} 個
                      </span>
                      <div className="flex gap-2 ml-auto">
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={handleBatchReject}
                          disabled={batchProcessing}
                        >
                          {batchProcessing && <Loader2 className="mr-1 size-3 animate-spin" />}
                          批次拒絕
                        </Button>
                        <Button
                          size="sm"
                          onClick={handleBatchConfirm}
                          disabled={batchProcessing}
                        >
                          {batchProcessing && <Loader2 className="mr-1 size-3 animate-spin" />}
                          批次通過
                        </Button>
                      </div>
                    </>
                  )}
                </div>
              )}
              <WizardPendingList
                results={filtered}
                onAnyChange={() => refetch()}
                selectedIds={selectedIds}
                onToggleSelect={handleToggleSelect}
              />
            </div>
          )}
        </TabsContent>
      </Tabs>
    </div>
  )
}
