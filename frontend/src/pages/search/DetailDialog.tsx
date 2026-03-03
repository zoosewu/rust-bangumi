import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { useEffectQuery } from "@/hooks/useEffectQuery"
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { toast } from "sonner"
import type { SearchResult, DetailItem } from "@/schemas/search"

interface DetailDialogProps {
  result: SearchResult | null
  onClose: () => void
}

export function DetailDialog({ result, onClose }: DetailDialogProps) {
  const { t } = useTranslation()
  const [subscribeTarget, setSubscribeTarget] = useState<DetailItem & { animeTitle: string } | null>(null)
  const [newName, setNewName] = useState("")
  const [newInterval, setNewInterval] = useState("30")

  const { data: detail, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        if (!result) return { items: [] }
        return yield* api.getDetail(result.detail_key, result.source)
      }),
    [result?.detail_key, result?.source],
  )

  const { mutate: createSubscription, isLoading: creating } = useEffectMutation(
    (req: { source_url: string; name?: string; fetch_interval_minutes?: number }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createSubscription(req)
      }),
  )

  const handleSubscribeClick = (item: DetailItem) => {
    setSubscribeTarget({ ...item, animeTitle: result?.title ?? "" })
    setNewName(`${result?.title ?? ""} - ${item.subgroup_name}`)
    setNewInterval("30")
  }

  const handleCreateSubscription = () => {
    if (!subscribeTarget) return
    createSubscription({
      source_url: subscribeTarget.rss_url,
      name: newName || undefined,
      fetch_interval_minutes: Number(newInterval) || 30,
    })
      .then(() => {
        toast.success(t("subscriptions.created", "Subscription created"))
        setSubscribeTarget(null)
      })
      .catch(() => {
        toast.error(t("common.saveFailed", "Failed to create subscription"))
      })
  }

  const items = detail?.items ?? []

  return (
    <>
      {/* Main detail dialog */}
      <Dialog open={!!result} onOpenChange={(open) => { if (!open) onClose() }}>
        <DialogContent className="sm:max-w-[calc(100%-2rem)]">
          <DialogHeader>
            <div className="flex items-center gap-4">
              {result?.thumbnail_url && (
                <div className="w-24 h-32 flex-shrink-0 rounded overflow-hidden bg-muted">
                  <img
                    src={result.thumbnail_url}
                    alt={result.title}
                    className="w-full h-full object-cover"
                    onError={(e) => {
                      ;(e.target as HTMLImageElement).style.display = "none"
                    }}
                  />
                </div>
              )}
              <DialogTitle className="text-lg">{result?.title}</DialogTitle>
            </div>
          </DialogHeader>

          {isLoading && (
            <p className="text-sm text-muted-foreground py-4">
              {t("search.detail.loading")}
            </p>
          )}

          {!isLoading && items.length === 0 && (
            <p className="text-sm text-muted-foreground py-4">
              {t("search.detail.noItems")}
            </p>
          )}

          {!isLoading && items.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("search.detail.subgroup")}</TableHead>
                  <TableHead>{t("search.detail.rssUrl")}</TableHead>
                  <TableHead className="w-20" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {items.map((item, idx) => (
                  <TableRow key={idx}>
                    <TableCell className="font-medium whitespace-nowrap">
                      {item.subgroup_name}
                    </TableCell>
                    <TableCell>
                      <button
                        type="button"
                        className="text-xs font-mono text-blue-500 hover:underline text-left break-all"
                        onClick={() =>
                          window.open(item.rss_url, "", "noopener,width=900,height=700")
                        }
                      >
                        {item.rss_url}
                      </button>
                    </TableCell>
                    <TableCell>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => handleSubscribeClick(item)}
                      >
                        {t("search.detail.subscribe")}
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </DialogContent>
      </Dialog>

      {/* Nested subscribe dialog */}
      <Dialog
        open={!!subscribeTarget}
        onOpenChange={(open) => { if (!open) setSubscribeTarget(null) }}
      >
        <DialogContent className="sm:max-w-[calc(100%-2rem)]">
          <DialogHeader>
            <DialogTitle>{t("subscriptions.addSubscription")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            {subscribeTarget && (
              <div className="space-y-1">
                <Label>{t("subscriptions.sourceUrl")}</Label>
                <p className="text-sm font-mono text-muted-foreground break-all">
                  {subscribeTarget.rss_url}
                </p>
              </div>
            )}
            <div className="space-y-2">
              <Label>{t("subscriptions.name")}</Label>
              <Input
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
    </>
  )
}
