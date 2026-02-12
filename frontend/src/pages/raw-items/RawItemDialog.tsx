import { useTranslation } from "react-i18next"
import { Link } from "react-router-dom"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { StatusBadge } from "@/components/shared/StatusBadge"

interface RawItemDialogProps {
  itemId: number
  open: boolean
  onOpenChange: (open: boolean) => void
  subMap: Map<number, string>
  parserMap: Map<number, string>
}

export function RawItemDialog({
  itemId,
  open,
  onOpenChange,
  subMap,
  parserMap,
}: RawItemDialogProps) {
  const { t } = useTranslation()

  const { data: item, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getRawItem(itemId)
      }),
    [itemId],
  )

  return (
    <FullScreenDialog
      open={open}
      onOpenChange={onOpenChange}
      title={item ? item.title : `Raw Item #${itemId}`}
    >
      {isLoading || !item ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <div className="space-y-6">
          {/* Basic info */}
          <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
            <InfoItem label={t("common.id")} value={String(item.item_id)} />
            <InfoItem label={t("common.status")}>
              <StatusBadge status={item.status} />
            </InfoItem>
            <InfoItem
              label={t("rawItems.created")}
              value={item.created_at.slice(0, 19).replace("T", " ")}
            />
            {item.pub_date && (
              <InfoItem
                label={t("rawItems.pubDate", "Published")}
                value={item.pub_date.slice(0, 19).replace("T", " ")}
              />
            )}
            {item.parsed_at && (
              <InfoItem
                label={t("rawItems.parsedAt", "Parsed At")}
                value={item.parsed_at.slice(0, 19).replace("T", " ")}
              />
            )}
          </div>

          {/* Title */}
          <div>
            <p className="text-xs text-muted-foreground mb-1">{t("rawItems.itemTitle")}</p>
            <p className="text-sm font-mono break-all bg-muted/50 rounded p-2">{item.title}</p>
          </div>

          {/* Download URL */}
          <div>
            <p className="text-xs text-muted-foreground mb-1">{t("rawItems.downloadUrl", "Download URL")}</p>
            <p className="text-sm font-mono break-all bg-muted/50 rounded p-2">{item.download_url}</p>
          </div>

          {/* Error message */}
          {item.error_message && (
            <div>
              <p className="text-xs text-muted-foreground mb-1">{t("rawItems.errorMessage", "Error")}</p>
              <p className="text-sm font-mono break-all bg-red-50 dark:bg-red-950/30 text-red-800 dark:text-red-300 rounded p-2">
                {item.error_message}
              </p>
            </div>
          )}

          {/* Subscription */}
          <div>
            <p className="text-xs text-muted-foreground mb-1">{t("rawItems.subId")}</p>
            <Link
              to="/subscriptions"
              className="text-sm text-primary underline"
            >
              {subMap.get(item.subscription_id) ?? `#${item.subscription_id}`}
            </Link>
          </div>

          {/* Parser */}
          {item.parser_id != null && (
            <div>
              <p className="text-xs text-muted-foreground mb-1">{t("rawItems.parser")}</p>
              <Link
                to="/parsers"
                className="text-sm text-primary underline"
              >
                {parserMap.get(item.parser_id) ?? `#${item.parser_id}`}
              </Link>
            </div>
          )}

          {/* Description */}
          {item.description && (
            <div>
              <p className="text-xs text-muted-foreground mb-1">{t("rawItems.description", "Description")}</p>
              <p className="text-sm bg-muted/50 rounded p-2 whitespace-pre-wrap">{item.description}</p>
            </div>
          )}
        </div>
      )}
    </FullScreenDialog>
  )
}

function InfoItem({
  label,
  value,
  children,
}: {
  label: string
  value?: string
  children?: React.ReactNode
}) {
  return (
    <div>
      <p className="text-xs text-muted-foreground">{label}</p>
      {children ?? <p className="text-sm font-medium">{value}</p>}
    </div>
  )
}
