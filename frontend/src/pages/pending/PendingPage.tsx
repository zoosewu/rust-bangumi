import { useState } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { PageHeader } from "@/components/shared/PageHeader"
import { WizardPendingList } from "@/components/shared/WizardPendingList"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Loader2 } from "lucide-react"

export default function PendingPage() {
  const [activeTab, setActiveTab] = useState("all")

  const { data: results, isLoading, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getPendingAiResults()),
    [],
  )

  const filtered = (results ?? []).filter((r) => {
    if (activeTab === "parser") return r.result_type === "parser"
    if (activeTab === "filter") return r.result_type === "filter"
    return true
  })

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
          ) : (
            <WizardPendingList
              results={filtered}
              onAnyChange={() => refetch()}
            />
          )}
        </TabsContent>
      </Tabs>
    </div>
  )
}
