import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { StatusBadge } from "@/components/shared/StatusBadge"
import { DeleteSubscriptionDialog } from "@/components/shared/DeleteSubscriptionDialog"
import { PageHeader } from "@/components/shared/PageHeader"
import { Button } from "@/components/ui/button"
import { Plus } from "lucide-react"
import { SubscriptionDialog } from "./SubscriptionDialog"
import { CreateSubscriptionWizard } from "./CreateSubscriptionWizard"
import type { Subscription } from "@/schemas/subscription"
import { formatDateTime } from "@/lib/datetime"

export default function SubscriptionsPage() {
  const { t } = useTranslation()
  const [selectedSub, setSelectedSub] = useState<Subscription | null>(null)
  const [createOpen, setCreateOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<{
    id: number
    name: string
  } | null>(null)

  const { data: subscriptions, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getSubscriptions
      }),
    [],
  )

  const { mutate: deleteSubscription, isLoading: deleting } = useEffectMutation(
    ({ id, purge }: { id: number; purge: boolean }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.deleteSubscription(id, purge)
      }),
  )

  const handleDeleteClick = (id: number, name: string) => {
    setDeleteTarget({ id, name })
  }

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
      render: (item) => item.fetch_interval_minutes === 0 ? t("subscriptions.fetchOnce", "Once") : `${item.fetch_interval_minutes} min`,
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
          ? formatDateTime(String(item.last_fetched_at))
          : t("common.never"),
    },
    {
      key: "actions",
      header: "",
      render: (item) => (
        <Button
          variant="ghost"
          size="sm"
          className="text-destructive"
          onClick={(e) => {
            e.stopPropagation()
            handleDeleteClick(
              item.subscription_id as number,
              (item.name ?? item.source_url) as string,
            )
          }}
        >
          {t("common.delete")}
        </Button>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <PageHeader
        title={t("subscriptions.title")}
        actions={
          <Button onClick={() => setCreateOpen(true)}>
            <Plus className="h-4 w-4 mr-2" />
            {t("subscriptions.addSubscription")}
          </Button>
        }
      />

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <DataTable
          columns={columns}
          data={(subscriptions ?? []) as unknown as Record<string, unknown>[]}
          keyField="subscription_id"
          onRowClick={(row) => {
            const found = (subscriptions ?? []).find(
              (s) => s.subscription_id === row.subscription_id,
            )
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
          onSubscriptionChange={refetch}
        />
      )}

      {/* Create Wizard */}
      <CreateSubscriptionWizard
        open={createOpen}
        onOpenChange={setCreateOpen}
        onCreated={refetch}
      />

      {/* Delete Confirm */}
      <DeleteSubscriptionDialog
        open={!!deleteTarget}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        subscriptionName={deleteTarget?.name ?? ""}
        loading={deleting}
        onDeactivate={() => {
          if (deleteTarget) {
            deleteSubscription({ id: deleteTarget.id, purge: false }).then(() => {
              setDeleteTarget(null)
              refetch()
            })
          }
        }}
        onPurge={() => {
          if (deleteTarget) {
            deleteSubscription({ id: deleteTarget.id, purge: true }).then(() => {
              setDeleteTarget(null)
              refetch()
            })
          }
        }}
      />
    </div>
  )
}
