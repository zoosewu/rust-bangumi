import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { SearchBar } from "@/components/shared/SearchBar"
import { PageHeader } from "@/components/shared/PageHeader"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import { toast } from "sonner"
import type { SearchResult } from "@/schemas/search"
import type { ServiceModule } from "@/schemas/service-module"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

export default function SearchPage() {
  const { t } = useTranslation()
  const [rawQuery, setRawQuery] = useState("")
  const [debouncedQuery, setDebouncedQuery] = useState("")
  const [subscribeTarget, setSubscribeTarget] = useState<SearchResult | null>(null)
  const [newName, setNewName] = useState("")
  const [newInterval, setNewInterval] = useState("30")
  const [newPreferredDl, setNewPreferredDl] = useState<number | null>(null)

  // Debounce search input by 500ms
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

  const { data: downloaderModules } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getDownloaderModules
      }),
    [],
  )

  const { mutate: createSubscription, isLoading: creating } = useEffectMutation(
    (req: {
      source_url: string
      name?: string
      fetch_interval_minutes?: number
      preferred_downloader_id?: number | null
    }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createSubscription(req)
      }),
  )

  const handleSubscribeClick = (result: SearchResult) => {
    setSubscribeTarget(result)
    setNewName("")
    setNewInterval("30")
    setNewPreferredDl(null)
  }

  const handleCreateSubscription = () => {
    if (!subscribeTarget) return
    createSubscription({
      source_url: subscribeTarget.subscription_url,
      name: newName || undefined,
      fetch_interval_minutes: Number(newInterval) || 30,
      preferred_downloader_id: newPreferredDl,
    })
      .then(() => {
        toast.success(t("subscriptions.created", "Subscription created"))
        setSubscribeTarget(null)
      })
      .catch(() => {
        toast.error(t("common.saveFailed", "Failed to create subscription"))
      })
  }

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
        <p className="text-sm text-muted-foreground">{t("search.hint", "Type to search across all sources")}</p>
      )}

      {searchResults.length > 0 && (
        <div className="space-y-3">
          {searchResults.map((result, idx) => (
            <div
              key={`${result.source}-${result.subscription_url}-${idx}`}
              className="flex items-start gap-4 p-4 border rounded-lg bg-card"
            >
              <div className="w-16 h-20 flex-shrink-0 rounded overflow-hidden bg-muted">
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
                    {t("search.noImage", "No image")}
                  </div>
                )}
              </div>

              <div className="flex-1 min-w-0">
                <p className="font-medium truncate">{result.title}</p>
                {result.description && (
                  <p className="text-sm text-muted-foreground mt-1 line-clamp-2">
                    {result.description}
                  </p>
                )}
                <div className="flex items-center gap-2 mt-2">
                  <Badge variant="outline" className="text-xs">
                    {result.source}
                  </Badge>
                  <span className="text-xs text-muted-foreground font-mono truncate max-w-[300px]">
                    {result.subscription_url}
                  </span>
                </div>
              </div>

              <Button
                size="sm"
                onClick={() => handleSubscribeClick(result)}
                className="flex-shrink-0"
              >
                {t("search.subscribe")}
              </Button>
            </div>
          ))}
        </div>
      )}

      <Dialog open={!!subscribeTarget} onOpenChange={(open) => { if (!open) setSubscribeTarget(null) }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.addSubscription")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            {subscribeTarget && (
              <div className="space-y-1">
                <Label>{t("subscriptions.sourceUrl")}</Label>
                <p className="text-sm font-mono text-muted-foreground break-all">
                  {subscribeTarget.subscription_url}
                </p>
              </div>
            )}
            <div className="space-y-2">
              <Label>{t("subscriptions.name")}</Label>
              <Input
                placeholder={subscribeTarget?.title ?? t("subscriptions.name")}
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label>{t("subscriptions.fetchInterval")}</Label>
              <Input
                type="number"
                min="1"
                value={newInterval}
                onChange={(e) => setNewInterval(e.target.value)}
              />
            </div>
            {downloaderModules && (downloaderModules as ServiceModule[]).length > 0 && (
              <div className="space-y-2">
                <Label>{t("subscriptions.preferredDownloader")}</Label>
                <Select
                  value={newPreferredDl ? String(newPreferredDl) : "none"}
                  onValueChange={(v) => setNewPreferredDl(v === "none" ? null : Number(v))}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="none">{t("subscriptions.useGlobalPriority")}</SelectItem>
                    {(downloaderModules as ServiceModule[]).map((m) => (
                      <SelectItem key={m.module_id} value={String(m.module_id)}>
                        {m.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setSubscribeTarget(null)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={handleCreateSubscription} disabled={creating}>
              {creating ? t("common.creating") : t("common.create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
