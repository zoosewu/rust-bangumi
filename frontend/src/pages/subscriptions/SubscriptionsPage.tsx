import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { StatusBadge } from "@/components/shared/StatusBadge"
import { ConfirmDialog } from "@/components/shared/ConfirmDialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Filter, Plus, Trash2 } from "lucide-react"

export default function SubscriptionsPage() {
  const { t } = useTranslation()
  const [rulesDialogSub, setRulesDialogSub] = useState<{ id: number; fetcherId: number; name: string } | null>(null)
  const [createRuleOpen, setCreateRuleOpen] = useState(false)
  const [newRegex, setNewRegex] = useState("")
  const [newPositive, setNewPositive] = useState(true)
  const [newOrder, setNewOrder] = useState("1")
  const [deleteRuleTarget, setDeleteRuleTarget] = useState<number | null>(null)

  const { data: subscriptions, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getSubscriptions
      }),
    [],
  )

  const { data: subRules, refetch: refetchSubRules } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        if (!rulesDialogSub) return [] as const
        const api = yield* CoreApi
        return yield* api.getFilterRules("fetcher", rulesDialogSub.fetcherId)
      }),
    [rulesDialogSub?.fetcherId],
  )

  const { mutate: createRule, isLoading: creating } = useEffectMutation(
    (req: { fetcherId: number; regex_pattern: string; is_positive: boolean; rule_order: number }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createFilterRule({
          target_type: "fetcher",
          target_id: req.fetcherId,
          rule_order: req.rule_order,
          is_positive: req.is_positive,
          regex_pattern: req.regex_pattern,
        })
      }),
  )

  const { mutate: deleteRule, isLoading: deleting } = useEffectMutation(
    (ruleId: number) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.deleteFilterRule(ruleId)
      }),
  )

  const ruleColumns: Column<Record<string, unknown>>[] = [
    { key: "rule_id", header: t("common.id"), render: (item) => String(item.rule_id) },
    { key: "rule_order", header: t("common.order"), render: (item) => String(item.rule_order) },
    {
      key: "regex_pattern",
      header: t("common.pattern"),
      render: (item) => <code className="text-sm font-mono">{String(item.regex_pattern)}</code>,
    },
    {
      key: "is_positive",
      header: t("common.type"),
      render: (item) => (item.is_positive ? t("common.include") : t("common.exclude")),
    },
    {
      key: "actions",
      header: "",
      render: (item) => (
        <Button
          variant="ghost"
          size="sm"
          onClick={(e) => {
            e.stopPropagation()
            setDeleteRuleTarget(item.rule_id as number)
          }}
        >
          <Trash2 className="h-4 w-4 text-destructive" />
        </Button>
      ),
    },
  ]

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
    {
      key: "actions",
      header: "",
      render: (item) => (
        <Button
          variant="ghost"
          size="sm"
          onClick={(e) => {
            e.stopPropagation()
            setRulesDialogSub({
              id: item.subscription_id as number,
              fetcherId: item.fetcher_id as number,
              name: String(item.name ?? item.source_url),
            })
          }}
        >
          <Filter className="h-4 w-4" />
        </Button>
      ),
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
        />
      )}

      {/* Filter Rules Dialog */}
      <Dialog
        open={!!rulesDialogSub}
        onOpenChange={(open) => {
          if (!open) {
            setRulesDialogSub(null)
            setCreateRuleOpen(false)
          }
        }}
      >
        <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>{t("subscriptions.filterRulesFor", { name: rulesDialogSub?.name })}</DialogTitle>
          </DialogHeader>

          <div className="space-y-4">
            <div className="flex justify-end">
              <Button size="sm" onClick={() => setCreateRuleOpen(true)}>
                <Plus className="h-4 w-4 mr-2" />
                {t("subscriptions.addRule")}
              </Button>
            </div>
            {subRules && subRules.length > 0 ? (
              <DataTable
                columns={ruleColumns}
                data={subRules as unknown as Record<string, unknown>[]}
                keyField="rule_id"
              />
            ) : (
              <p className="text-sm text-muted-foreground text-center py-4">
                {t("subscriptions.noRules")}
              </p>
            )}
          </div>
        </DialogContent>
      </Dialog>

      {/* Create Rule Dialog */}
      <Dialog open={createRuleOpen} onOpenChange={setCreateRuleOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.addFilterRule")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div>
              <Label>{t("subscriptions.regexPattern")}</Label>
              <Input
                className="font-mono"
                value={newRegex}
                onChange={(e) => setNewRegex(e.target.value)}
                placeholder="e.g. 1080p"
              />
            </div>
            <div>
              <Label>{t("subscriptions.ruleOrder")}</Label>
              <Input
                type="number"
                value={newOrder}
                onChange={(e) => setNewOrder(e.target.value)}
              />
            </div>
            <div className="flex items-center gap-2">
              <Switch checked={newPositive} onCheckedChange={setNewPositive} />
              <Label>{newPositive ? t("common.include") : t("common.exclude")}</Label>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateRuleOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              disabled={!newRegex.trim() || creating || !rulesDialogSub}
              onClick={() => {
                if (!rulesDialogSub) return
                createRule({
                  fetcherId: rulesDialogSub.fetcherId,
                  regex_pattern: newRegex.trim(),
                  is_positive: newPositive,
                  rule_order: Number(newOrder) || 1,
                }).then(() => {
                  setNewRegex("")
                  setNewPositive(true)
                  setNewOrder("1")
                  setCreateRuleOpen(false)
                  refetchSubRules()
                })
              }}
            >
              {creating ? t("common.creating") : t("common.create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirm */}
      <ConfirmDialog
        open={deleteRuleTarget !== null}
        onOpenChange={(open) => !open && setDeleteRuleTarget(null)}
        title={t("subscriptions.deleteRule")}
        description={t("subscriptions.deleteRuleConfirm")}
        loading={deleting}
        onConfirm={() => {
          if (deleteRuleTarget !== null) {
            deleteRule(deleteRuleTarget).then(() => {
              setDeleteRuleTarget(null)
              refetchSubRules()
            })
          }
        }}
      />
    </div>
  )
}
