import { Effect } from "effect"
import { Plus } from "lucide-react"
import { useState } from "react"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import type {
  AiProvider,
  CreateAiProviderRequest,
  TestAiProviderResult,
  UpdateAiProviderRequest,
} from "@/schemas/ai"
import { CoreApi } from "@/services/CoreApi"
import { AiProviderEditDialog } from "./AiProviderEditDialog"
import { AiProviderList } from "./AiProviderList"

export function AiProvidersSection() {
  const { data: providers, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.listAiProviders),
    [],
  )

  // undefined = closed; null = creating; AiProvider = editing existing
  const [editing, setEditing] = useState<AiProvider | null | undefined>(undefined)

  const { mutate: doCreate } = useEffectMutation((req: CreateAiProviderRequest) =>
    Effect.flatMap(CoreApi, (api) => api.createAiProvider(req)),
  )
  const { mutate: doUpdate } = useEffectMutation(
    ({ id, req }: { id: number; req: UpdateAiProviderRequest }) =>
      Effect.flatMap(CoreApi, (api) => api.updateAiProvider(id, req)),
  )
  const { mutate: doDelete } = useEffectMutation((id: number) =>
    Effect.flatMap(CoreApi, (api) => api.deleteAiProvider(id)),
  )
  const { mutate: doReorder } = useEffectMutation((ordered_ids: readonly number[]) =>
    Effect.flatMap(CoreApi, (api) => api.reorderAiProviders(ordered_ids)),
  )
  const { mutate: doTest } = useEffectMutation((id: number) =>
    Effect.flatMap(CoreApi, (api) => api.testAiProvider(id)),
  )

  const list = providers ?? []

  const handleTest = async (id: number): Promise<TestAiProviderResult> => {
    const r = await doTest(id)
    return r ?? { ok: false, error: "no response" }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>AI Providers</CardTitle>
        <CardDescription>
          按優先順序由上至下嘗試；遇到 provider 端故障（5xx、timeout、rate limit）會 fallback 到下一個。
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {list.length === 0 ? (
          <div className="rounded border border-dashed p-6 text-center text-sm text-muted-foreground">
            尚無 provider，請新增第一個。
          </div>
        ) : (
          <AiProviderList
            providers={list}
            onReorder={async (ids) => {
              await doReorder(ids)
              await refetch()
            }}
            onEdit={(p) => setEditing(p)}
            onDelete={async (id) => {
              await doDelete(id)
              await refetch()
            }}
            onToggle={async (id, is_enabled) => {
              await doUpdate({ id, req: { is_enabled } })
              await refetch()
            }}
            onTest={handleTest}
          />
        )}

        <Button size="sm" onClick={() => setEditing(null)}>
          <Plus className="mr-1 size-3" /> 新增 Provider
        </Button>

        {editing !== undefined && (
          <AiProviderEditDialog
            provider={editing}
            onClose={() => setEditing(undefined)}
            onSubmit={async (req) => {
              if (editing) {
                await doUpdate({ id: editing.id, req })
              } else {
                await doCreate(req as CreateAiProviderRequest)
              }
              setEditing(undefined)
              await refetch()
            }}
          />
        )}
      </CardContent>
    </Card>
  )
}
