import { useState, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { PageHeader } from "@/components/shared/PageHeader"
import { AutoResizeTextarea } from "@/components/shared/AutoResizeTextarea"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Separator } from "@/components/ui/separator"
import { Loader2, RotateCcw } from "lucide-react"
import { toast } from "sonner"
import type { ServiceModule } from "@/schemas/service-module"
import type { ResponseFormatMode } from "@/schemas/ai"

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
  const { t } = useTranslation()
  return (
    <div className="space-y-6 max-w-2xl">
      <PageHeader title={t("settings.title")} />
      <DownloaderPrioritySection />
      <Separator />
      <AiConnectionSection />
      <Separator />
      <ParserPromptSection />
      <Separator />
      <FilterPromptSection />
    </div>
  )
}

function DownloaderPrioritySection() {
  const { t } = useTranslation()
  const { data: modules, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getDownloaderModules),
    [],
  )

  const { mutate: doUpdate, isLoading: saving } = useEffectMutation(
    ({ id, priority }: { id: number; priority: number }) =>
      Effect.flatMap(CoreApi, (api) => api.updateServiceModule(id, { priority })),
  )

  const [drafts, setDrafts] = useState<Record<number, string>>({})

  useEffect(() => {
    if (modules) {
      const initial: Record<number, string> = {}
      modules.forEach((m: ServiceModule) => { initial[m.module_id] = String(m.priority) })
      setDrafts(initial)
    }
  }, [modules])

  if (!modules || modules.length === 0) return null

  const sorted = [...modules].sort((a: ServiceModule, b: ServiceModule) => b.priority - a.priority)

  const handleSaveAll = async () => {
    for (const m of sorted) {
      const priority = Number(drafts[m.module_id] ?? m.priority)
      if (priority !== m.priority) {
        await doUpdate({ id: m.module_id, priority })
      }
    }
    toast.success(t("settings.saved"))
    refetch()
  }

  const hasPendingChanges = sorted.some(
    (m: ServiceModule) => String(drafts[m.module_id] ?? m.priority) !== String(m.priority),
  )

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.downloader.title")}</CardTitle>
        <CardDescription>{t("settings.downloader.description")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        {sorted.map((m: ServiceModule, idx: number) => (
          <div key={m.module_id} className="flex items-center gap-3">
            <Badge variant="outline" className="text-xs w-5 justify-center shrink-0">
              {idx + 1}
            </Badge>
            <span className="text-sm flex-1 truncate">{m.name}</span>
            <span className="text-xs text-muted-foreground shrink-0">{t("settings.downloader.priority")}</span>
            <Input
              type="number"
              className="w-20 h-8 text-sm"
              value={drafts[m.module_id] ?? m.priority}
              onChange={(e) =>
                setDrafts((d) => ({ ...d, [m.module_id]: e.target.value }))
              }
            />
          </div>
        ))}
        <div className="flex justify-end pt-1">
          <Button size="sm" onClick={handleSaveAll} disabled={saving || !hasPendingChanges}>
            {saving && <Loader2 className="mr-1 size-3 animate-spin" />}
            {t("common.save")}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

function AiConnectionSection() {
  const { t } = useTranslation()

  const responseFormatOptions: { value: ResponseFormatMode; label: string; description: string }[] = [
    {
      value: "strict",
      label: t("settings.ai.responseFormat_strict_label"),
      description: t("settings.ai.responseFormat_strict_desc"),
    },
    {
      value: "non_strict",
      label: t("settings.ai.responseFormat_non_strict_label"),
      description: t("settings.ai.responseFormat_non_strict_desc"),
    },
    {
      value: "inject_schema",
      label: t("settings.ai.responseFormat_inject_schema_label"),
      description: t("settings.ai.responseFormat_inject_schema_desc"),
    },
  ]

  const { data: settings } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getAiSettings),
    [],
  )

  const [baseUrl, setBaseUrl] = useState("")
  const [apiKey, setApiKey] = useState("")
  const [modelName, setModelName] = useState("")
  const [maxTokens, setMaxTokens] = useState("4096")
  const [responseFormatMode, setResponseFormatMode] = useState<ResponseFormatMode>("strict")
  const [testResult, setTestResult] = useState<{ ok: boolean; error?: string } | null>(null)

  useEffect(() => {
    if (settings) {
      setBaseUrl(settings.base_url)
      setModelName(settings.model_name)
      setMaxTokens(String(settings.max_tokens))
      setResponseFormatMode(settings.response_format_mode)
    }
  }, [settings])

  const { mutate: save, isLoading: saving } = useEffectMutation(
    () =>
      Effect.flatMap(CoreApi, (api) =>
        api.updateAiSettings({
          base_url: baseUrl,
          api_key: apiKey || undefined,
          model_name: modelName,
          max_tokens: Number(maxTokens) || 4096,
          response_format_mode: responseFormatMode,
        }),
      ),
  )

  const { mutate: test, isLoading: testing } = useEffectMutation(
    () => Effect.flatMap(CoreApi, (api) => api.testAiConnection),
  )

  const selectedOption = responseFormatOptions.find((o) => o.value === responseFormatMode)

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.ai.title")}</CardTitle>
        <CardDescription>{t("settings.ai.description")}</CardDescription>
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
            placeholder={t("settings.ai.apiKeyPlaceholder")}
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
        <div className="space-y-2">
          <Label>{t("settings.ai.maxTokens")}</Label>
          <Input
            type="number"
            value={maxTokens}
            onChange={(e) => setMaxTokens(e.target.value)}
            placeholder="4096"
            min={256}
            max={128000}
            className="w-40"
          />
          <p className="text-xs text-muted-foreground">{t("settings.ai.maxTokensHint")}</p>
        </div>
        <div className="space-y-2">
          <Label>{t("settings.ai.responseFormat")}</Label>
          <Select
            value={responseFormatMode}
            onValueChange={(v) => setResponseFormatMode(v as ResponseFormatMode)}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {responseFormatOptions.map((opt) => (
                <SelectItem key={opt.value} value={opt.value}>
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {selectedOption && (
            <p className="text-xs text-muted-foreground">{selectedOption.description}</p>
          )}
        </div>
        {testResult && (
          <p className={`text-sm ${testResult.ok ? "text-green-600" : "text-destructive"}`}>
            {testResult.ok
              ? t("settings.ai.testSuccess")
              : t("settings.ai.testFailure", { error: testResult.error })}
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
            {t("settings.ai.testConnection")}
          </Button>
          <Button size="sm" onClick={() => save()} disabled={saving}>
            {saving && <Loader2 className="mr-1 size-3 animate-spin" />}
            {t("common.save")}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

function ParserPromptSection() {
  const { t } = useTranslation()
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
        <CardTitle>{t("settings.parserPrompt.title")}</CardTitle>
        <CardDescription>{t("settings.parserPrompt.description")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label>{t("settings.parserPrompt.fixedPrompt")}</Label>
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
              {t("settings.parserPrompt.revertDefault")}
            </Button>
          </div>
          <PromptTextarea
            value={fixed}
            onChange={setFixed}
            placeholder={t("settings.parserPrompt.fixedPromptPlaceholder")}
          />
        </div>
        <div className="space-y-2">
          <Label>{t("settings.parserPrompt.customPrompt")}</Label>
          <PromptTextarea
            value={custom}
            onChange={setCustom}
            placeholder={t("settings.parserPrompt.customPromptPlaceholder")}
          />
        </div>
        <Button size="sm" onClick={() => save()} disabled={saving}>
          {saving && <Loader2 className="mr-1 size-3 animate-spin" />}
          {t("common.save")}
        </Button>
      </CardContent>
    </Card>
  )
}

function FilterPromptSection() {
  const { t } = useTranslation()
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
        <CardTitle>{t("settings.filterPrompt.title")}</CardTitle>
        <CardDescription>{t("settings.filterPrompt.description")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label>{t("settings.filterPrompt.fixedPrompt")}</Label>
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
              {t("settings.filterPrompt.revertDefault")}
            </Button>
          </div>
          <PromptTextarea
            value={fixed}
            onChange={setFixed}
            placeholder={t("settings.filterPrompt.fixedPromptPlaceholder")}
          />
        </div>
        <div className="space-y-2">
          <Label>{t("settings.filterPrompt.customPrompt")}</Label>
          <PromptTextarea
            value={custom}
            onChange={setCustom}
            placeholder={t("settings.filterPrompt.customPromptPlaceholder")}
          />
        </div>
        <Button size="sm" onClick={() => save()} disabled={saving}>
          {saving && <Loader2 className="mr-1 size-3 animate-spin" />}
          {t("common.save")}
        </Button>
      </CardContent>
    </Card>
  )
}
