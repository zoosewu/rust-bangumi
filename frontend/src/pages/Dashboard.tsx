import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"
import { ParserEditor } from "@/components/shared/ParserEditor"
import {
  Film,
  Rss,
  Download,
  AlertTriangle,
  FileText,
  CheckCircle2,
  XCircle,
} from "lucide-react"

export default function Dashboard() {
  const { t } = useTranslation()

  const { data: stats, error, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getDashboardStats
      }),
    [],
  )

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">{t("dashboard.title")}</h1>

      {/* Service health */}
      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : error ? (
        <div className="flex items-center gap-2 rounded-md border border-destructive bg-destructive/10 px-4 py-3">
          <XCircle className="h-5 w-5 text-destructive shrink-0" />
          <p className="text-sm text-destructive">{t("dashboard.coreUnavailable", "Core service is not responding.")}</p>
        </div>
      ) : stats ? (
        <>
          {/* Services */}
          {stats.services.length > 0 && (
            <div className="space-y-2">
              <h2 className="text-sm font-semibold text-muted-foreground">{t("dashboard.services", "Services")}</h2>
              <div className="flex flex-wrap gap-2">
                {stats.services.map((svc) => (
                  <Badge
                    key={svc.name}
                    variant={svc.is_healthy ? "default" : "destructive"}
                    className="gap-1"
                  >
                    {svc.is_healthy ? (
                      <CheckCircle2 className="h-3 w-3" />
                    ) : (
                      <XCircle className="h-3 w-3" />
                    )}
                    {svc.name} ({svc.module_type})
                  </Badge>
                ))}
              </div>
            </div>
          )}

          {/* Stats cards */}
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-2">
            <StatCard icon={Film} label={t("dashboard.totalAnime", "Anime")} value={stats.total_anime} />
            <StatCard icon={Film} label={t("dashboard.totalSeries", "Seasons")} value={stats.total_series} />
            <StatCard icon={Rss} label={t("dashboard.activeSubs", "Subscriptions")} value={stats.active_subscriptions} />
            <StatCard icon={Download} label={t("dashboard.downloading", "Downloading")} value={stats.downloading} color="blue" />
            <StatCard icon={CheckCircle2} label={t("dashboard.completed", "Completed")} value={stats.completed} color="green" />
            <StatCard icon={XCircle} label={t("dashboard.failed", "Failed")} value={stats.failed} color="red" />
            <StatCard icon={FileText} label={t("dashboard.pendingRawItems", "Pending Items")} value={stats.pending_raw_items} />
            <StatCard icon={AlertTriangle} label={t("dashboard.pendingConflicts", "Conflicts")} value={stats.pending_conflicts} color={stats.pending_conflicts > 0 ? "yellow" : undefined} />
          </div>
        </>
      ) : null}

      {/* Global filters and parsers */}
      <Tabs defaultValue="filters">
        <TabsList variant="line">
          <TabsTrigger value="filters">{t("dashboard.globalFilterRules", "Global Filter Rules")}</TabsTrigger>
          <TabsTrigger value="parsers">{t("dashboard.globalParsers", "Global Parsers")}</TabsTrigger>
        </TabsList>
        <TabsContent value="filters" className="mt-4">
          <FilterRuleEditor targetType="global" targetId={null} />
        </TabsContent>
        <TabsContent value="parsers" className="mt-4">
          <ParserEditor createdFromType="global" createdFromId={null} />
        </TabsContent>
      </Tabs>
    </div>
  )
}

function StatCard({
  icon: Icon,
  label,
  value,
  color,
}: {
  icon: React.ElementType
  label: string
  value: number
  color?: "blue" | "green" | "red" | "yellow"
}) {
  const colorClasses = {
    blue: "text-blue-600 dark:text-blue-400",
    green: "text-green-600 dark:text-green-400",
    red: "text-red-600 dark:text-red-400",
    yellow: "text-yellow-600 dark:text-yellow-400",
  }

  return (
    <div className="flex items-center justify-between rounded-md border px-3 py-2">
      <span className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
        <Icon className="h-3.5 w-3.5" />
        {label}
      </span>
      <span className={`text-lg font-bold tabular-nums ${color ? colorClasses[color] : ""}`}>
        {value}
      </span>
    </div>
  )
}
