import { useState } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { toast } from "sonner"
import type { ServiceModule } from "@/schemas/service-module"

export function DownloaderPrioritySection() {
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
      toast.success(`${module.name} 優先級已更新為 ${priority}`)
      refetch()
    })
  }

  if (!modules || modules.length === 0) return null

  return (
    <div className="space-y-2">
      <h3 className="text-sm font-medium">Downloader 優先級</h3>
      <p className="text-xs text-muted-foreground">數字越大優先級越高（預設 50）</p>
      {modules.map((m) => (
        <div key={m.module_id} className="flex items-center gap-3">
          <span className="text-sm flex-1">{m.name}</span>
          <Input
            type="number"
            className="w-20 h-7 text-sm"
            defaultValue={m.priority}
            onChange={(e) =>
              setDrafts((d) => ({ ...d, [m.module_id]: Number(e.target.value) }))
            }
          />
          <Button size="sm" variant="outline" className="h-7" onClick={() => handleSave(m)}>
            儲存
          </Button>
        </div>
      ))}
    </div>
  )
}
