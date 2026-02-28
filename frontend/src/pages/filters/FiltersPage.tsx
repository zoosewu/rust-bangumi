import { useState, useCallback, useMemo } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { Plus } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { FilterAddForm } from "@/components/shared/FilterAddForm"
import { FilterPreviewPanel } from "@/components/shared/FilterPreviewPanel"
import { ConfirmDialog } from "@/components/shared/ConfirmDialog"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import type { FilterRule, FilterPreviewResponse } from "@/schemas/filter"

const TARGET_TYPE_KEYS: Record<string, string> = {
  anime_work: "sidebar.anime",
  anime: "sidebar.animeSeries",
  subtitle_group: "sidebar.subtitleGroups",
  fetcher: "sidebar.subscriptions",
}

function formatTarget(rule: FilterRule, t: (k: string) => string): string {
  const key = TARGET_TYPE_KEYS[rule.target_type]
  const label = key ? t(key) : rule.target_type
  return `${label} #${rule.target_id}`
}

export default function FiltersPage() {
  const { t } = useTranslation()
  const [addOpen, setAddOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<FilterRule | null>(null)
  const [deletePreview, setDeletePreview] = useState<FilterPreviewResponse | null>(null)

  const { data: rules, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getFilterRules()),
    [],
  )

  // Sort: global first, then by target_type, then by rule_order
  const sortedRules = useMemo(() => {
    if (!rules) return []
    return [...rules].sort((a, b) => {
      if (a.target_type === "global" && b.target_type !== "global") return -1
      if (a.target_type !== "global" && b.target_type === "global") return 1
      if (a.target_type !== b.target_type) return a.target_type.localeCompare(b.target_type)
      return a.rule_order - b.rule_order
    })
  }, [rules])

  const { mutate: deleteRule, isLoading: deleting } = useEffectMutation(
    (ruleId: number) => Effect.flatMap(CoreApi, (api) => api.deleteFilterRule(ruleId)),
  )

  const handleDeleteClick = useCallback((rule: FilterRule) => {
    setDeleteTarget(rule)
    AppRuntime.runPromise(
      Effect.flatMap(CoreApi, (api) =>
        api.previewFilter({
          target_type: rule.target_type,
          target_id: rule.target_id,
          regex_pattern: rule.regex_pattern,
          is_positive: rule.is_positive,
          exclude_filter_id: rule.rule_id,
        }),
      ),
    ).then(setDeletePreview).catch(() => setDeletePreview(null))
  }, [])

  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteTarget) return
    await deleteRule(deleteTarget.rule_id)
    setDeleteTarget(null)
    setDeletePreview(null)
    refetch()
  }, [deleteTarget, deleteRule, refetch])

  const handleAddSuccess = useCallback(() => {
    setAddOpen(false)
    refetch()
  }, [refetch])

  const columns: Column<Record<string, unknown>>[] = [
    {
      key: "target_type",
      header: t("parsers.entity"),
      render: (item) => {
        const rule = item as unknown as FilterRule
        if (rule.target_type === "global") {
          return <span className="text-muted-foreground text-xs">Global</span>
        }
        return <span className="text-xs">{formatTarget(rule, t)}</span>
      },
    },
    {
      key: "is_positive",
      header: t("common.type"),
      render: (item) => (
        <Badge variant={(item.is_positive as boolean) ? "default" : "destructive"}>
          {(item.is_positive as boolean) ? "include" : "exclude"}
        </Badge>
      ),
    },
    {
      key: "regex_pattern",
      header: t("filters.regexPattern"),
      render: (item) => (
        <code className="font-mono text-xs">{String(item.regex_pattern)}</code>
      ),
    },
    {
      key: "rule_order",
      header: t("filters.ruleOrder"),
      render: (item) => String(item.rule_order),
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
            handleDeleteClick(item as unknown as FilterRule)
          }}
        >
          {t("common.delete")}
        </Button>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("filters.title")}</h1>
        <Button onClick={() => setAddOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          {t("filters.addFilter")}
        </Button>
      </div>

      <DataTable
        columns={columns}
        data={sortedRules as unknown as Record<string, unknown>[]}
        keyField="rule_id"
      />

      {/* Add Dialog */}
      <Dialog open={addOpen} onOpenChange={setAddOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>{t("filters.addFilter")}</DialogTitle>
          </DialogHeader>
          <FilterAddForm
            targetType="global"
            targetId={null}
            currentRuleCount={rules?.filter((r) => r.target_type === "global").length ?? 0}
            onSuccess={handleAddSuccess}
          />
        </DialogContent>
      </Dialog>

      {/* Delete Confirm */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteTarget(null)
            setDeletePreview(null)
          }
        }}
        title={t("filter.confirmRemove")}
        description={
          deleteTarget
            ? `${deleteTarget.is_positive ? "Include" : "Exclude"}: ${deleteTarget.regex_pattern}`
            : ""
        }
        onConfirm={handleDeleteConfirm}
        loading={deleting}
      >
        {deletePreview?.regex_valid && (
          <FilterPreviewPanel
            before={deletePreview.after}
            after={deletePreview.before}
          />
        )}
      </ConfirmDialog>
    </div>
  )
}
