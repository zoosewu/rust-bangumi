import type { AnimeSeriesRich } from "@/schemas/anime"

interface Props {
  series: AnimeSeriesRich
  onClick?: () => void
}

export function AnimeSeriesCard({ series, onClick }: Props) {
  const hasImage = !!series.cover_image_url
  const initial = series.anime_title.charAt(0).toUpperCase()

  return (
    <div
      className="cursor-pointer rounded-lg overflow-hidden border border-border hover:border-primary transition-colors"
      onClick={onClick}
    >
      {/* 封面圖 — 2:3 比例 */}
      <div className="relative w-full aspect-[2/3] bg-muted">
        {hasImage ? (
          <img
            src={series.cover_image_url!}
            alt={series.anime_title}
            className="w-full h-full object-cover"
            loading="lazy"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center text-4xl font-bold text-muted-foreground">
            {initial}
          </div>
        )}
        {/* 標題覆蓋層 */}
        <div className="absolute bottom-0 left-0 right-0 px-2 py-1 bg-gradient-to-t from-black/70 to-transparent">
          <p className="text-white text-sm font-medium line-clamp-2">
            {series.anime_title}
          </p>
        </div>
      </div>

      {/* 資訊列 */}
      <div className="px-2 py-1.5 text-xs text-muted-foreground space-y-0.5">
        <p>
          S{series.series_no}
          {series.season
            ? ` · ${series.season.year} ${series.season.season}`
            : ""}
          {` · ${series.episode_downloaded}/${series.episode_found}`}
        </p>
        {series.subscriptions.length > 0 && (
          <p className="truncate">
            {series.subscriptions.map((s: { name?: string | null }) => s.name ?? "Unknown").join(", ")}
          </p>
        )}
      </div>
    </div>
  )
}
