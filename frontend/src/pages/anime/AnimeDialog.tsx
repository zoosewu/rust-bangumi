import { useState, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { ChevronLeft, ChevronRight } from "lucide-react"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"
import { ParserEditor } from "@/components/shared/ParserEditor"
import { InfoSection } from "@/components/shared/InfoSection"
import { InfoItem } from "@/components/shared/InfoItem"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { AnimeDialog as AnimeSeriesDialog } from "@/pages/anime-series/AnimeSeriesDialog"
import type { AnimeWork, AnimeRich } from "@/schemas/anime"

interface AnimeWorkDialogProps {
  anime: AnimeWork
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function AnimeWorkDialog({ anime, open, onOpenChange }: AnimeWorkDialogProps) {
  const { t } = useTranslation()
  const [selectedSeries, setSelectedSeries] = useState<AnimeRich | null>(null)
  const [coverIndex, setCoverIndex] = useState(0)

  const { data: allSeries, refetch: refetchSeries } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getAllAnime()
      }),
    [],
  )

  const { data: coversData, refetch: refetchCovers } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getAnimeCoverImages(anime.anime_id)
      }),
    [anime.anime_id],
  )

  // Reset cover index when covers data changes (e.g. anime switches)
  useEffect(() => {
    if (coversData && coversData.length > 0) {
      const defaultIdx = coversData.findIndex((c) => c.is_default)
      setCoverIndex(defaultIdx >= 0 ? defaultIdx : 0)
    }
  }, [coversData])

  const { mutate: doSetDefault } = useEffectMutation(
    (animeId: number, coverId: number) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.setDefaultCoverImage(animeId, coverId)
      }),
  )

  const covers = coversData ?? []

  // Filter series belonging to this anime
  const animeSeries = (allSeries ?? []).filter((s) => s.anime_id === anime.anime_id)

  const handleSetDefault = async (coverId: number) => {
    await doSetDefault(anime.anime_id, coverId)
    refetchCovers()
  }

  return (
    <>
      <FullScreenDialog
        open={open}
        onOpenChange={onOpenChange}
        title={anime.title}
      >
        <div className="space-y-6">
          {/* Cover image switcher */}
          {covers.length > 0 && (
            <div className="group relative w-40 mx-auto aspect-[2/3] flex-shrink-0 mb-4">
              <img
                src={covers[coverIndex]?.image_url}
                alt="Cover"
                className="w-full h-full object-cover rounded-lg"
              />
              {covers.length > 1 && (
                <>
                  <button
                    className="absolute left-1 top-1/2 -translate-y-1/2 bg-black/50 hover:bg-black/70 text-white rounded-full p-0.5 opacity-0 group-hover:opacity-100 transition-opacity"
                    onClick={async (e) => {
                      e.stopPropagation()
                      const newIdx = (coverIndex - 1 + covers.length) % covers.length
                      setCoverIndex(newIdx)
                      await handleSetDefault(covers[newIdx].cover_id)
                    }}
                  >
                    <ChevronLeft className="h-4 w-4" />
                  </button>
                  <button
                    className="absolute right-1 top-1/2 -translate-y-1/2 bg-black/50 hover:bg-black/70 text-white rounded-full p-0.5 opacity-0 group-hover:opacity-100 transition-opacity"
                    onClick={async (e) => {
                      e.stopPropagation()
                      const newIdx = (coverIndex + 1) % covers.length
                      setCoverIndex(newIdx)
                      await handleSetDefault(covers[newIdx].cover_id)
                    }}
                  >
                    <ChevronRight className="h-4 w-4" />
                  </button>
                </>
              )}
              <div className="absolute bottom-1 left-0 right-0 text-center pointer-events-none">
                <span className="text-white/70 text-xs bg-black/30 px-1 rounded">
                  {coverIndex + 1}/{covers.length}
                </span>
              </div>
            </div>
          )}

          {/* Anime info */}
          <InfoSection>
            <InfoItem label={t("common.id")} value={String(anime.anime_id)} />
            <InfoItem label={t("common.name")} value={anime.title} />
            <InfoItem label={t("anime.created", "Created")} value={anime.created_at.slice(0, 10)} />
          </InfoSection>

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
