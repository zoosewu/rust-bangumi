import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { ConfirmDialog } from "@/components/shared/ConfirmDialog"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { Plus, Trash2 } from "lucide-react"

export default function Dashboard() {
  const { t } = useTranslation()
  const [createOpen, setCreateOpen] = useState(false)
  const [newRegex, setNewRegex] = useState("")
  const [newPositive, setNewPositive] = useState(true)
  const [newOrder, setNewOrder] = useState("1")
  const [deleteTarget, setDeleteTarget] = useState<number | null>(null)

  const health = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getHealth
      }),
    [],
  )

  const { data: rules, isLoading: rulesLoading, refetch: refetchRules } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getFilterRules("global")
      }),
    [],
  )

  const { mutate: createRule, isLoading: creating } = useEffectMutation(
    (req: { regex_pattern: string; is_positive: boolean; rule_order: number }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createFilterRule({
          target_type: "global",
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
            setDeleteTarget(item.rule_id as number)
          }}
        >
          <Trash2 className="h-4 w-4 text-destructive" />
        </Button>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">{t("dashboard.title")}</h1>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {t("dashboard.coreService")}
            </CardTitle>
          </CardHeader>
          <CardContent>
            {health.isLoading ? (
              <span className="text-muted-foreground">{t("common.checking")}</span>
            ) : health.error ? (
              <Badge variant="destructive">{t("common.offline")}</Badge>
            ) : (
              <Badge className="bg-green-100 text-green-800">{t("common.online")}</Badge>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Global Filter Rules */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">{t("dashboard.globalFilterRules")}</h2>
          <Button size="sm" onClick={() => setCreateOpen(true)}>
            <Plus className="h-4 w-4 mr-2" />
            {t("dashboard.addRule")}
          </Button>
        </div>
        {rulesLoading ? (
          <p className="text-muted-foreground">{t("common.loading")}</p>
        ) : rules && rules.length > 0 ? (
          <DataTable
            columns={ruleColumns}
            data={rules as unknown as Record<string, unknown>[]}
            keyField="rule_id"
          />
        ) : (
          <Card>
            <CardContent className="py-6 text-center text-muted-foreground">
              {t("dashboard.noGlobalRules")}
            </CardContent>
          </Card>
        )}
      </div>

      {/* Create Rule Dialog */}
      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("dashboard.addGlobalRule")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div>
              <Label>{t("dashboard.regexPattern")}</Label>
              <Input
                className="font-mono"
                value={newRegex}
                onChange={(e) => setNewRegex(e.target.value)}
                placeholder="e.g. 1080p"
              />
            </div>
            <div>
              <Label>{t("dashboard.ruleOrder")}</Label>
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
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              disabled={!newRegex.trim() || creating}
              onClick={() => {
                createRule({
                  regex_pattern: newRegex.trim(),
                  is_positive: newPositive,
                  rule_order: Number(newOrder) || 1,
                }).then(() => {
                  setNewRegex("")
                  setNewPositive(true)
                  setNewOrder("1")
                  setCreateOpen(false)
                  refetchRules()
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
        open={deleteTarget !== null}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title={t("dashboard.deleteRule")}
        description={t("dashboard.deleteRuleConfirm")}
        loading={deleting}
        onConfirm={() => {
          if (deleteTarget !== null) {
            deleteRule(deleteTarget).then(() => {
              setDeleteTarget(null)
              refetchRules()
            })
          }
        }}
      />
    </div>
  )
}
