import { useState } from "react"
import { Link } from "react-router-dom"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { toast } from "sonner"

export default function ConflictsPage() {
  const { t } = useTranslation()
  const { data: conflicts, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getConflicts
      }),
    [],
  )

  const { mutate: resolveConflict, isLoading: resolving } = useEffectMutation(
    (args: { conflictId: number; fetcherId: number }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.resolveConflict(args.conflictId, args.fetcherId)
      }),
  )

  const [resolvingId, setResolvingId] = useState<number | null>(null)

  const handleResolve = (conflictId: number, fetcherId: number) => {
    setResolvingId(conflictId)
    resolveConflict({ conflictId, fetcherId })
      .then(() => {
        toast.success(t("conflicts.resolved"))
        refetch()
      })
      .catch(() => {
        toast.error(t("conflicts.resolveFailed"))
      })
      .finally(() => setResolvingId(null))
  }

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">{t("conflicts.title")}</h1>

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : !conflicts?.length ? (
        <Card>
          <CardContent className="py-8 text-center text-muted-foreground">
            {t("conflicts.noConflicts")}
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-4">
          {conflicts.map((conflict) => {
            const id = conflict.conflict_id as number
            const candidates = (conflict.candidate_fetchers ?? []) as {
              fetcher_id: number
              name: string
            }[]
            return (
              <Card key={id}>
                <CardHeader className="pb-2">
                  <CardTitle className="text-sm flex items-center justify-between">
                    <span>{t("conflicts.conflict")} #{id}</span>
                    <span className="text-xs font-normal">
                      {t("conflicts.subscription")}{" "}
                      <Link
                        to="/subscriptions"
                        className="text-primary underline cursor-pointer"
                      >
                        #{String(conflict.subscription_id)}
                      </Link>
                    </span>
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                  <div className="text-sm">
                    <span className="text-muted-foreground">{t("conflicts.rssUrl")}: </span>
                    <code className="text-xs font-mono">
                      {String(conflict.rss_url)}
                    </code>
                  </div>
                  <div className="text-sm">
                    <span className="text-muted-foreground">{t("conflicts.conflictType")}: </span>
                    {String(conflict.conflict_type)}
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground mb-2">
                      {t("conflicts.selectFetcher")}
                    </p>
                    <div className="flex gap-2 flex-wrap">
                      {candidates.map((f) => (
                        <Button
                          key={f.fetcher_id}
                          variant="outline"
                          size="sm"
                          disabled={resolving && resolvingId === id}
                          onClick={() => handleResolve(id, f.fetcher_id)}
                        >
                          {f.name} (#{f.fetcher_id})
                        </Button>
                      ))}
                    </div>
                  </div>
                </CardContent>
              </Card>
            )
          })}
        </div>
      )}
    </div>
  )
}
