import { useState, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { RegexInput } from "@/components/shared/RegexInput"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { ConfirmDialog } from "@/components/shared/ConfirmDialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Plus, Trash2 } from "lucide-react"
import type { FilterPreviewResponse } from "@/schemas/filter"

function useDebounce<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value)
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedValue(value), delay)
    return () => clearTimeout(timer)
  }, [value, delay])
  return debouncedValue
}

export default function FiltersPage() {
  const { t } = useTranslation()
  const [regexPattern, setRegexPattern] = useState("")
  const [isPositive, setIsPositive] = useState(true)
  const [preview, setPreview] = useState<FilterPreviewResponse | null>(null)

  // Existing filter rules
  const [createOpen, setCreateOpen] = useState(false)
  const [newRegex, setNewRegex] = useState("")
  const [newPositive, setNewPositive] = useState(true)
  const [newOrder, setNewOrder] = useState("1")
  const [deleteTarget, setDeleteTarget] = useState<number | null>(null)

  const { data: rules, isLoading: rulesLoading, refetch } = useEffectQuery(
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

  const debouncedRegex = useDebounce(regexPattern, 500)

  const { mutate: fetchPreview } = useEffectMutation(
    (pattern: string, positive: boolean) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.previewFilter({
          regex_pattern: pattern,
          is_positive: positive,
        })
      }),
  )

  useEffect(() => {
    if (!debouncedRegex) {
      setPreview(null)
      return
    }
    fetchPreview(debouncedRegex, isPositive)
      .then(setPreview)
      .catch(() => setPreview(null))
  }, [debouncedRegex, isPositive])

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
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("filters.title")}</h1>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          {t("filters.addRule")}
        </Button>
      </div>

      {/* Existing Rules Table */}
      {rulesLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <DataTable
          columns={ruleColumns}
          data={(rules ?? []) as unknown as Record<string, unknown>[]}
          keyField="rule_id"
        />
      )}

      {/* Preview Section */}
      <Card>
        <CardHeader>
          <CardTitle className="text-sm">{t("filters.previewFilter")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <Label>{t("filters.regexPattern")}</Label>
              <RegexInput
                value={regexPattern}
                onChange={setRegexPattern}
                placeholder="e.g. 1080p"
              />
            </div>
            <div className="flex items-center gap-2 pt-6">
              <Switch checked={isPositive} onCheckedChange={setIsPositive} />
              <Label>{isPositive ? t("common.include") : t("common.exclude")}</Label>
            </div>
          </div>
        </CardContent>
      </Card>

      {preview && (
        <div className="grid grid-cols-2 gap-4">
          <PreviewPanel
            title={t("filters.before")}
            passed={preview.before.passed_items}
            filtered={preview.before.filtered_items}
            passedLabel={t("filters.passed")}
            filteredLabel={t("filters.filtered")}
          />
          <PreviewPanel
            title={t("filters.after")}
            passed={preview.after.passed_items}
            filtered={preview.after.filtered_items}
            passedLabel={t("filters.passed")}
            filteredLabel={t("filters.filtered")}
            highlightDiff
          />
        </div>
      )}

      {/* Create Dialog */}
      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("filters.addFilterRule")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div>
              <Label>{t("filters.regexPattern")}</Label>
              <Input
                className="font-mono"
                value={newRegex}
                onChange={(e) => setNewRegex(e.target.value)}
                placeholder="e.g. 1080p"
              />
            </div>
            <div>
              <Label>{t("filters.ruleOrder")}</Label>
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
                  setCreateOpen(false)
                  refetch()
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
        title={t("filters.deleteRule")}
        description={t("filters.deleteRuleConfirm")}
        loading={deleting}
        onConfirm={() => {
          if (deleteTarget !== null) {
            deleteRule(deleteTarget).then(() => {
              setDeleteTarget(null)
              refetch()
            })
          }
        }}
      />
    </div>
  )
}

function PreviewPanel({
  title,
  passed,
  filtered,
  passedLabel,
  filteredLabel,
  highlightDiff,
}: {
  title: string
  passed: readonly { readonly item_id: number; readonly title: string }[]
  filtered: readonly { readonly item_id: number; readonly title: string }[]
  passedLabel: string
  filteredLabel: string
  highlightDiff?: boolean
}) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm">{title}</CardTitle>
        <div className="flex gap-4 text-xs">
          <span className="text-green-600">{passedLabel}: {passed.length}</span>
          <span className="text-red-600">{filteredLabel}: {filtered.length}</span>
        </div>
      </CardHeader>
      <CardContent>
        <ScrollArea className="h-80">
          <div className="space-y-1">
            {passed.map((item) => (
              <div
                key={item.item_id}
                className="text-xs px-2 py-1 rounded font-mono bg-green-50 text-green-800"
              >
                {item.title}
              </div>
            ))}
            {filtered.map((item) => (
              <div
                key={item.item_id}
                className={cn(
                  "text-xs px-2 py-1 rounded font-mono",
                  highlightDiff
                    ? "bg-red-50 text-red-600"
                    : "bg-gray-50 text-gray-500",
                )}
              >
                {item.title}
              </div>
            ))}
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  )
}
