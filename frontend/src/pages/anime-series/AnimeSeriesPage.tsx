import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { Badge } from "@/components/ui/badge"
import { AnimeSeriesDialog } from "./AnimeSeriesDialog"
import type { AnimeSeriesRich } from "@/schemas/anime"

export default function AnimeSeriesPage() {
  const { t } = useTranslation()
  const [selected, setSelected] = useState<AnimeSeriesRich | null>(null)

  const { data: seriesList, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getAllAnimeSeries
      }),
    [],
  )

  const columns: Column<Record<string, unknown>>[] = [
    {
      key: "anime_title",
      header: t("animeSeries.animeTitle", "Anime"),
      render: (item) => (
        <span className="font-medium">{String(item.anime_title)}</span>
      ),
    },
    {
      key: "season",
      header: t("animeSeries.season", "Season"),
      render: (item) => {
        const s = item.season as { year: number; season: string }
        return (
          <Badge variant="outline">
            {s.year} {s.season}
          </Badge>
        )
      },
    },
    {
      key: "episodes",
      header: t("animeSeries.episodes", "Episodes"),
      render: (item) => (
        <span className="tabular-nums">
          {String(item.episode_downloaded)} / {String(item.episode_found)}
        </span>
      ),
    },
    {
      key: "subscriptions",
      header: t("animeSeries.subscriptions", "Subscriptions"),
      render: (item) => {
        const subs = item.subscriptions as Array<{ subscription_id: number; name: string | null }>
        if (!subs || subs.length === 0) return <span className="text-muted-foreground">-</span>
        return (
          <div className="flex flex-wrap gap-1">
            {subs.map((sub) => (
              <Badge key={sub.subscription_id} variant="secondary" className="text-xs">
                {sub.name ?? `#${sub.subscription_id}`}
              </Badge>
            ))}
          </div>
        )
      },
    },
  ]

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">{t("animeSeries.title", "Anime Series")}</h1>
      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <DataTable
          columns={columns}
          data={(seriesList ?? []) as unknown as Record<string, unknown>[]}
          keyField="series_id"
          onRowClick={(row) => {
            const rich = (seriesList ?? []).find((s) => s.series_id === row.series_id)
            if (rich) setSelected(rich)
          }}
        />
      )}

      {selected && (
        <AnimeSeriesDialog
          series={selected}
          open={!!selected}
          onOpenChange={(open) => {
            if (!open) {
              setSelected(null)
              refetch()
            }
          }}
        />
      )}
    </div>
  )
}
