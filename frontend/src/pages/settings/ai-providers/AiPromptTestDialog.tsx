import { useState } from "react"
import { Effect } from "effect"
import { Loader2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { TagBadge } from "@/components/shared/TagBadge"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { CoreApi } from "@/services/CoreApi"
import type {
  AiProvider,
  PromptTestKind,
  PromptTestRequest,
  PromptTestResult,
  TitleEvaluation,
} from "@/schemas/ai"

export interface AiPromptTestDialogProps {
  provider: AiProvider
  onClose: () => void
  onRun: (req: PromptTestRequest) => Promise<PromptTestResult>
}

const OUTCOME_LABEL: Record<TitleEvaluation["outcome"], string> = {
  match: "解析成功",
  no_match: "未解析",
  keep: "保留",
  drop: "排除",
}

/** 用設定好的真實 prompt 與所選訂閱的真實標題測試這個 provider（不建立待確認結果） */
export function AiPromptTestDialog({ provider, onClose, onRun }: AiPromptTestDialogProps) {
  const { data: subscriptions } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getSubscriptions),
    [],
  )
  const [kind, setKind] = useState<PromptTestKind>("filter")
  const [subscriptionId, setSubscriptionId] = useState<string>("")
  const [running, setRunning] = useState(false)
  const [result, setResult] = useState<PromptTestResult | null>(null)
  const [error, setError] = useState<string | null>(null)

  const run = async () => {
    setRunning(true)
    setResult(null)
    setError(null)
    try {
      setResult(
        await onRun({
          existing_provider_id: provider.id,
          provider_kind: provider.provider_kind,
          base_url: provider.base_url,
          api_key: "",
          model_name: provider.model_name,
          max_tokens: provider.max_tokens,
          response_format_mode: provider.response_format_mode,
          kind,
          subscription_id: Number(subscriptionId),
        }),
      )
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setRunning(false)
    }
  }

  return (
    <Dialog
      open
      onOpenChange={(o) => {
        if (!o) onClose()
      }}
    >
      <DialogContent className="max-h-[90vh] max-w-3xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle>
            真實 prompt 測試：{provider.name}（{provider.model_name}）
          </DialogTitle>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">
          用設定頁的 prompt 與所選訂閱最早三集的標題，送給這個 provider（不 fallback、不建立待確認結果）。
        </p>
        <div className="flex flex-wrap items-end gap-3">
          <div className="space-y-1">
            <Label>類型</Label>
            <Select value={kind} onValueChange={(v) => setKind(v as PromptTestKind)}>
              <SelectTrigger className="w-36" aria-label="類型">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="filter">過濾規則</SelectItem>
                <SelectItem value="parser">解析器</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label>訂閱</Label>
            <Select value={subscriptionId} onValueChange={setSubscriptionId}>
              <SelectTrigger className="w-72" aria-label="訂閱">
                <SelectValue placeholder="選擇訂閱" />
              </SelectTrigger>
              <SelectContent>
                {(subscriptions ?? []).map((s) => (
                  <SelectItem key={s.subscription_id} value={String(s.subscription_id)}>
                    {s.name ?? `訂閱 ${s.subscription_id}`}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <Button onClick={run} disabled={running || !subscriptionId}>
            {running && <Loader2 className="mr-1 size-3 animate-spin" />}
            執行
          </Button>
        </div>

        {error && (
          <div className="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
            {error}
          </div>
        )}

        {result && (
          <div className="space-y-3">
            <div className="flex items-center gap-2 text-sm">
              <TagBadge tone={result.ok ? "success" : "danger"}>{result.ok ? "成功" : "失敗"}</TagBadge>
              <span className="text-muted-foreground">
                耗時 {(result.elapsed_ms / 1000).toFixed(1)} 秒・送出 {result.titles.length} 個標題・
                prompt 約 {result.system_prompt.length + result.user_prompt.length} 字
              </span>
            </div>
            {result.error && (
              <div className="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive break-all">
                {result.error}
              </div>
            )}
            {result.evaluations.length > 0 && (
              <div className="rounded-md border divide-y">
                {result.evaluations.map((e, i) => (
                  <div key={i} className="flex items-start gap-2 p-2 text-sm">
                    <TagBadge tone={e.outcome === "match" || e.outcome === "keep" ? "success" : "muted"}>
                      {OUTCOME_LABEL[e.outcome]}
                    </TagBadge>
                    <div className="min-w-0">
                      <div className="break-all font-mono text-xs">{e.title}</div>
                      {e.detail && <div className="text-xs text-muted-foreground">{e.detail}</div>}
                    </div>
                  </div>
                ))}
              </div>
            )}
            {result.parsed != null && (
              <details>
                <summary className="cursor-pointer text-sm">AI 產生的規則（JSON）</summary>
                <pre className="mt-1 max-h-64 overflow-auto rounded bg-muted p-2 text-xs">
                  {JSON.stringify(result.parsed, null, 2)}
                </pre>
              </details>
            )}
            <details>
              <summary className="cursor-pointer text-sm">送出的 prompt</summary>
              <pre className="mt-1 max-h-64 overflow-auto whitespace-pre-wrap rounded bg-muted p-2 text-xs">
                {`[system]\n${result.system_prompt}\n\n[user]\n${result.user_prompt}`}
              </pre>
            </details>
            {result.raw_response && (
              <details>
                <summary className="cursor-pointer text-sm">AI 原始回應</summary>
                <pre className="mt-1 max-h-64 overflow-auto whitespace-pre-wrap rounded bg-muted p-2 text-xs">
                  {result.raw_response}
                </pre>
              </details>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
