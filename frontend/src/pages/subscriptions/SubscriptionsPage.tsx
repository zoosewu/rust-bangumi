import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { StatusBadge } from "@/components/shared/StatusBadge"
import { SubscriptionDialog } from "./SubscriptionDialog"
import type { Subscription } from "@/schemas/subscription"

export default function SubscriptionsPage() {
  const { t } = useTranslation()
  const [selectedSub, setSelectedSub] = useState<Subscription | null>(null)

  const { data: subscriptions, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getSubscriptions
      }),
    [],
  )

  const columns: Column<Record<string, unknown>>[] = [
    {
      key: "subscription_id",
      header: t("common.id"),
      render: (item) => String(item.subscription_id),
    },
    {
      key: "name",
      header: t("common.name"),
      render: (item) => String(item.name ?? item.source_url),
    },
    {
      key: "source_url",
      header: t("subscriptions.sourceUrl"),
      render: (item) => (
        <span className="text-xs font-mono truncate max-w-[300px] block">
          {String(item.source_url)}
        </span>
      ),
    },
    {
      key: "fetch_interval_minutes",
      header: t("subscriptions.interval"),
      render: (item) => `${item.fetch_interval_minutes} min`,
    },
    {
      key: "is_active",
      header: t("common.status"),
      render: (item) => (
        <StatusBadge status={item.is_active ? "parsed" : "failed"} />
      ),
    },
    {
      key: "last_fetched_at",
      header: t("subscriptions.lastFetched"),
      render: (item) =>
        item.last_fetched_at
          ? String(item.last_fetched_at).slice(0, 19).replace("T", " ")
          : t("common.never"),
    },
  ]

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">{t("subscriptions.title")}</h1>
      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <DataTable
          columns={columns}
          data={(subscriptions ?? []) as unknown as Record<string, unknown>[]}
          keyField="subscription_id"
          onRowClick={(row) => {
            const found = (subscriptions ?? []).find((s) => s.subscription_id === row.subscription_id)
            if (found) setSelectedSub(found)
          }}
        />
      )}

      {selectedSub && (
        <SubscriptionDialog
          subscription={selectedSub}
          open={!!selectedSub}
          onOpenChange={(open) => {
            if (!open) {
              setSelectedSub(null)
              refetch()
            }
          }}
        />
      )}
    </div>
  )
}
