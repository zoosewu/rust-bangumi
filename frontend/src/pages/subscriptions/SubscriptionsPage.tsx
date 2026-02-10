import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { StatusBadge } from "@/components/shared/StatusBadge"

export default function SubscriptionsPage() {
  const { data: subscriptions, isLoading } = useEffectQuery(
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
      header: "ID",
      render: (item) => String(item.subscription_id),
    },
    {
      key: "name",
      header: "Name",
      render: (item) => String(item.name ?? item.source_url),
    },
    {
      key: "source_url",
      header: "Source URL",
      render: (item) => (
        <span className="text-xs font-mono truncate max-w-[300px] block">
          {String(item.source_url)}
        </span>
      ),
    },
    {
      key: "fetch_interval_minutes",
      header: "Interval",
      render: (item) => `${item.fetch_interval_minutes} min`,
    },
    {
      key: "is_active",
      header: "Status",
      render: (item) => (
        <StatusBadge status={item.is_active ? "parsed" : "failed"} />
      ),
    },
    {
      key: "last_fetched_at",
      header: "Last Fetched",
      render: (item) =>
        item.last_fetched_at
          ? String(item.last_fetched_at).slice(0, 19).replace("T", " ")
          : "Never",
    },
  ]

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Subscriptions</h1>
      {isLoading ? (
        <p className="text-muted-foreground">Loading...</p>
      ) : (
        <DataTable
          columns={columns}
          data={(subscriptions ?? []) as unknown as Record<string, unknown>[]}
          keyField="subscription_id"
        />
      )}
    </div>
  )
}
