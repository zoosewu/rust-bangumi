import { useTranslation } from "react-i18next"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"
import { ParserEditor } from "@/components/shared/ParserEditor"
import { CopyButton } from "@/components/shared/CopyButton"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import type { Subscription } from "@/schemas/subscription"

interface SubscriptionDialogProps {
  subscription: Subscription
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function SubscriptionDialog({ subscription, open, onOpenChange }: SubscriptionDialogProps) {
  const { t } = useTranslation()

  return (
    <FullScreenDialog
      open={open}
      onOpenChange={onOpenChange}
      title={subscription.name ?? String(subscription.source_url)}
    >
      <div className="space-y-6">
        {/* Source URL — standalone at top */}
        <div>
          <p className="text-xs text-muted-foreground mb-1">{t("subscriptions.sourceUrl", "Source URL")}</p>
          <div className="flex items-start gap-1 bg-muted/50 rounded p-2">
            <p className="text-sm font-mono break-all flex-1">{String(subscription.source_url)}</p>
            <CopyButton text={String(subscription.source_url)} />
          </div>
        </div>

        {/* Subscription info */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <InfoItem label={t("common.id")} value={String(subscription.subscription_id)} />
          <InfoItem label={t("common.name")} value={subscription.name ?? "-"} />
          <InfoItem label={t("subscriptions.interval", "Interval")} value={`${subscription.fetch_interval_minutes} min`} />
          <InfoItem
            label={t("common.status")}
            value={subscription.is_active ? "Active" : "Inactive"}
          />
          <InfoItem
            label={t("subscriptions.lastFetched", "Last Fetched")}
            value={subscription.last_fetched_at ? String(subscription.last_fetched_at).slice(0, 19).replace("T", " ") : t("common.never")}
          />
        </div>

        {/* Sub-tabs for filter rules and parsers */}
        <Tabs defaultValue="filters">
          <TabsList variant="line">
            <TabsTrigger value="filters">{t("dialog.filterRules", "Filter Rules")}</TabsTrigger>
            <TabsTrigger value="parsers">{t("dialog.parsers", "Parsers")}</TabsTrigger>
          </TabsList>
          <TabsContent value="filters" className="mt-4">
            <FilterRuleEditor
              targetType="fetcher"
              targetId={subscription.subscription_id}
            />
          </TabsContent>
          <TabsContent value="parsers" className="mt-4">
            <ParserEditor
              createdFromType="subscription"
              createdFromId={subscription.subscription_id}
            />
          </TabsContent>
        </Tabs>
      </div>
    </FullScreenDialog>
  )
}

function InfoItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="text-sm font-medium break-all">{value}</p>
    </div>
  )
}
