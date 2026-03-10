import { useState, useEffect, useRef } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { Button } from "@/components/ui/button"
import { AutoResizeTextarea } from "@/components/shared/AutoResizeTextarea"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Badge } from "@/components/ui/badge"
import { Loader2, RefreshCw } from "lucide-react"
import type { PendingAiResult, ConfirmPendingRequest } from "@/schemas/ai"
import type { Subscription } from "@/schemas/subscription"
import type { AnimeWork } from "@/schemas/anime"

interface AiResultPanelProps {
  result: PendingAiResult
  onConfirmed?: () => void
  onRejected?: () => void
  onRegenerated?: (updated: PendingAiResult) => void
  children?: React.ReactNode
  previewSlot?: React.ReactNode
  defaultLevel?: "global" | "subscription" | "anime_work"
  defaultTargetId?: number
}

export function AiResultPanel({
  result,
  onConfirmed,
  onRejected,
  onRegenerated,
  children,
  previewSlot,
  defaultLevel = "global",
  defaultTargetId,
}: AiResultPanelProps) {
  const { t } = useTranslation()
  const [fixedPrompt, setFixedPrompt] = useState(result.used_fixed_prompt)
  const [customPrompt, setCustomPrompt] = useState(result.used_custom_prompt ?? "")

  // 套用層級：優先從 DB 讀取，否則使用 prop 預設
  const initLevel =
    (result.confirm_level as "global" | "subscription" | "anime_work" | null) ?? defaultLevel
  const initTargetId = result.confirm_target_id ?? defaultTargetId

  const [level, setLevel] = useState<"global" | "subscription" | "anime_work">(initLevel)
  const [targetId, setTargetId] = useState<number | undefined>(initTargetId)
  const levelSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // 當 result.id 變更（重新生成後換了不同 id）時同步層級設定
  useEffect(() => {
    const newLevel =
      (result.confirm_level as "global" | "subscription" | "anime_work" | null) ?? defaultLevel
    const newTargetId = result.confirm_target_id ?? defaultTargetId
    setLevel(newLevel)
    setTargetId(newTargetId)
  }, [result.id])

  // 下拉選項資料
  const [subscriptions, setSubscriptions] = useState<readonly Subscription[] | null>(null)
  const [animeWorks, setAnimeWorks] = useState<readonly AnimeWork[] | null>(null)

  useEffect(() => {
    if (level === "subscription" && subscriptions === null) {
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.getSubscriptions),
      ).then(setSubscriptions).catch(() => {})
    }
    if (level === "anime_work" && animeWorks === null) {
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.getAnimeWorks),
      ).then(setAnimeWorks).catch(() => {})
    }
  }, [level])

  const { mutate: saveLevelSettings } = useEffectMutation(
    (req: { confirm_level: string | null; confirm_target_id: number | null }) =>
      Effect.flatMap(CoreApi, (api) =>
        api.updatePendingAiResult(result.id, req),
      ),
  )

  const scheduleLevelSave = (
    lv: "global" | "subscription" | "anime_work",
    tid: number | undefined,
  ) => {
    if (levelSaveTimerRef.current) clearTimeout(levelSaveTimerRef.current)
    levelSaveTimerRef.current = setTimeout(() => {
      saveLevelSettings({
        confirm_level: lv,
        confirm_target_id: tid ?? null,
      })
    }, 500)
  }

  const handleLevelChange = (newLevel: "global" | "subscription" | "anime_work") => {
    setLevel(newLevel)
    const newTargetId = newLevel === "global" ? undefined : targetId
    setTargetId(newTargetId)
    scheduleLevelSave(newLevel, newTargetId)
  }

  const handleTargetIdChange = (newId: number | undefined) => {
    setTargetId(newId)
    scheduleLevelSave(level, newId)
  }

  const { mutate: confirm, isLoading: confirming } = useEffectMutation(
    (req: ConfirmPendingRequest) =>
      Effect.flatMap(CoreApi, (api) => api.confirmPendingAiResult(result.id, req)),
  )

  const { mutate: reject, isLoading: rejecting } = useEffectMutation(
    () => Effect.flatMap(CoreApi, (api) => api.rejectPendingAiResult(result.id)),
  )

  const { mutate: regenerate, isLoading: regenerating } = useEffectMutation(
    () =>
      Effect.flatMap(CoreApi, (api) =>
        api.regeneratePendingAiResult(result.id, {
          custom_prompt: customPrompt || undefined,
          fixed_prompt: fixedPrompt || undefined,
        }),
      ),
  )

  const statusVariant = (
    {
      generating: "secondary",
      pending: "default",
      confirmed: "outline",
      failed: "destructive",
    } as const
  )[result.status] ?? "default"

  const isPending = result.status === "pending"
  const isFailed = result.status === "failed"
  const isGenerating = result.status === "generating"
  const isConfirmed = result.status === "confirmed"

  // 下拉選單選項
  const targetOptions: { value: string; label: string }[] = (() => {
    if (level === "subscription" && subscriptions) {
      return subscriptions.map((s) => ({
        value: String(s.subscription_id),
        label: s.name
          ? `#${s.subscription_id} ${s.name}`
          : `#${s.subscription_id} ${s.source_url}`,
      }))
    }
    if (level === "anime_work" && animeWorks) {
      return animeWorks.map((w) => ({
        value: String(w.anime_id),
        label: `#${w.anime_id} ${w.title}`,
      }))
    }
    return []
  })()

  return (
    <div className="space-y-4">
      {/* 標題列 */}
      <div className="flex items-center justify-between">
        <div className="space-y-1">
          <p className="font-medium">{result.source_title}</p>
          <p className="text-xs text-muted-foreground">
            {new Date(result.created_at).toLocaleString()}
          </p>
        </div>
        <Badge variant={statusVariant}>
          {isGenerating && <Loader2 className="mr-1 size-3 animate-spin" />}
          {result.status}
        </Badge>
      </div>

      {/* 錯誤訊息 */}
      {isFailed && result.error_message && (
        <p className="text-sm text-destructive bg-destructive/10 rounded p-2">
          {result.error_message}
        </p>
      )}

      {/* 固定 Prompt / 自訂 Prompt（兩欄）+ 重新生成 */}
      <div className="space-y-2">
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1">
            <Label className="text-xs">{t("aiResult.fixedPrompt")}</Label>
            <AutoResizeTextarea
              value={fixedPrompt}
              onChange={(e) => setFixedPrompt(e.target.value)}
              placeholder={t("aiResult.promptPlaceholder")}
              className="text-xs font-mono"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-xs">{t("aiResult.customPrompt")}</Label>
            <AutoResizeTextarea
              value={customPrompt}
              onChange={(e) => setCustomPrompt(e.target.value)}
              placeholder={t("aiResult.promptPlaceholder")}
              className="text-xs font-mono"
            />
          </div>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() =>
            regenerate().then((updated) => {
              if (updated) {
                setCustomPrompt("")
                setFixedPrompt(updated.used_fixed_prompt)
                onRegenerated?.(updated)
              }
            })
          }
          disabled={regenerating || isGenerating}
        >
          {regenerating ? (
            <Loader2 className="mr-1 size-3 animate-spin" />
          ) : (
            <RefreshCw className="mr-1 size-3" />
          )}
          {t("aiResult.regenerate")}
        </Button>
      </div>

      {/* 編輯器（由外部注入） */}
      {(isPending || isFailed) && children && (
        <div className="border rounded-lg p-4">{children}</div>
      )}

      {/* 預覽比較（由外部注入） */}
      {isPending && previewSlot && <div>{previewSlot}</div>}

      {/* 套用層級 + 確認/拒絕 */}
      {!isConfirmed && (
        <div className="flex items-center gap-3 pt-2 border-t flex-wrap">
          {isPending && (
            <div className="flex items-center gap-2 flex-1 flex-wrap">
              <Label className="text-sm whitespace-nowrap">{t("aiResult.applyLevel")}</Label>
              <Select value={level} onValueChange={(v) => handleLevelChange(v as typeof level)}>
                <SelectTrigger className="w-32 h-8">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="global">{t("aiResult.levelGlobal")}</SelectItem>
                  <SelectItem value="subscription">{t("aiResult.levelSubscription")}</SelectItem>
                  <SelectItem value="anime_work">{t("aiResult.levelAnimeWork")}</SelectItem>
                </SelectContent>
              </Select>
              {level !== "global" && (
                <Select
                  value={targetId !== undefined ? String(targetId) : ""}
                  onValueChange={(v) => handleTargetIdChange(v ? Number(v) : undefined)}
                >
                  <SelectTrigger className="h-8 min-w-[200px] flex-1">
                    <SelectValue
                      placeholder={level === "subscription" ? t("aiResult.selectSubscription") : t("aiResult.selectAnimeWork")}
                    />
                  </SelectTrigger>
                  <SelectContent>
                    {targetOptions.map((opt) => (
                      <SelectItem key={opt.value} value={opt.value}>
                        {opt.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </div>
          )}
          {!isPending && <div className="flex-1" />}
          <Button
            variant="outline"
            size="sm"
            onClick={() => reject().then(() => onRejected?.())}
            disabled={rejecting}
          >
            {t("aiResult.reject")}
          </Button>
          {isPending && (
            <Button
              size="sm"
              onClick={() =>
                confirm({
                  level,
                  target_id: targetId,
                }).then(() => onConfirmed?.())
              }
              disabled={confirming}
            >
              {confirming && <Loader2 className="mr-1 size-3 animate-spin" />}
              {t("aiResult.confirm")}
            </Button>
          )}
        </div>
      )}
    </div>
  )
}
