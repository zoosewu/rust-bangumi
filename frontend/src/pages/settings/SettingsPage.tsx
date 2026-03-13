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
import { Switch } from "@/components/ui/switch"
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
import { Loader2, RotateCcw, Plus, Pencil, Trash2, X, Check } from "lucide-react"
import { toast } from "sonner"
import type { ServiceModule } from "@/schemas/service-module"
import type { ResponseFormatMode } from "@/schemas/ai"
import type { Webhook } from "@/schemas/webhook"

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
      <Separator />
      <WebhookSection />
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

const DEFAULT_PAYLOAD_TEMPLATE = `{"download_id": {{download_id}}, "anime_title": "{{anime_title}}", "episode_no": {{episode_no}}, "subtitle_group": "{{subtitle_group}}", "video_path": "{{video_path}}"}`

interface WebhookFormState {
  name: string
  url: string
  payload_template: string
  is_active: boolean
}

function emptyForm(): WebhookFormState {
  return { name: "", url: "", payload_template: DEFAULT_PAYLOAD_TEMPLATE, is_active: true }
}

function WebhookSection() {
  const { t } = useTranslation()
  const { data: webhooks, refetch } = useEffectQuery(
    () => Effect.flatMap(CoreApi, (api) => api.getWebhooks),
    [],
  )

  const [editingId, setEditingId] = useState<number | "new" | null>(null)
  const [form, setForm] = useState<WebhookFormState>(emptyForm())

  const { mutate: doCreate, isLoading: creating } = useEffectMutation(
    (req: WebhookFormState) => Effect.flatMap(CoreApi, (api) => api.createWebhook(req)),
  )
  const { mutate: doUpdate, isLoading: updating } = useEffectMutation(
    ({ id, req }: { id: number; req: WebhookFormState }) =>
      Effect.flatMap(CoreApi, (api) => api.updateWebhook(id, req)),
  )
  const { mutate: doDelete, isLoading: deleting } = useEffectMutation(
    (id: number) => Effect.flatMap(CoreApi, (api) => api.deleteWebhook(id)),
  )
  const { mutate: doToggle } = useEffectMutation(
    ({ id, is_active }: { id: number; is_active: boolean }) =>
      Effect.flatMap(CoreApi, (api) => api.updateWebhook(id, { is_active })),
  )

  const isBusy = creating || updating || deleting

  const startNew = () => {
    setForm(emptyForm())
    setEditingId("new")
  }

  const startEdit = (w: Webhook) => {
    setForm({ name: w.name, url: w.url, payload_template: w.payload_template, is_active: w.is_active })
    setEditingId(w.webhook_id)
  }

  const cancelEdit = () => setEditingId(null)

  const handleSave = async () => {
    if (!form.name.trim() || !form.url.trim()) {
      toast.error(t("settings.webhook.nameUrlRequired"))
      return
    }
    if (editingId === "new") {
      await doCreate(form)
      toast.success(t("settings.webhook.created"))
    } else if (typeof editingId === "number") {
      await doUpdate({ id: editingId, req: form })
      toast.success(t("settings.saved"))
    }
    setEditingId(null)
    refetch()
  }

  const handleDelete = async (id: number) => {
    await doDelete(id)
    toast.success(t("settings.webhook.deleted"))
    refetch()
  }

  const handleToggle = async (w: Webhook) => {
    await doToggle({ id: w.webhook_id, is_active: !w.is_active })
    refetch()
  }

  const list = webhooks ?? []

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle>{t("settings.webhook.title")}</CardTitle>
            <CardDescription className="mt-1">{t("settings.webhook.description")}</CardDescription>
          </div>
          {editingId === null && (
            <Button size="sm" variant="outline" onClick={startNew}>
              <Plus className="mr-1 size-3" />
              {t("settings.webhook.add")}
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {list.length === 0 && editingId === null && (
          <p className="text-sm text-muted-foreground">{t("settings.webhook.empty")}</p>
        )}

        {list.map((w) => (
          <div key={w.webhook_id}>
            {editingId === w.webhook_id ? (
              <WebhookForm
                form={form}
                onChange={setForm}
                onSave={handleSave}
                onCancel={cancelEdit}
                isBusy={isBusy}
                t={t}
              />
            ) : (
              <div className="flex items-center gap-2 py-1.5 border rounded-md px-3">
                <Switch
                  checked={w.is_active}
                  onCheckedChange={() => handleToggle(w)}
                  className="shrink-0"
                />
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium truncate">{w.name}</p>
                  <p className="text-xs text-muted-foreground truncate">{w.url}</p>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-7 shrink-0"
                  onClick={() => startEdit(w)}
                >
                  <Pencil className="size-3" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-7 shrink-0 text-destructive hover:text-destructive"
                  onClick={() => handleDelete(w.webhook_id)}
                  disabled={deleting}
                >
                  <Trash2 className="size-3" />
                </Button>
              </div>
            )}
          </div>
        ))}

        {editingId === "new" && (
          <WebhookForm
            form={form}
            onChange={setForm}
            onSave={handleSave}
            onCancel={cancelEdit}
            isBusy={isBusy}
            t={t}
          />
        )}
      </CardContent>
    </Card>
  )
}

interface TestResult {
  ok: boolean
  status_code?: number
  error?: string
  rendered_payload: string
}

function WebhookForm({
  form,
  onChange,
  onSave,
  onCancel,
  isBusy,
  t,
}: {
  form: WebhookFormState
  onChange: (f: WebhookFormState) => void
  onSave: () => void
  onCancel: () => void
  isBusy: boolean
  t: ReturnType<typeof useTranslation>["t"]
}) {
  const [testResult, setTestResult] = useState<TestResult | null>(null)

  const { mutate: doTest, isLoading: testing } = useEffectMutation(
    (req: { url: string; payload_template: string }) =>
      Effect.flatMap(CoreApi, (api) => api.testWebhook(req)),
  )

  const handleTest = async () => {
    if (!form.url.trim()) {
      toast.error(t("settings.webhook.nameUrlRequired"))
      return
    }
    const result = await doTest({ url: form.url, payload_template: form.payload_template })
    if (result) {
      setTestResult(result)
    }
  }

  // Reset test result when form changes
  const handleChange = (next: WebhookFormState) => {
    setTestResult(null)
    onChange(next)
  }

  return (
    <div className="border rounded-md p-4 space-y-4 bg-muted/30">
      {/* Name */}
      <div className="space-y-1.5">
        <Label className="text-sm">{t("settings.webhook.name")}</Label>
        <Input
          value={form.name}
          onChange={(e) => handleChange({ ...form, name: e.target.value })}
          placeholder={t("settings.webhook.namePlaceholder")}
          className="text-sm"
        />
      </div>

      {/* URL */}
      <div className="space-y-1.5">
        <Label className="text-sm">URL</Label>
        <Input
          value={form.url}
          onChange={(e) => handleChange({ ...form, url: e.target.value })}
          placeholder="https://example.com/webhook"
          className="text-sm font-mono"
        />
      </div>

      {/* Payload Template */}
      <div className="space-y-1.5">
        <Label className="text-sm">{t("settings.webhook.payloadTemplate")}</Label>
        <p className="text-xs text-muted-foreground">{t("settings.webhook.templateVars")}</p>
        <AutoResizeTextarea
          value={form.payload_template}
          onChange={(e) => handleChange({ ...form, payload_template: e.target.value })}
          className="font-mono text-xs"
          placeholder={DEFAULT_PAYLOAD_TEMPLATE}
        />
      </div>

      {/* Test Result */}
      {testResult && (
        <div className={`rounded-md border p-3 space-y-2 text-xs ${testResult.ok ? "border-green-500/40 bg-green-500/5" : "border-destructive/40 bg-destructive/5"}`}>
          <div className="flex items-center gap-1.5 font-medium">
            {testResult.ok ? (
              <span className="text-green-600">
                ✓ {t("settings.webhook.testSuccess")}
                {testResult.status_code && ` (HTTP ${testResult.status_code})`}
              </span>
            ) : (
              <span className="text-destructive">
                ✗ {t("settings.webhook.testFailure")}
                {testResult.status_code && ` (HTTP ${testResult.status_code})`}
                {testResult.error && `: ${testResult.error}`}
              </span>
            )}
          </div>
          <div className="space-y-1">
            <p className="text-muted-foreground">{t("settings.webhook.renderedPayload")}</p>
            <pre className="bg-background rounded p-2 overflow-x-auto whitespace-pre-wrap break-all font-mono">
              {testResult.rendered_payload}
            </pre>
          </div>
        </div>
      )}

      {/* Footer: active toggle + buttons */}
      <div className="flex items-center gap-3 pt-1">
        <div className="flex items-center gap-2 flex-1">
          <Switch
            checked={form.is_active}
            onCheckedChange={(v) => handleChange({ ...form, is_active: v })}
          />
          <Label className="text-sm">{t("settings.webhook.enabledLabel")}</Label>
        </div>
        <div className="flex gap-2">
          <Button variant="ghost" size="sm" onClick={onCancel} disabled={isBusy || testing}>
            <X className="mr-1 size-3" />
            {t("common.cancel")}
          </Button>
          <Button variant="outline" size="sm" onClick={handleTest} disabled={isBusy || testing}>
            {testing ? <Loader2 className="mr-1 size-3 animate-spin" /> : null}
            {t("settings.webhook.test")}
          </Button>
          <Button size="sm" onClick={onSave} disabled={isBusy || testing}>
            {isBusy ? (
              <Loader2 className="mr-1 size-3 animate-spin" />
            ) : (
              <Check className="mr-1 size-3" />
            )}
            {t("common.save")}
          </Button>
        </div>
      </div>
    </div>
  )
}
