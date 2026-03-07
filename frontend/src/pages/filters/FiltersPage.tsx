import { useState, useCallback, useMemo } from "react"
import { useTranslation } from "react-i18next"
import { Link } from "react-router-dom"
import { Effect } from "effect"
import { Plus } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { FilterAddForm } from "@/components/shared/FilterAddForm"
import { DeleteFilterRuleDialog } from "@/components/shared/DeleteFilterRuleDialog"
import { PageHeader } from "@/components/shared/PageHeader"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { CoreApi } from "@/services/CoreApi"
import type { FilterRule } from "@/schemas/filter"
import { SearchBar } from "@/components/shared/SearchBar"
import { useTableSearch } from "@/hooks/useTableSearch"

const TARGET_ROUTES: Record<string, string> = {
  anime_work: "/anime-works",
  anime: "/anime",
  subtitle_group: "/subtitle-groups",
  fetcher: "/subscriptions",
}

export default function FiltersPage() {
  const { t } = useTranslation()
  const [addOpen, setAddOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")
  const [deleteTarget, setDeleteTarget] = useState<FilterRule | null>(null)

  const { data: rules, isLoading, error, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getFilterRules()),
    [],
  )

  if (error) {
    console.error("[FiltersPage] getFilterRules failed:", error)
  }

  // global 排最前，其次按 target_type、再按 rule_order
  const sortedRules = useMemo(() => {
    if (!rules) return []
    return [...rules].sort((a, b) => {
      if (a.target_type === "global" && b.target_type !== "global") return -1
      if (a.target_type !== "global" && b.target_type === "global") return 1
      if (a.target_type !== b.target_type) return a.target_type.localeCompare(b.target_type)
      return a.rule_order - b.rule_order
    })
  }, [rules])

  const filteredRules = useTableSearch(sortedRules, searchQuery)

  const { mutate: deleteRule, isLoading: deleting } = useEffectMutation(
    (ruleId: number) => Effect.flatMap(CoreApi, (api) => api.deleteFilterRule(ruleId)),
  )

  const handleDeleteClick = useCallback((rule: FilterRule) => {
    setDeleteTarget(rule)
  }, [])

  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteTarget) return
    await deleteRule(deleteTarget.rule_id)
    setDeleteTarget(null)
    refetch()
  }, [deleteTarget, deleteRule, refetch])

  const handleAddSuccess = useCallback(() => {
    setAddOpen(false)
    refetch()
  }, [refetch])

  const columns: Column<Record<string, unknown>>[] = [
    {
      key: "regex_pattern",
      header: t("filters.regexPattern"),
      render: (item) => (
        <code className="font-mono text-xs">{String(item.regex_pattern)}</code>
      ),
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
      key: "target_type",
      header: t("parsers.entity"),
      render: (item) => {
        const rule = item as unknown as FilterRule
        if (rule.target_type === "global") {
          return <span className="text-muted-foreground text-xs">Global</span>
        }
        const route = TARGET_ROUTES[rule.target_type]
        const label = rule.target_name ?? `#${rule.target_id}`
        if (route) {
          return (
            <Link
              to={route}
              className="text-xs text-primary underline-offset-2 hover:underline"
              onClick={(e) => e.stopPropagation()}
            >
              {label}
            </Link>
          )
        }
        return <span className="text-xs">{label}</span>
      },
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
      <PageHeader
        title={t("filters.title")}
        actions={
          <Button onClick={() => setAddOpen(true)}>
            <Plus className="h-4 w-4 mr-2" />
            {t("filters.addFilter")}
          </Button>
        }
      />

      <SearchBar value={searchQuery} onChange={setSearchQuery} />

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : error ? (
        <p className="text-destructive text-sm">
          {t("common.error")}: {String(error)}
        </p>
      ) : filteredRules.length === 0 ? (
        <p className="text-sm text-muted-foreground">{searchQuery ? t("common.noResults") : t("filters.noRules", "No filter rules found.")}</p>
      ) : (
        <DataTable
          columns={columns}
          data={filteredRules as unknown as Record<string, unknown>[]}
          keyField="rule_id"
        />
      )}

      {/* Add — FullScreenDialog with preview */}
      <FullScreenDialog
        open={addOpen}
        onOpenChange={setAddOpen}
        title={t("filters.addFilter")}
      >
        <FilterAddForm
          targetType="global"
          targetId={null}
          currentRuleCount={rules?.filter((r) => r.target_type === "global").length ?? 0}
          onSuccess={handleAddSuccess}
        />
      </FullScreenDialog>

      {/* Delete Confirm */}
      <DeleteFilterRuleDialog
        open={!!deleteTarget}
        onOpenChange={(open) => { if (!open) setDeleteTarget(null) }}
        rule={deleteTarget ? { is_positive: deleteTarget.is_positive, regex_pattern: deleteTarget.regex_pattern, id: deleteTarget.rule_id } : null}
        targetType={deleteTarget?.target_type ?? "global"}
        targetId={deleteTarget?.target_id ?? null}
        onConfirm={handleDeleteConfirm}
        loading={deleting}
      />
    </div>
  )
}
