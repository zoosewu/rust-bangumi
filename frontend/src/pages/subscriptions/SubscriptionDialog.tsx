import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"
import { ParserEditor } from "@/components/shared/ParserEditor"
import { CopyButton } from "@/components/shared/CopyButton"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Pencil, Save, X } from "lucide-react"
import { toast } from "sonner"
import type { Subscription } from "@/schemas/subscription"

interface SubscriptionDialogProps {
  subscription: Subscription
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubscriptionChange?: () => void
}

export function SubscriptionDialog({ subscription, open, onOpenChange, onSubscriptionChange }: SubscriptionDialogProps) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  const [editForm, setEditForm] = useState({
    name: subscription.name ?? "",
    fetch_interval_minutes: subscription.fetch_interval_minutes,
    is_active: subscription.is_active,
  })

  const { mutate: doUpdate, isLoading: saving } = useEffectMutation(
    (req: { name?: string; fetch_interval_minutes?: number; is_active?: boolean }) =>
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
    }).then(() => {
      toast.success(t("common.saved", "Saved"))
      setEditing(false)
      onSubscriptionChange?.()
    }).catch(() => {
      toast.error(t("common.saveFailed", "Save failed"))
    })
  }

  return (
    <FullScreenDialog
      open={open}
      onOpenChange={onOpenChange}
      title={subscription.name ?? String(subscription.source_url)}
    >
      <div className="space-y-6">
        {/* Source URL — standalone at top */}
        <div>
          <p className="text-xs text-muted-foreground mb-1">{t("subscriptions.sourceUrl", "Source URL")}</p>
          <div className="flex items-start gap-1 bg-muted/50 rounded p-2">
            <p className="text-sm font-mono break-all flex-1">{String(subscription.source_url)}</p>
            <CopyButton text={String(subscription.source_url)} />
          </div>
        </div>

        {/* Info section with edit mode — same pattern as AnimeSeriesDialog */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-medium text-muted-foreground">{t("dialog.info", "Info")}</h3>
            {!editing ? (
              <Button variant="ghost" size="sm" onClick={() => {
                setEditForm({
                  name: subscription.name ?? "",
                  fetch_interval_minutes: subscription.fetch_interval_minutes,
                  is_active: subscription.is_active,
                })
                setEditing(true)
              }}>
                <Pencil className="h-3.5 w-3.5 mr-1" />
                {t("common.edit", "Edit")}
              </Button>
            ) : (
              <div className="flex gap-1">
                <Button variant="ghost" size="sm" onClick={() => setEditing(false)} disabled={saving}>
                  <X className="h-3.5 w-3.5 mr-1" />
                  {t("common.cancel", "Cancel")}
                </Button>
                <Button size="sm" onClick={handleSave} disabled={saving}>
                  <Save className="h-3.5 w-3.5 mr-1" />
                  {t("common.save", "Save")}
                </Button>
              </div>
            )}
          </div>

          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
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
                      min={1}
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
              </>
            ) : (
              <>
                <InfoItem label={t("common.name")} value={subscription.name ?? "-"} />
                <InfoItem label={t("subscriptions.interval", "Interval")} value={`${subscription.fetch_interval_minutes} min`} />
                <InfoItem
                  label={t("common.status")}
                  value={subscription.is_active ? "Active" : "Inactive"}
                />
              </>
            )}
            <InfoItem
              label={t("subscriptions.lastFetched", "Last Fetched")}
              value={subscription.last_fetched_at ? String(subscription.last_fetched_at).slice(0, 19).replace("T", " ") : t("common.never")}
            />
          </div>
        </div>

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

function InfoItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="text-sm font-medium break-all">{value}</p>
    </div>
  )
}
