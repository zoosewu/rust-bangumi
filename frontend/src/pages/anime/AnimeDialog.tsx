import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"
import { ParserEditor } from "@/components/shared/ParserEditor"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { AnimeSeriesDialog } from "@/pages/anime-series/AnimeSeriesDialog"
import type { Anime, AnimeSeriesRich } from "@/schemas/anime"

interface AnimeDialogProps {
  anime: Anime
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function AnimeDialog({ anime, open, onOpenChange }: AnimeDialogProps) {
  const { t } = useTranslation()
  const [selectedSeries, setSelectedSeries] = useState<AnimeSeriesRich | null>(null)

  const { data: allSeries, refetch: refetchSeries } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getAllAnimeSeries
      }),
    [],
  )

  // Filter series belonging to this anime
  const animeSeries = (allSeries ?? []).filter((s) => s.anime_id === anime.anime_id)

  return (
    <>
      <FullScreenDialog
        open={open}
        onOpenChange={onOpenChange}
        title={anime.title}
      >
        <div className="space-y-6">
          {/* Anime info */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <InfoItem label={t("common.id")} value={String(anime.anime_id)} />
            <InfoItem label={t("common.name")} value={anime.title} />
            <InfoItem label={t("anime.created", "Created")} value={anime.created_at.slice(0, 10)} />
          </div>

          {/* Series list */}
          <div className="space-y-2">
            <h3 className="text-sm font-semibold">{t("animeSeries.series", "Seasons")}</h3>
            {animeSeries.length > 0 ? (
              <div className="rounded-md border divide-y">
                {animeSeries.map((s) => (
                  <button
                    key={s.series_id}
                    type="button"
                    className="w-full flex items-center gap-3 px-3 py-2 text-sm text-left hover:bg-accent transition-colors"
                    onClick={() => setSelectedSeries(s)}
                  >
                    <Badge variant="outline">
                      S{s.series_no} - {s.season.year} {s.season.season}
                    </Badge>
                    <span className="tabular-nums">
                      {s.episode_downloaded} / {s.episode_found} eps
                    </span>
                    {s.subscriptions.length > 0 && (
                      <div className="flex gap-1 ml-auto">
                        {s.subscriptions.map((sub) => (
                          <Badge key={sub.subscription_id} variant="secondary" className="text-xs">
                            {sub.name ?? `#${sub.subscription_id}`}
                          </Badge>
                        ))}
                      </div>
                    )}
                  </button>
                ))}
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">{t("animeSeries.noSeries", "No seasons for this anime.")}</p>
            )}
          </div>

          {/* Sub-tabs for filter rules and parsers */}
          <Tabs defaultValue="filters">
            <TabsList variant="line">
              <TabsTrigger value="filters">{t("dialog.filterRules", "Filter Rules")}</TabsTrigger>
              <TabsTrigger value="parsers">{t("dialog.parsers", "Parsers")}</TabsTrigger>
            </TabsList>
            <TabsContent value="filters" className="mt-4">
              <FilterRuleEditor
                targetType="anime"
                targetId={anime.anime_id}
              />
            </TabsContent>
            <TabsContent value="parsers" className="mt-4">
              <ParserEditor
                createdFromType="anime"
                createdFromId={anime.anime_id}
                onParsersChange={refetchSeries}
              />
            </TabsContent>
          </Tabs>
        </div>
      </FullScreenDialog>

      {/* Stacked dialog for anime series */}
      {selectedSeries && (
        <AnimeSeriesDialog
          series={selectedSeries}
          open={!!selectedSeries}
          onOpenChange={(open) => {
            if (!open) setSelectedSeries(null)
          }}
        />
      )}
    </>
  )
}

function InfoItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="text-sm font-medium">{value}</p>
    </div>
  )
}
