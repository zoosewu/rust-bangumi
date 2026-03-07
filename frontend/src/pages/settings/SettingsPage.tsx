import { useState, useEffect } from "react"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { PageHeader } from "@/components/shared/PageHeader"
import { AutoResizeTextarea } from "@/components/shared/AutoResizeTextarea"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card"
import { Separator } from "@/components/ui/separator"
import { Loader2, RotateCcw } from "lucide-react"

function PromptTextarea({
  value,
  onChange,
  placeholder,
}: {
  value: string
  onChange: (v: string) => void
  placeholder?: string
}) {
  return (
    <AutoResizeTextarea
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      className="font-mono text-sm"
    />
  )
}

export default function SettingsPage() {
  return (
    <div className="space-y-6 max-w-2xl">
      <PageHeader title="設定" />
      <AiConnectionSection />
      <Separator />
      <ParserPromptSection />
      <Separator />
      <FilterPromptSection />
    </div>
  )
}

function AiConnectionSection() {
  const { data: settings } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getAiSettings),
    [],
  )

  const [baseUrl, setBaseUrl] = useState("")
  const [apiKey, setApiKey] = useState("")
  const [modelName, setModelName] = useState("")
  const [testResult, setTestResult] = useState<{ ok: boolean; error?: string } | null>(null)

  useEffect(() => {
    if (settings) {
      setBaseUrl(settings.base_url)
      setModelName(settings.model_name)
    }
  }, [settings])

  const { mutate: save, isLoading: saving } = useEffectMutation(
    () =>
      Effect.flatMap(CoreApi, (api) =>
        api.updateAiSettings({
          base_url: baseUrl,
          api_key: apiKey || undefined,
          model_name: modelName,
        }),
      ),
  )

  const { mutate: test, isLoading: testing } = useEffectMutation(
    () => Effect.flatMap(CoreApi, (api) => api.testAiConnection),
  )

  return (
    <Card>
      <CardHeader>
        <CardTitle>AI 連線設定</CardTitle>
        <CardDescription>設定 OpenAI-compatible API 連線資訊</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <Label>Base URL</Label>
          <Input
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://api.openai.com/v1"
          />
        </div>
        <div className="space-y-2">
          <Label>API Key</Label>
          <Input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="輸入新 API Key（留空保持不變）"
          />
        </div>
        <div className="space-y-2">
          <Label>Model Name</Label>
          <Input
            value={modelName}
            onChange={(e) => setModelName(e.target.value)}
            placeholder="gpt-4o-mini"
          />
        </div>
        {testResult && (
          <p className={`text-sm ${testResult.ok ? "text-green-600" : "text-destructive"}`}>
            {testResult.ok ? "連線成功" : `連線失敗: ${testResult.error}`}
          </p>
        )}
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => test().then((r) => { if (r) setTestResult(r) })}
            disabled={testing}
          >
            {testing && <Loader2 className="mr-1 size-3 animate-spin" />}
            測試連線
          </Button>
          <Button size="sm" onClick={() => save()} disabled={saving}>
            {saving && <Loader2 className="mr-1 size-3 animate-spin" />}
            儲存
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

function ParserPromptSection() {
  const { data: settings, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getAiPromptSettings),
    [],
  )
  const [fixed, setFixed] = useState("")
  const [custom, setCustom] = useState("")

  useEffect(() => {
    if (settings) {
      setFixed(settings.fixed_parser_prompt ?? "")
      setCustom(settings.custom_parser_prompt ?? "")
    }
  }, [settings])

  const { mutate: save, isLoading: saving } = useEffectMutation(
    () =>
      Effect.flatMap(CoreApi, (api) =>
        api.updateAiPromptSettings({
          fixed_parser_prompt: fixed,
          custom_parser_prompt: custom,
        }),
      ),
  )

  const { mutate: revert, isLoading: reverting } = useEffectMutation(
    () => Effect.flatMap(CoreApi, (api) => api.revertParserPrompt),
  )

  return (
    <Card>
      <CardHeader>
        <CardTitle>Parser Prompt 設定</CardTitle>
        <CardDescription>AI 生成解析器時使用的 Prompt</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label>固定 Prompt</Label>
            <Button
              variant="ghost"
              size="sm"
              onClick={() =>
                revert().then((r) => {
                  if (r) {
                    setFixed(r.value)
                    refetch()
                  }
                })
              }
              disabled={reverting}
            >
              <RotateCcw className="mr-1 size-3" />
              Revert 預設值
            </Button>
          </div>
          <PromptTextarea
            value={fixed}
            onChange={setFixed}
            placeholder="留空則不使用固定 Prompt"
          />
        </div>
        <div className="space-y-2">
          <Label>自訂 Prompt（追加在固定 Prompt 之後）</Label>
          <PromptTextarea
            value={custom}
            onChange={setCustom}
            placeholder="留空"
          />
        </div>
        <Button size="sm" onClick={() => save()} disabled={saving}>
          {saving && <Loader2 className="mr-1 size-3 animate-spin" />}
          儲存
        </Button>
      </CardContent>
    </Card>
  )
}

function FilterPromptSection() {
  const { data: settings, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getAiPromptSettings),
    [],
  )
  const [fixed, setFixed] = useState("")
  const [custom, setCustom] = useState("")

  useEffect(() => {
    if (settings) {
      setFixed(settings.fixed_filter_prompt ?? "")
      setCustom(settings.custom_filter_prompt ?? "")
    }
  }, [settings])

  const { mutate: save, isLoading: saving } = useEffectMutation(
    () =>
      Effect.flatMap(CoreApi, (api) =>
        api.updateAiPromptSettings({
          fixed_filter_prompt: fixed,
          custom_filter_prompt: custom,
        }),
      ),
  )

  const { mutate: revert, isLoading: reverting } = useEffectMutation(
    () => Effect.flatMap(CoreApi, (api) => api.revertFilterPrompt),
  )

  return (
    <Card>
      <CardHeader>
        <CardTitle>Filter Prompt 設定</CardTitle>
        <CardDescription>AI 生成過濾規則時使用的 Prompt</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label>固定 Prompt</Label>
            <Button
              variant="ghost"
              size="sm"
              onClick={() =>
                revert().then((r) => {
                  if (r) {
                    setFixed(r.value)
                    refetch()
                  }
                })
              }
              disabled={reverting}
            >
              <RotateCcw className="mr-1 size-3" />
              Revert 預設值
            </Button>
          </div>
          <PromptTextarea
            value={fixed}
            onChange={setFixed}
            placeholder="留空則不使用固定 Prompt"
          />
        </div>
        <div className="space-y-2">
          <Label>自訂 Prompt（追加在固定 Prompt 之後）</Label>
          <PromptTextarea
            value={custom}
            onChange={setCustom}
            placeholder="留空"
          />
        </div>
        <Button size="sm" onClick={() => save()} disabled={saving}>
          {saving && <Loader2 className="mr-1 size-3 animate-spin" />}
          儲存
        </Button>
      </CardContent>
    </Card>
  )
}
