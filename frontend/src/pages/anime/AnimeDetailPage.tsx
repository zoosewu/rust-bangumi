import { useParams, useNavigate } from "react-router-dom"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { ArrowLeft } from "lucide-react"

export default function AnimeDetailPage() {
  const { animeId } = useParams<{ animeId: string }>()
  const navigate = useNavigate()
  const id = Number(animeId)

  const { data: animes, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getAnimes
      }),
    [],
  )

  const anime = animes?.find((a) => a.anime_id === id)

  const { data: filterRules } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getFilterRules("anime", id)
      }),
    [id],
  )

  const filterColumns: Column<Record<string, unknown>>[] = [
    {
      key: "rule_id",
      header: "ID",
      render: (item) => String(item.rule_id),
    },
    {
      key: "regex_pattern",
      header: "Pattern",
      render: (item) => (
        <code className="text-sm font-mono">{String(item.regex_pattern)}</code>
      ),
    },
    {
      key: "is_positive",
      header: "Type",
      render: (item) => (item.is_positive ? "Include" : "Exclude"),
    },
    {
      key: "rule_order",
      header: "Order",
      render: (item) => String(item.rule_order),
    },
  ]

  if (isLoading) {
    return <p className="text-muted-foreground">Loading...</p>
  }

  if (!anime) {
    return <p className="text-destructive">Anime not found</p>
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="sm" onClick={() => navigate("/anime")}>
          <ArrowLeft className="h-4 w-4 mr-1" />
          Back
        </Button>
        <h1 className="text-2xl font-bold">{anime.title}</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm text-muted-foreground">Details</CardTitle>
        </CardHeader>
        <CardContent className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span className="text-muted-foreground">ID:</span> {anime.anime_id}
          </div>
          <div>
            <span className="text-muted-foreground">Created:</span>{" "}
            {anime.created_at.slice(0, 10)}
          </div>
        </CardContent>
      </Card>

      <Tabs defaultValue="filters">
        <TabsList>
          <TabsTrigger value="filters">Filter Rules</TabsTrigger>
        </TabsList>
        <TabsContent value="filters" className="mt-4">
          {filterRules && filterRules.length > 0 ? (
            <DataTable
              columns={filterColumns}
              data={filterRules as unknown as Record<string, unknown>[]}
              keyField="rule_id"
            />
          ) : (
            <p className="text-sm text-muted-foreground">
              No filter rules for this anime.
            </p>
          )}
        </TabsContent>
      </Tabs>
    </div>
  )
}
