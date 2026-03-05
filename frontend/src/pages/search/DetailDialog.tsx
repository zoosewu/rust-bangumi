import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import type { SearchResult } from "@/schemas/search"

interface DetailDialogProps {
  result: SearchResult | null
  onClose: () => void
  onSubscribeClick?: (sourceUrl: string, name: string) => void
}

export function DetailDialog({ result, onClose, onSubscribeClick }: DetailDialogProps) {
  const { t } = useTranslation()

  const { data: detail, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        if (!result) return { items: [] }
        return yield* api.getDetail(result.detail_key, result.source)
      }),
    [result?.detail_key, result?.source],
  )

  const items = detail?.items ?? []

  return (
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
                      onClick={() =>
                        onSubscribeClick?.(
                          item.rss_url,
                          `${result?.title ?? ""} - ${item.subgroup_name}`,
                        )
                      }
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
  )
}
