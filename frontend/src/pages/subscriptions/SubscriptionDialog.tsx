import { useState } from "react"
import { useTranslation } from "react-i18next"
import { formatDateTime } from "@/lib/datetime"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"
import { ParserEditor } from "@/components/shared/ParserEditor"
import { MonospaceBlock } from "@/components/shared/MonospaceBlock"
import { InfoSection } from "@/components/shared/InfoSection"
import { InfoItem } from "@/components/shared/InfoItem"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { toast } from "sonner"
import type { Subscription } from "@/schemas/subscription"
import type { ServiceModule } from "@/schemas/service-module"

interface SubscriptionDialogProps {
  subscription: Subscription
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubscriptionChange?: () => void
}

export function SubscriptionDialog({ subscription, open, onOpenChange, onSubscriptionChange }: SubscriptionDialogProps) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  // Track saved values locally so the UI reflects changes before the parent refetches
  const [savedValues, setSavedValues] = useState({
    name: subscription.name ?? "",
    fetch_interval_minutes: subscription.fetch_interval_minutes,
    is_active: subscription.is_active,
    preferred_downloader_id: subscription.preferred_downloader_id ?? null,
  })
  const [editForm, setEditForm] = useState({
    name: subscription.name ?? "",
    fetch_interval_minutes: subscription.fetch_interval_minutes,
    is_active: subscription.is_active,
    preferred_downloader_id: subscription.preferred_downloader_id ?? null,
  })

  const { data: downloaderModules } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getDownloaderModules
      }),
    [],
  )

  const { mutate: doUpdate, isLoading: saving } = useEffectMutation(
    (req: { name?: string; fetch_interval_minutes?: number; is_active?: boolean; preferred_downloader_id?: number | null }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.updateSubscription(subscription.subscription_id, req)
      }),
  )

  const handleSave = () => {
    doUpdate({
      name: editForm.name || undefined,
      fetch_interval_minutes: editForm.fetch_interval_minutes,
      is_active: editForm.is_active,
      preferred_downloader_id: editForm.preferred_downloader_id,
    }).then(() => {
      toast.success(t("common.saved", "Saved"))
      setSavedValues({ ...editForm })
      setEditing(false)
      onSubscriptionChange?.()
    }).catch(() => {
      toast.error(t("common.saveFailed", "Save failed"))
    })
  }

  const preferredDownloaderName = savedValues.preferred_downloader_id && downloaderModules
    ? ((downloaderModules as ServiceModule[]).find(
        (m) => m.module_id === savedValues.preferred_downloader_id,
      )?.name ?? `ID: ${savedValues.preferred_downloader_id}`)
    : "-"

  return (
    <FullScreenDialog
      open={open}
      onOpenChange={onOpenChange}
      title={subscription.name ?? String(subscription.source_url)}
    >
      <div className="space-y-6">
        {/* Source URL — standalone at top */}
        <MonospaceBlock
          label={t("subscriptions.sourceUrl", "Source URL")}
          text={String(subscription.source_url)}
        />

        {/* Info section with edit mode */}
        <InfoSection
          editing={editing}
          saving={saving}
          onEdit={() => {
            setEditForm({ ...savedValues })
            setEditing(true)
          }}
          onSave={handleSave}
          onCancel={() => setEditing(false)}
        >
          <InfoItem label={t("common.id")} value={String(subscription.subscription_id)} />
          {editing ? (
            <>
              <div>
                <p className="text-xs text-muted-foreground mb-1">{t("common.name")}</p>
                <Input
                  value={editForm.name}
                  onChange={(e) => setEditForm((f) => ({ ...f, name: e.target.value }))}
                  placeholder={t("subscriptions.name")}
                  className="h-8 text-sm"
                />
              </div>
              <div>
                <p className="text-xs text-muted-foreground mb-1">{t("subscriptions.interval", "Interval")}</p>
                <div className="flex items-center gap-1">
                  <Input
                    type="number"
                    min={0}
                    value={editForm.fetch_interval_minutes}
                    onChange={(e) => setEditForm((f) => ({ ...f, fetch_interval_minutes: Number(e.target.value) }))}
                    className="h-8 text-sm w-20"
                  />
                  <span className="text-xs text-muted-foreground">min</span>
                </div>
              </div>
              <div>
                <p className="text-xs text-muted-foreground mb-1">{t("common.status")}</p>
                <div className="flex items-center gap-2 h-8">
                  <Switch
                    checked={editForm.is_active}
                    onCheckedChange={(checked) => setEditForm((f) => ({ ...f, is_active: checked }))}
                  />
                  <span className="text-sm">{editForm.is_active ? "Active" : "Inactive"}</span>
                </div>
              </div>
              {downloaderModules && downloaderModules.length > 0 && (
                <div>
                  <p className="text-xs text-muted-foreground mb-1">{t("subscriptions.preferredDownloader")}</p>
                  <Select
                    value={editForm.preferred_downloader_id ? String(editForm.preferred_downloader_id) : "none"}
                    onValueChange={(v) =>
                      setEditForm((f) => ({
                        ...f,
                        preferred_downloader_id: v === "none" ? null : Number(v),
                      }))
                    }
                  >
                    <SelectTrigger size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="none">{t("subscriptions.useGlobalPriority")}</SelectItem>
                      {(downloaderModules as ServiceModule[]).map((m) => (
                        <SelectItem key={m.module_id} value={String(m.module_id)}>
                          {m.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              )}
            </>
          ) : (
            <>
              <InfoItem label={t("common.name")} value={savedValues.name || "-"} />
              <InfoItem label={t("subscriptions.interval", "Interval")} value={savedValues.fetch_interval_minutes === 0 ? t("subscriptions.fetchOnce", "Once") : `${savedValues.fetch_interval_minutes} min`} />
              <InfoItem
                label={t("common.status")}
                value={savedValues.is_active ? "Active" : "Inactive"}
              />
              <InfoItem
                label={t("subscriptions.preferredDownloader")}
                value={preferredDownloaderName}
              />
            </>
          )}
          <InfoItem
            label={t("subscriptions.lastFetched", "Last Fetched")}
            value={subscription.last_fetched_at ? formatDateTime(String(subscription.last_fetched_at)) : t("common.never")}
          />
        </InfoSection>

        {/* Sub-tabs for filter rules and parsers */}
        <Tabs defaultValue="filters">
          <TabsList variant="line">
            <TabsTrigger value="filters">{t("dialog.filterRules", "Filter Rules")}</TabsTrigger>
            <TabsTrigger value="parsers">{t("dialog.parsers", "Parsers")}</TabsTrigger>
          </TabsList>
          <TabsContent value="filters" className="mt-4">
            <FilterRuleEditor
              targetType="fetcher"
              targetId={subscription.subscription_id}
            />
          </TabsContent>
          <TabsContent value="parsers" className="mt-4">
            <ParserEditor
              createdFromType="subscription"
              createdFromId={subscription.subscription_id}
            />
          </TabsContent>
        </Tabs>
      </div>
    </FullScreenDialog>
  )
}
