import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { SearchBar } from "@/components/shared/SearchBar"
import { PageHeader } from "@/components/shared/PageHeader"
import { Badge } from "@/components/ui/badge"
import type { SearchResult } from "@/schemas/search"
import { DetailDialog } from "./DetailDialog"

export default function SearchPage() {
  const { t } = useTranslation()
  const [rawQuery, setRawQuery] = useState("")
  const [debouncedQuery, setDebouncedQuery] = useState("")
  const [selectedResult, setSelectedResult] = useState<SearchResult | null>(null)

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(rawQuery.trim())
    }, 500)
    return () => clearTimeout(timer)
  }, [rawQuery])

  const { data: results, isLoading, error } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        if (!debouncedQuery) return { results: [] }
        return yield* api.search(debouncedQuery)
      }),
    [debouncedQuery],
  )

  const searchResults = results?.results ?? []

  return (
    <div className="space-y-6">
      <PageHeader title={t("search.title")} />

      <SearchBar
        value={rawQuery}
        onChange={setRawQuery}
        placeholder={t("search.placeholder")}
      />

      {isLoading && debouncedQuery && (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      )}

      {!!error && (
        <p className="text-destructive text-sm">
          {t("common.error")}: {String(error)}
        </p>
      )}

      {!isLoading && !error && debouncedQuery && searchResults.length === 0 && (
        <p className="text-sm text-muted-foreground">{t("search.noResults")}</p>
      )}

      {!debouncedQuery && !isLoading && (
        <p className="text-sm text-muted-foreground">
          {t("search.hint", "Type to search across all sources")}
        </p>
      )}

      {searchResults.length > 0 && (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4">
          {searchResults.map((result, idx) => (
            <button
              key={`${result.source}-${result.detail_key}-${idx}`}
              type="button"
              className="flex flex-col items-center gap-2 p-3 border rounded-lg bg-card hover:bg-accent cursor-pointer text-left transition-colors"
              onClick={() => setSelectedResult(result)}
            >
              <div className="w-full aspect-[3/4] rounded overflow-hidden bg-muted flex-shrink-0">
                {result.thumbnail_url ? (
                  <img
                    src={result.thumbnail_url}
                    alt={result.title}
                    className="w-full h-full object-cover"
                    onError={(e) => {
                      ;(e.target as HTMLImageElement).style.display = "none"
                    }}
                  />
                ) : (
                  <div className="w-full h-full flex items-center justify-center text-muted-foreground text-xs">
                    {t("search.noImage")}
                  </div>
                )}
              </div>
              <p className="text-sm font-medium line-clamp-2 w-full">{result.title}</p>
              <Badge variant="outline" className="text-xs self-start">
                {result.source}
              </Badge>
            </button>
          ))}
        </div>
      )}

      <DetailDialog
        result={selectedResult}
        onClose={() => setSelectedResult(null)}
      />
    </div>
  )
}
