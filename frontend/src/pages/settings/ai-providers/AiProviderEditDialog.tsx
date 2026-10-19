import { useEffect, useState } from "react"
import { ChevronDown, Loader2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import type {
  AiProvider,
  AiProviderKind,
  CreateAiProviderRequest,
  ResponseFormatMode,
  UpdateAiProviderRequest,
  TestAiProviderResult,
  ListAiModelsRequest,
} from "@/schemas/ai"

const KINDS: AiProviderKind[] = ["openai_compatible"]
const MODES: ResponseFormatMode[] = ["strict", "non_strict", "inject_schema"]

export interface AiProviderEditDialogProps {
  /** null = 新增模式；AiProvider = 編輯模式 */
  provider: AiProvider | null
  onClose: () => void
  onSubmit: (req: CreateAiProviderRequest | UpdateAiProviderRequest) => Promise<void>
  onTestConfig: (req: CreateAiProviderRequest) => Promise<TestAiProviderResult>
  /** 向 provider 取得模型列表（失敗時 reject 並帶錯誤訊息） */
  onListModels: (req: ListAiModelsRequest) => Promise<readonly string[]>
}

type ModelList =
  | { state: "idle" }
  | { state: "loading"; source: string }
  | { state: "loaded"; source: string; models: readonly string[] }
  | { state: "error"; source: string; message: string }

export function AiProviderEditDialog({
  provider,
  onClose,
  onSubmit,
  onTestConfig,
  onListModels,
}: AiProviderEditDialogProps) {
  const isEdit = provider !== null
  const [name, setName] = useState("")
  const [kind, setKind] = useState<AiProviderKind>("openai_compatible")
  const [baseUrl, setBaseUrl] = useState("")
  const [apiKey, setApiKey] = useState("")
  const [modelName, setModelName] = useState("")
  const [maxTokens, setMaxTokens] = useState("4096")
  const [mode, setMode] = useState<ResponseFormatMode>("non_strict")
  const [enabled, setEnabled] = useState(true)
  const [saving, setSaving] = useState(false)
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestAiProviderResult | null>(null)
  const [modelList, setModelList] = useState<ModelList>({ state: "idle" })

  useEffect(() => {
    if (provider) {
      setName(provider.name)
      setKind(provider.provider_kind)
      setBaseUrl(provider.base_url)
      setApiKey("") // 編輯模式留空 = 不更新
      setModelName(provider.model_name)
      setMaxTokens(String(provider.max_tokens))
      setMode(provider.response_format_mode)
      setEnabled(provider.is_enabled)
    } else {
      setName("")
      setKind("openai_compatible")
      setBaseUrl("")
      setApiKey("")
      setModelName("")
      setMaxTokens("4096")
      setMode("non_strict")
      setEnabled(true)
    }
    setTestResult(null)
    setModelList({ state: "idle" })
  }, [provider])

  // 列表跟著撈取時的 Base URL 與 Key；兩者改了就視為還沒撈過
  const source = `${baseUrl}\n${apiKey}`
  const current: ModelList =
    modelList.state !== "idle" && modelList.source !== source ? { state: "idle" } : modelList

  const loadModels = async () => {
    setModelList({ state: "loading", source })
    try {
      const models = await onListModels({
        existing_provider_id: provider?.id,
        base_url: baseUrl,
        api_key: apiKey,
      })
      setModelList({ state: "loaded", source, models })
    } catch (e) {
      setModelList({
        state: "error",
        source,
        message: e instanceof Error ? e.message : String(e),
      })
    }
  }

  const filter = modelName.trim().toLowerCase()
  const shownModels =
    current.state === "loaded"
      ? current.models.filter((m) => !filter || m.toLowerCase().includes(filter))
      : []

  const buildTestRequest = (): CreateAiProviderRequest => ({
    existing_provider_id: provider?.id,
    name,
    provider_kind: kind,
    base_url: baseUrl,
    api_key: apiKey,
    model_name: modelName,
    max_tokens: Number(maxTokens) || 4096,
    response_format_mode: mode,
    is_enabled: enabled,
  })

  const handleTest = async () => {
    setTesting(true)
    setTestResult(null)
    try {
      const result = await onTestConfig(buildTestRequest())
      setTestResult(result)
    } catch (e) {
      setTestResult({
        ok: false,
        error: e instanceof Error ? e.message : String(e),
      })
    } finally {
      setTesting(false)
    }
  }

  const handleSubmit = async () => {
    setSaving(true)
    try {
      if (isEdit) {
        const req: UpdateAiProviderRequest = {
          name,
          base_url: baseUrl,
          api_key: apiKey, // 空字串 → 後端保留舊值
          model_name: modelName,
          max_tokens: Number(maxTokens) || 4096,
          response_format_mode: mode,
          is_enabled: enabled,
        }
        await onSubmit(req)
      } else {
        const req: CreateAiProviderRequest = {
          name,
          provider_kind: kind,
          base_url: baseUrl,
          api_key: apiKey,
          model_name: modelName,
          max_tokens: Number(maxTokens) || 4096,
          response_format_mode: mode,
          is_enabled: enabled,
        }
        await onSubmit(req)
      }
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog
      open
      onOpenChange={(o) => {
        if (!o) onClose()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{isEdit ? "編輯 Provider" : "新增 Provider"}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1">
            <Label>名稱</Label>
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="space-y-1">
            <Label>協議</Label>
            <Select
              value={kind}
              onValueChange={(v) => setKind(v as AiProviderKind)}
              disabled={isEdit}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {KINDS.map((k) => (
                  <SelectItem key={k} value={k}>
                    {k}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label>Base URL</Label>
            <Input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.openai.com/v1"
            />
          </div>
          <div className="space-y-1">
            <Label>API Key</Label>
            <Input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={isEdit ? "••••••••（留空表示不變更）" : "sk-..."}
            />
          </div>
          <div className="space-y-1">
            <Label>Model 名稱</Label>
            <div className="flex gap-1">
              <Input
                value={modelName}
                onChange={(e) => setModelName(e.target.value)}
                placeholder="gpt-4o-mini"
              />
              <DropdownMenu
                onOpenChange={(open) => {
                  if (open && current.state !== "loaded") void loadModels()
                }}
              >
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="outline"
                    size="icon"
                    aria-label="選擇模型"
                    disabled={!baseUrl}
                  >
                    <ChevronDown className="size-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="max-h-72 w-80 overflow-y-auto">
                  {current.state === "loading" && (
                    <div className="flex items-center gap-2 p-2 text-sm text-muted-foreground">
                      <Loader2 className="size-3 animate-spin" /> 載入模型列表…
                    </div>
                  )}
                  {current.state === "error" && (
                    <div className="p-2 text-sm text-destructive">{current.message}</div>
                  )}
                  {current.state === "loaded" && shownModels.length === 0 && (
                    <div className="p-2 text-sm text-muted-foreground">
                      {current.models.length === 0 ? "provider 沒有回傳任何模型" : "沒有符合的模型"}
                    </div>
                  )}
                  {shownModels.map((m) => (
                    <DropdownMenuItem key={m} onSelect={() => setModelName(m)}>
                      <span className="truncate font-mono text-xs">{m}</span>
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
            {current.state === "loaded" && (
              <p className="text-xs text-muted-foreground">
                共 {current.models.length} 個模型，可在輸入框打字篩選
              </p>
            )}
          </div>
          <div className="space-y-1">
            <Label>Max Tokens</Label>
            <Input
              type="number"
              value={maxTokens}
              onChange={(e) => setMaxTokens(e.target.value)}
              min={256}
              max={128000}
              className="w-40"
            />
          </div>
          <div className="space-y-1">
            <Label>Response Format</Label>
            <Select
              value={mode}
              onValueChange={(v) => setMode(v as ResponseFormatMode)}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {MODES.map((m) => (
                  <SelectItem key={m} value={m}>
                    {m}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-2">
            <Switch checked={enabled} onCheckedChange={setEnabled} />
            <Label>啟用</Label>
          </div>
          {testResult && (
            <div
              className={`rounded-md border p-3 text-sm ${
                testResult.ok
                  ? "border-green-500/40 bg-green-500/5 text-green-700"
                  : "border-destructive/40 bg-destructive/5 text-destructive"
              }`}
            >
              {testResult.ok ? "✓ 測試成功" : `✗ ${testResult.error ?? "測試失敗"}`}
            </div>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose} disabled={saving || testing}>
            取消
          </Button>
          <Button
            variant="secondary"
            onClick={handleTest}
            disabled={saving || testing || !baseUrl || !modelName}
          >
            {testing ? <Loader2 className="mr-1 size-3 animate-spin" /> : null}
            測試
          </Button>
          <Button onClick={handleSubmit} disabled={saving || testing || !name}>
            儲存
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
