import { useState } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
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

interface AiResultPanelProps {
  result: PendingAiResult
  onConfirmed?: () => void
  onRejected?: () => void
  onRegenerated?: (updated: PendingAiResult) => void
  children?: React.ReactNode
  previewSlot?: React.ReactNode
}

export function AiResultPanel({
  result,
  onConfirmed,
  onRejected,
  onRegenerated,
  children,
  previewSlot,
}: AiResultPanelProps) {
  const [tempPrompt, setTempPrompt] = useState("")
  const [tempFixedPrompt, setTempFixedPrompt] = useState(result.used_fixed_prompt)
  const [level, setLevel] = useState<"global" | "subscription" | "anime_work">("global")
  const [targetId, setTargetId] = useState<string>("")

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
          custom_prompt: tempPrompt || undefined,
          fixed_prompt: tempFixedPrompt || undefined,
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

      {/* 編輯器（由外部注入） */}
      {(isPending || isFailed) && children && (
        <div className="border rounded-lg p-4">{children}</div>
      )}

      {/* 預覽比較（由外部注入） */}
      {isPending && previewSlot && <div>{previewSlot}</div>}

      {/* 固定 Prompt（可臨時覆蓋） */}
      <div className="space-y-2">
        <Label className="text-sm">固定 Prompt（臨時覆蓋，不影響全局設定）</Label>
        <Textarea
          value={tempFixedPrompt}
          onChange={(e) => setTempFixedPrompt(e.target.value)}
          rows={4}
          className="text-sm font-mono"
        />
      </div>

      {/* 臨時自訂 Prompt */}
      <div className="space-y-2">
        <Label className="text-sm">臨時自訂 Prompt（僅影響本次重新生成）</Label>
        <Textarea
          value={tempPrompt}
          onChange={(e) => setTempPrompt(e.target.value)}
          placeholder={result.used_custom_prompt ?? "留空使用全局設定"}
          rows={3}
          className="text-sm font-mono"
        />
        <Button
          variant="outline"
          size="sm"
          onClick={() =>
            regenerate().then((updated) => {
              if (updated) {
                setTempPrompt("")
                setTempFixedPrompt(updated.used_fixed_prompt)
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
          重新生成
        </Button>
      </div>

      {/* 套用層級 + 確認/拒絕 */}
      {isPending && (
        <div className="flex items-center gap-3 pt-2 border-t">
          <div className="flex items-center gap-2 flex-1">
            <Label className="text-sm whitespace-nowrap">套用層級</Label>
            <Select value={level} onValueChange={(v) => setLevel(v as typeof level)}>
              <SelectTrigger className="w-36 h-8">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="global">全局</SelectItem>
                <SelectItem value="subscription">訂閱</SelectItem>
                <SelectItem value="anime_work">動畫作品</SelectItem>
              </SelectContent>
            </Select>
            {level !== "global" && (
              <input
                type="number"
                placeholder="目標 ID"
                value={targetId}
                onChange={(e) => setTargetId(e.target.value)}
                className="h-8 w-24 rounded border px-2 text-sm"
              />
            )}
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() => reject().then(() => onRejected?.())}
            disabled={rejecting}
          >
            拒絕
          </Button>
          <Button
            size="sm"
            onClick={() =>
              confirm({
                level,
                target_id: targetId ? parseInt(targetId) : undefined,
              }).then(() => onConfirmed?.())
            }
            disabled={confirming}
          >
            {confirming && <Loader2 className="mr-1 size-3 animate-spin" />}
            確認套用
          </Button>
        </div>
      )}
    </div>
  )
}
