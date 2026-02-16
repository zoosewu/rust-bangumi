import { useTranslation } from "react-i18next"
import { Link } from "react-router-dom"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { StatusBadge } from "@/components/shared/StatusBadge"
import { InfoSection } from "@/components/shared/InfoSection"
import { InfoItem } from "@/components/shared/InfoItem"
import { MonospaceBlock } from "@/components/shared/MonospaceBlock"
import { DownloadBadge } from "@/components/shared/DownloadBadge"

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
          {/* Download URL */}
          <MonospaceBlock
            label={t("rawItems.downloadUrl", "Download URL")}
            text={item.download_url}
          />

          {/* Title */}
          <MonospaceBlock
            label={t("rawItems.itemTitle")}
            text={item.title}
            copyable={false}
          />

          {/* Description */}
          {item.description && (
            <MonospaceBlock
              label={t("rawItems.description", "Description")}
              text={item.description}
              copyable={false}
              preWrap
            />
          )}

          {/* Metadata grid */}
          <InfoSection cols={3}>
            <InfoItem label={t("common.id")} value={String(item.item_id)} />
            <InfoItem label={t("common.status")}>
              <StatusBadge status={item.status} />
            </InfoItem>
            {item.filter_passed != null && (
              <InfoItem label={t("rawItems.filterStatus")}>
                <StatusBadge status={item.filter_passed ? "parsed" : "failed"} />
              </InfoItem>
            )}
            <InfoItem label={t("rawItems.download")}>
              {item.download ? (
                <DownloadBadge status={item.download.status} progress={item.download.progress} />
              ) : (
                <span className="text-sm text-muted-foreground">-</span>
              )}
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
          </InfoSection>

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
            <p className="text-xs text-muted-foreground mb-1">{t("rawItems.subscriptionSource")}</p>
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
        </div>
      )}
    </FullScreenDialog>
  )
}
