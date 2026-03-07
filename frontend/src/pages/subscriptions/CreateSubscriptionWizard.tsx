import { useState, useRef, useEffect, useCallback } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { WizardPendingList } from "@/components/shared/WizardPendingList"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
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
  initialUrl?: string
  initialName?: string
  initialInterval?: string
}

type WizardStep = 1 | 2 | 3

export function CreateSubscriptionWizard({
  open,
  onOpenChange,
  onCreated,
  initialUrl = "",
  initialName = "",
  initialInterval = "",
}: CreateSubscriptionWizardProps) {
  const [step, setStep] = useState<WizardStep>(1)
  const [url, setUrl] = useState(initialUrl)
  const [name, setName] = useState(initialName)
  const [interval, setIntervalVal] = useState(initialInterval || "30")
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
  const [detecting, setDetecting] = useState(false)

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

        const rawList = items as RawAnimeItem[]
        const allRawSettled = rawList.length > 0 && rawList.every((item) => item.status !== "pending")
        const hasGeneratingPendings = (pendings as PendingAiResult[]).some(
          (p) => p.status === "generating",
        )
        if (!allRawSettled || hasGeneratingPendings) {
          step2PollRef.current = setTimeout(() => pollStep2(subId), 1000)
        } else {
          setStep2Polling(false)
        }
      } catch {
        setStep2Polling(false)
      }
    },
    [],
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
    [],
  )

  // Re-initialize form from props whenever wizard opens
  useEffect(() => {
    if (open) {
      setStep(1)
      setUrl(initialUrl)
      setName(initialName)
      setIntervalVal(initialInterval || "30")
      setFetcherId(undefined)
      setSubscriptionId(undefined)
      setRawItems([])
      setParserPendings([])
      setFilterPendings([])
      setStep2Polling(false)
      setStep3Polling(false)
    }
  }, [open, initialUrl, initialName, initialInterval])

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

  // Step 2 返回 Step 1：刪除訂閱，保留表單內容
  const goBackToStep1 = async () => {
    stopStep2Polling()
    if (subscriptionId !== undefined) {
      await AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.deleteSubscription(subscriptionId, true)),
      ).catch(() => {})
    }
    setSubscriptionId(undefined)
    setRawItems([])
    setParserPendings([])
    setFilterPendings([])
    setStep2Polling(false)
    setStep(1)
  }

  // Step 2/3 取消：刪除訂閱，關閉 wizard
  const closeAndCleanup = async () => {
    stopStep2Polling()
    stopStep3Polling()
    if (subscriptionId !== undefined) {
      await AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.deleteSubscription(subscriptionId, true)),
      ).catch(() => {})
    }
    reset()
    onOpenChange(false)
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
  const parsed = rawItems.filter((i) => i.status === "parsed" || i.status === "partial").length
  const failed = rawItems.filter((i) => i.status === "no_match" || i.status === "failed").length
  // 過濾掉生成失敗的解析器，只顯示待確認或仍在生成中的項目
  const displayParserPendings = parserPendings.filter((p) => p.status !== "failed")
  const step2NextEnabled = !step2Polling

  // Step 3 computed values
  const allFilterSettled =
    filterPendings.length === 0 ||
    filterPendings.every((p) => p.status === "confirmed" || p.status === "failed")
  const step3DoneEnabled = !step3Polling && allFilterSettled

  const stepLabels: Record<WizardStep, string> = {
    1: "基本設定",
    2: "解析確認",
    3: "衝突過濾",
  }

  const handleClose = (v: boolean) => {
    if (!v) {
      if (subscriptionId !== undefined) {
        AppRuntime.runPromise(
          Effect.flatMap(CoreApi, (api) => api.deleteSubscription(subscriptionId, true)),
        ).catch(() => {})
      }
      reset()
      onOpenChange(false)
    } else {
      onOpenChange(true)
    }
  }

  const stepFooter = (() => {
    if (step === 1) return (
      <>
        <Button variant="outline" onClick={() => onOpenChange(false)}>取消</Button>
        <Button onClick={handleCreate} disabled={!url.trim() || creating}>
          {creating && <Loader2 className="mr-1 size-4 animate-spin" />}
          建立訂閱
        </Button>
      </>
    )
    if (step === 2) return (
      <>
        <Button variant="outline" onClick={closeAndCleanup}>取消</Button>
        <Button variant="outline" onClick={goBackToStep1}>返回</Button>
        <Button
          onClick={async () => {
            if (subscriptionId !== undefined) {
              setDetecting(true)
              await AppRuntime.runPromise(
                Effect.flatMap(CoreApi, (api) => api.detectConflicts(subscriptionId)),
              ).catch(() => {})
              setDetecting(false)
            }
            setStep(3)
          }}
          disabled={!step2NextEnabled || detecting}
        >
          {detecting && <Loader2 className="mr-1 size-4 animate-spin" />}
          下一步
        </Button>
      </>
    )
    return (
      <>
        <Button variant="outline" onClick={closeAndCleanup}>取消</Button>
        <Button variant="outline" onClick={() => { stopStep3Polling(); setStep(2) }}>返回</Button>
        <Button
          onClick={() => { onCreated?.(); onOpenChange(false); reset() }}
          disabled={!step3DoneEnabled}
        >
          完成
        </Button>
      </>
    )
  })()

  return (
    <FullScreenDialog
      open={open}
      onOpenChange={handleClose}
      title={`新增訂閱 — ${stepLabels[step]}`}
      size="md"
      subHeader={
        <div className="flex gap-2">
          {([1, 2, 3] as WizardStep[]).map((s) => (
            <div
              key={s}
              className={`flex-1 h-1 rounded-full transition-colors ${s <= step ? "bg-primary" : "bg-muted"}`}
            />
          ))}
        </div>
      }
      footer={stepFooter}
    >
      {/* Step 1: 基本設定 */}
      {step === 1 && (
        <div className="space-y-4">
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
              onValueChange={(v) => setFetcherId(v === "auto" ? undefined : Number(v))}
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
        <div className="space-y-4">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            {step2Polling && <Loader2 className="size-4 animate-spin" />}
            <span>共 {total} 項、{parsed} 已解析、{failed} 解析失敗</span>
          </div>
          {!step2Polling && displayParserPendings.length === 0 ? (
            <div className="flex items-center gap-2 text-green-600 py-4">
              <CheckCircle2 className="size-5" />
              <span className="text-sm font-medium">所有項目解析成功</span>
            </div>
          ) : (
            <WizardPendingList
              results={displayParserPendings}
              onAnyChange={() => {
                if (subscriptionId !== undefined) {
                  AppRuntime.runPromise(
                    Effect.flatMap(CoreApi, (api) =>
                      api.getPendingAiResults({ subscription_id: subscriptionId, result_type: "parser" }),
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
        <div className="space-y-4">
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
                      api.getPendingAiResults({ subscription_id: subscriptionId, result_type: "filter" }),
                    ),
                  ).then((pendings) => setFilterPendings(pendings as PendingAiResult[]))
                }
              }}
            />
          )}
        </div>
      )}
    </FullScreenDialog>
  )
}
