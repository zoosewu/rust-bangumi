import { useState } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
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
import { CheckCircle2, Loader2 } from "lucide-react"

interface CreateSubscriptionWizardProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onCreated?: () => void
}

type WizardStep = 1 | 2

export function CreateSubscriptionWizard({
  open,
  onOpenChange,
  onCreated,
}: CreateSubscriptionWizardProps) {
  const [step, setStep] = useState<WizardStep>(1)
  const [url, setUrl] = useState("")
  const [name, setName] = useState("")
  const [interval, setInterval] = useState("30")
  const [created, setCreated] = useState(false)

  const reset = () => {
    setStep(1)
    setUrl("")
    setName("")
    setInterval("30")
    setCreated(false)
  }

  const { mutate: createSub, isLoading: creating } = useEffectMutation(
    () =>
      Effect.flatMap(CoreApi, (api) =>
        api.createSubscription({
          source_url: url.trim(),
          name: name.trim() || undefined,
          fetch_interval_minutes: interval === "" ? 30 : parseInt(interval),
        }),
      ),
  )

  const handleCreate = () => {
    createSub().then((sub) => {
      if (sub) {
        setCreated(true)
        setStep(2)
      }
    })
  }

  const stepTitles: Record<WizardStep, string> = {
    1: "基本設定",
    2: "建立完成",
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v) reset()
        onOpenChange(v)
      }}
    >
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>新增訂閱 — {stepTitles[step]}</DialogTitle>
        </DialogHeader>

        {/* Step 指示器 */}
        <div className="flex gap-2 mb-4">
          {([1, 2] as WizardStep[]).map((s) => (
            <div
              key={s}
              className={`flex-1 h-1 rounded-full ${s <= step ? "bg-primary" : "bg-muted"}`}
            />
          ))}
        </div>

        {/* Step 1: 表單 */}
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
              <Label>抓取間隔（分鐘，0 = 單次）</Label>
              <Input
                type="number"
                min="0"
                value={interval}
                onChange={(e) => setInterval(e.target.value)}
              />
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                取消
              </Button>
              <Button onClick={handleCreate} disabled={!url.trim() || creating}>
                {creating && <Loader2 className="mr-1 size-4 animate-spin" />}
                建立訂閱
              </Button>
            </DialogFooter>
          </div>
        )}

        {/* Step 2: 完成 */}
        {step === 2 && created && (
          <div className="space-y-4">
            <div className="flex items-center gap-2 text-green-600">
              <CheckCircle2 className="size-5" />
              <span className="text-sm font-medium">訂閱建立成功</span>
            </div>
            <p className="text-sm text-muted-foreground">
              訂閱已建立，系統將在下一個排程週期自動抓取 RSS 並嘗試解析。若有解析失敗的項目，AI
              將自動生成解析器，可在「待確認」頁面查看並確認。
            </p>
            <DialogFooter>
              <Button
                onClick={() => {
                  onCreated?.()
                  onOpenChange(false)
                  reset()
                }}
              >
                完成
              </Button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
