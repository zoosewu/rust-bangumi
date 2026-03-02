import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { toast } from "sonner"
import type { ServiceModule } from "@/schemas/service-module"

export function DownloaderPrioritySection() {
  const { t } = useTranslation()
  const { data: modules, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getDownloaderModules
      }),
    [],
  )

  const { mutate: doUpdate } = useEffectMutation(
    ({ id, priority }: { id: number; priority: number }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.updateServiceModule(id, { priority })
      }),
  )

  const [drafts, setDrafts] = useState<Record<number, number>>({})

  const handleSave = (module: ServiceModule) => {
    const priority = drafts[module.module_id] ?? module.priority
    doUpdate({ id: module.module_id, priority }).then(() => {
      toast.success(t("common.saved", "Saved"))
      refetch()
    })
  }

  if (!modules || modules.length === 0) return null

  return (
    <div className="space-y-2">
      <h2 className="text-sm font-semibold text-muted-foreground">{t("dashboard.downloaderPriority")}</h2>
      <p className="text-xs text-muted-foreground">{t("dashboard.downloaderPriorityDesc")}</p>
      {modules.map((m) => (
        <div key={m.module_id} className="flex items-center gap-3">
          <span className="text-sm flex-1">{m.name}</span>
          <Input
            type="number"
            className="w-20 h-8 text-sm"
            defaultValue={m.priority}
            onChange={(e) =>
              setDrafts((d) => ({ ...d, [m.module_id]: Number(e.target.value) }))
            }
          />
          <Button size="sm" variant="outline" onClick={() => handleSave(m)}>
            {t("common.save")}
          </Button>
        </div>
      ))}
    </div>
  )
}
