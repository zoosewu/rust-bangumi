import { useState, useRef, useEffect, useCallback } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { WizardPendingList } from "@/components/shared/WizardPendingList"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { CheckCircle2, Loader2 } from "lucide-react"
import type { RawAnimeItem } from "@/schemas/download"
import type { PendingAiResult } from "@/schemas/ai"

interface CreateSubscriptionWizardProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onCreated?: () => void
}

type WizardStep = 1 | 2 | 3

export function CreateSubscriptionWizard({
  open,
  onOpenChange,
  onCreated,
}: CreateSubscriptionWizardProps) {
  const [step, setStep] = useState<WizardStep>(1)
  const [url, setUrl] = useState("")
  const [name, setName] = useState("")
  const [interval, setIntervalVal] = useState("30")
  const [fetcherId, setFetcherId] = useState<number | undefined>(undefined)
  const [subscriptionId, setSubscriptionId] = useState<number | undefined>(undefined)

  // Step 2 state
  const [rawItems, setRawItems] = useState<RawAnimeItem[]>([])
  const [parserPendings, setParserPendings] = useState<PendingAiResult[]>([])
  const [step2Polling, setStep2Polling] = useState(false)
  const step2PollRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Step 3 state
  const [filterPendings, setFilterPendings] = useState<PendingAiResult[]>([])
  const [step3Polling, setStep3Polling] = useState(false)
  const step3PollRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const { data: fetcherModules } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getFetcherModules),
    [],
  )

  const stopStep2Polling = useCallback(() => {
    if (step2PollRef.current) {
      clearTimeout(step2PollRef.current)
      step2PollRef.current = null
    }
  }, [])

  const stopStep3Polling = useCallback(() => {
    if (step3PollRef.current) {
      clearTimeout(step3PollRef.current)
      step3PollRef.current = null
    }
  }, [])

  const pollStep2 = useCallback(
    async (subId: number) => {
      try {
        const [items, pendings] = await Promise.all([
          AppRuntime.runPromise(
            Effect.flatMap(CoreApi, (api) =>
              api.getRawItems({ subscription_id: subId, limit: 200 }),
            ),
          ),
          AppRuntime.runPromise(
            Effect.flatMap(CoreApi, (api) =>
              api.getPendingAiResults({ subscription_id: subId, result_type: "parser" }),
            ),
          ),
        ])

        setRawItems(items as RawAnimeItem[])
        setParserPendings(pendings as PendingAiResult[])

        const allSettled = (items as RawAnimeItem[]).every((item) => item.status !== "pending")
        if (!allSettled) {
          step2PollRef.current = setTimeout(() => pollStep2(subId), 1000)
        } else {
          setStep2Polling(false)
        }
      } catch {
        setStep2Polling(false)
      }
    },
    [stopStep2Polling],
  )

  const pollStep3 = useCallback(
    async (subId: number) => {
      try {
        const pendings = await AppRuntime.runPromise(
          Effect.flatMap(CoreApi, (api) =>
            api.getPendingAiResults({ subscription_id: subId, result_type: "filter" }),
          ),
        )

        setFilterPendings(pendings as PendingAiResult[])

        const hasGenerating = (pendings as PendingAiResult[]).some(
          (p) => p.status === "generating",
        )
        if (hasGenerating) {
          step3PollRef.current = setTimeout(() => pollStep3(subId), 1000)
        } else {
          setStep3Polling(false)
        }
      } catch {
        setStep3Polling(false)
      }
    },
    [stopStep3Polling],
  )

  // Start step 2 polling when entering step 2
  useEffect(() => {
    if (step === 2 && subscriptionId !== undefined) {
      setStep2Polling(true)
      pollStep2(subscriptionId)
      return () => stopStep2Polling()
    }
  }, [step, subscriptionId, pollStep2, stopStep2Polling])

  // Start step 3 polling when entering step 3
  useEffect(() => {
    if (step === 3 && subscriptionId !== undefined) {
      setStep3Polling(true)
      pollStep3(subscriptionId)
      return () => stopStep3Polling()
    }
  }, [step, subscriptionId, pollStep3, stopStep3Polling])

  const reset = () => {
    stopStep2Polling()
    stopStep3Polling()
    setStep(1)
    setUrl("")
    setName("")
    setIntervalVal("30")
    setFetcherId(undefined)
    setSubscriptionId(undefined)
    setRawItems([])
    setParserPendings([])
    setFilterPendings([])
    setStep2Polling(false)
    setStep3Polling(false)
  }

  const { mutate: createSub, isLoading: creating } = useEffectMutation(() =>
    Effect.flatMap(CoreApi, (api) =>
      api.createSubscription({
        source_url: url.trim(),
        name: name.trim() || undefined,
        fetch_interval_minutes: interval === "" ? 30 : parseInt(interval),
        fetcher_id: fetcherId,
      }),
    ),
  )

  const handleCreate = () => {
    createSub().then((sub) => {
      if (sub) {
        setSubscriptionId(sub.subscription_id)
        setStep(2)
      }
    })
  }

  // Step 2 computed values
  const total = rawItems.length
  const parsed = rawItems.filter((i) => i.status === "matched" || i.status === "completed" || i.status === "downloaded").length
  const failed = rawItems.filter((i) => i.status === "unmatched" || i.status === "failed").length
  const allParserSettled =
    parserPendings.length === 0 ||
    parserPendings.every((p) => p.status === "confirmed" || p.status === "rejected" || p.status === "failed")
  const step2NextEnabled = !step2Polling && allParserSettled

  // Step 3 computed values
  const allFilterSettled =
    filterPendings.length === 0 ||
    filterPendings.every((p) => p.status === "confirmed" || p.status === "rejected" || p.status === "failed")
  const step3DoneEnabled = !step3Polling && allFilterSettled

  const stepLabels: Record<WizardStep, string> = {
    1: "基本設定",
    2: "解析確認",
    3: "衝突過濾",
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v) reset()
        onOpenChange(v)
      }}
    >
      <DialogContent className="sm:max-w-xl max-h-[80vh] flex flex-col overflow-hidden">
        <DialogHeader>
          <DialogTitle>新增訂閱 — {stepLabels[step]}</DialogTitle>
        </DialogHeader>

        {/* Step indicator */}
        <div className="flex gap-2 mb-2">
          {([1, 2, 3] as WizardStep[]).map((s) => (
            <div
              key={s}
              className={`flex-1 h-1 rounded-full transition-colors ${s <= step ? "bg-primary" : "bg-muted"}`}
            />
          ))}
        </div>

        <div className="flex-1 overflow-y-auto min-h-0">
          {/* Step 1: 基本設定 */}
          {step === 1 && (
            <div className="space-y-4 py-1 pr-1">
              <div className="space-y-2">
                <Label>RSS URL *</Label>
                <Input
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  placeholder="https://mikanani.me/RSS/..."
                />
              </div>
              <div className="space-y-2">
                <Label>名稱</Label>
                <Input value={name} onChange={(e) => setName(e.target.value)} />
              </div>
              <div className="space-y-2">
                <Label>抓取間隔（分鐘）</Label>
                <Input
                  type="number"
                  min="0"
                  value={interval}
                  onChange={(e) => setIntervalVal(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label>Fetcher</Label>
                <Select
                  value={fetcherId !== undefined ? String(fetcherId) : "auto"}
                  onValueChange={(v) =>
                    setFetcherId(v === "auto" ? undefined : Number(v))
                  }
                >
                  <SelectTrigger>
                    <SelectValue placeholder="自動選擇" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="auto">自動選擇</SelectItem>
                    {(fetcherModules ?? []).map((m) => (
                      <SelectItem key={m.module_id} value={String(m.module_id)}>
                        {m.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
          )}

          {/* Step 2: 解析確認 */}
          {step === 2 && (
            <div className="space-y-4 py-1 pr-1">
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                {step2Polling && <Loader2 className="size-4 animate-spin" />}
                <span>
                  共 {total} 項、{parsed} 已解析、{failed} 解析失敗
                </span>
              </div>

              {!step2Polling && parserPendings.length === 0 ? (
                <div className="flex items-center gap-2 text-green-600 py-4">
                  <CheckCircle2 className="size-5" />
                  <span className="text-sm font-medium">所有項目解析成功</span>
                </div>
              ) : (
                <WizardPendingList
                  results={parserPendings}
                  onAnyChange={() => {
                    if (subscriptionId !== undefined) {
                      AppRuntime.runPromise(
                        Effect.flatMap(CoreApi, (api) =>
                          api.getPendingAiResults({
                            subscription_id: subscriptionId,
                            result_type: "parser",
                          }),
                        ),
                      ).then((pendings) => setParserPendings(pendings as PendingAiResult[]))
                    }
                  }}
                />
              )}
            </div>
          )}

          {/* Step 3: Conflict Filter */}
          {step === 3 && (
            <div className="space-y-4 py-1 pr-1">
              {step3Polling && (
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Loader2 className="size-4 animate-spin" />
                  <span>正在偵測衝突...</span>
                </div>
              )}

              {!step3Polling && filterPendings.length === 0 ? (
                <div className="flex items-center gap-2 text-green-600 py-4">
                  <CheckCircle2 className="size-5" />
                  <span className="text-sm font-medium">無衝突</span>
                </div>
              ) : (
                <WizardPendingList
                  results={filterPendings}
                  onAnyChange={() => {
                    if (subscriptionId !== undefined) {
                      AppRuntime.runPromise(
                        Effect.flatMap(CoreApi, (api) =>
                          api.getPendingAiResults({
                            subscription_id: subscriptionId,
                            result_type: "filter",
                          }),
                        ),
                      ).then((pendings) => setFilterPendings(pendings as PendingAiResult[]))
                    }
                  }}
                />
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <DialogFooter className="pt-2 border-t">
          {step === 1 && (
            <>
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                取消
              </Button>
              <Button onClick={handleCreate} disabled={!url.trim() || creating}>
                {creating && <Loader2 className="mr-1 size-4 animate-spin" />}
                建立訂閱
              </Button>
            </>
          )}

          {step === 2 && (
            <Button onClick={() => setStep(3)} disabled={!step2NextEnabled}>
              下一步
            </Button>
          )}

          {step === 3 && (
            <Button
              onClick={() => {
                onCreated?.()
                onOpenChange(false)
                reset()
              }}
              disabled={!step3DoneEnabled}
            >
              完成
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
