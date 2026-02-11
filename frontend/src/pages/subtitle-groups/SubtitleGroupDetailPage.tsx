import { useState } from "react"
import { useParams, useNavigate } from "react-router-dom"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
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
import { ArrowLeft, Plus, Trash2 } from "lucide-react"

export default function SubtitleGroupDetailPage() {
  const { t } = useTranslation()
  const { groupId } = useParams<{ groupId: string }>()
  const navigate = useNavigate()
  const id = Number(groupId)

  const [createOpen, setCreateOpen] = useState(false)
  const [newRegex, setNewRegex] = useState("")
  const [newPositive, setNewPositive] = useState(true)
  const [newOrder, setNewOrder] = useState("1")
  const [deleteTarget, setDeleteTarget] = useState<number | null>(null)

  const { data: groups, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getSubtitleGroups
      }),
    [],
  )

  const group = groups?.find((g) => g.group_id === id)

  const { data: filterRules, refetch: refetchRules } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getFilterRules("subtitle_group", id)
      }),
    [id],
  )

  const { mutate: createRule, isLoading: creating } = useEffectMutation(
    (req: { regex_pattern: string; is_positive: boolean; rule_order: number }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createFilterRule({
          target_type: "subtitle_group",
          target_id: id,
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

  const filterColumns: Column<Record<string, unknown>>[] = [
    {
      key: "rule_id",
      header: t("common.id"),
      render: (item) => String(item.rule_id),
    },
    {
      key: "regex_pattern",
      header: t("common.pattern"),
      render: (item) => (
        <code className="text-sm font-mono">{String(item.regex_pattern)}</code>
      ),
    },
    {
      key: "is_positive",
      header: t("common.type"),
      render: (item) => (item.is_positive ? t("common.include") : t("common.exclude")),
    },
    {
      key: "rule_order",
      header: t("common.order"),
      render: (item) => String(item.rule_order),
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

  if (isLoading) {
    return <p className="text-muted-foreground">{t("common.loading")}</p>
  }

  if (!group) {
    return <p className="text-destructive">{t("subtitleGroups.notFound")}</p>
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="sm" onClick={() => navigate("/subtitle-groups")}>
          <ArrowLeft className="h-4 w-4 mr-1" />
          {t("common.back")}
        </Button>
        <h1 className="text-2xl font-bold">{group.group_name}</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm text-muted-foreground">{t("anime.details")}</CardTitle>
        </CardHeader>
        <CardContent className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span className="text-muted-foreground">{t("common.id")}:</span> {group.group_id}
          </div>
          <div>
            <span className="text-muted-foreground">{t("rawItems.created")}:</span>{" "}
            {group.created_at.slice(0, 10)}
          </div>
        </CardContent>
      </Card>

      <Tabs defaultValue="filters">
        <TabsList>
          <TabsTrigger value="filters">{t("anime.filterRules")}</TabsTrigger>
        </TabsList>
        <TabsContent value="filters" className="mt-4 space-y-4">
          <div className="flex justify-end">
            <Button size="sm" onClick={() => setCreateOpen(true)}>
              <Plus className="h-4 w-4 mr-2" />
              {t("anime.addRule")}
            </Button>
          </div>
          {filterRules && filterRules.length > 0 ? (
            <DataTable
              columns={filterColumns}
              data={filterRules as unknown as Record<string, unknown>[]}
              keyField="rule_id"
            />
          ) : (
            <p className="text-sm text-muted-foreground">
              {t("subtitleGroups.noRules")}
            </p>
          )}
        </TabsContent>
      </Tabs>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("anime.addFilterRule")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div>
              <Label>{t("anime.regexPattern")}</Label>
              <Input
                className="font-mono"
                value={newRegex}
                onChange={(e) => setNewRegex(e.target.value)}
                placeholder="e.g. 1080p"
              />
            </div>
            <div>
              <Label>{t("anime.ruleOrder")}</Label>
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

      <ConfirmDialog
        open={deleteTarget !== null}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title={t("anime.deleteRule")}
        description={t("anime.deleteRuleConfirm")}
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
