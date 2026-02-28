import { useTranslation } from "react-i18next"
import { Link } from "react-router-dom"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { Card, CardContent } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { AlertTriangle } from "lucide-react"

export default function ConflictsPage() {
  const { t } = useTranslation()

  const { data: conflicts, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getConflictingLinks
      }),
    [],
  )

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <h1 className="text-2xl font-bold">{t("conflicts.title")}</h1>
        {conflicts && conflicts.length > 0 && (
          <Badge variant="destructive">{conflicts.length}</Badge>
        )}
      </div>

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : !conflicts?.length ? (
        <Card>
          <CardContent className="py-8 text-center text-muted-foreground">
            <AlertTriangle className="h-8 w-8 mx-auto mb-2 opacity-30" />
            {t("conflicts.noConflicts")}
          </CardContent>
        </Card>
      ) : (
        <div className="rounded-md border">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b bg-muted/50">
                <th className="px-4 py-3 text-left font-medium text-muted-foreground">
                  {t("conflicts.animeTitle")}
                </th>
                <th className="px-4 py-3 text-left font-medium text-muted-foreground">
                  {t("conflicts.seriesTitle")}
                </th>
                <th className="px-4 py-3 text-left font-medium text-muted-foreground">
                  {t("conflicts.subscription")}
                </th>
                <th className="px-4 py-3 text-center font-medium text-muted-foreground w-20">
                  {t("conflicts.episode")}
                </th>
                <th className="px-4 py-3 text-left font-medium text-muted-foreground">
                  {t("conflicts.subtitleGroup")}
                </th>
                <th className="px-4 py-3 text-center font-medium text-muted-foreground w-24">
                  {t("conflicts.conflictCount")}
                </th>
              </tr>
            </thead>
            <tbody className="divide-y">
              {conflicts.map((c) => (
                <tr key={c.link_id} className="hover:bg-muted/30 transition-colors">
                  <td className="px-4 py-3">
                    <Link
                      to="/anime-works"
                      className="text-primary underline-offset-2 hover:underline font-medium"
                    >
                      {c.anime_work_title}
                    </Link>
                  </td>
                  <td className="px-4 py-3">
                    <Link
                      to="/anime"
                      className="text-primary underline-offset-2 hover:underline"
                    >
                      {c.anime_work_title} S{c.series_no}
                    </Link>
                  </td>
                  <td className="px-4 py-3">
                    {c.subscription_id ? (
                      <Link
                        to="/subscriptions"
                        className="text-primary underline-offset-2 hover:underline"
                      >
                        {c.subscription_name ?? `#${c.subscription_id}`}
                      </Link>
                    ) : (
                      <span className="text-muted-foreground text-xs">—</span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-center">
                    Ep.{c.episode_no}
                  </td>
                  <td className="px-4 py-3 text-muted-foreground">
                    {c.group_name}
                  </td>
                  <td className="px-4 py-3 text-center">
                    <Badge variant="outline" className="text-amber-600 border-amber-300">
                      {c.conflicting_link_ids.length + 1}
                    </Badge>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
