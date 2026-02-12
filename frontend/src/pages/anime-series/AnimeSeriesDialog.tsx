import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"
import { ParserEditor } from "@/components/shared/ParserEditor"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { SubtitleGroupDialog } from "@/pages/subtitle-groups/SubtitleGroupDialog"
import { cn } from "@/lib/utils"
import { Pencil, Save, X } from "lucide-react"
import { toast } from "sonner"
import type { AnimeSeriesRich, AnimeLinkRich } from "@/schemas/anime"

interface AnimeSeriesDialogProps {
  series: AnimeSeriesRich
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function AnimeSeriesDialog({ series, open, onOpenChange }: AnimeSeriesDialogProps) {
  const { t } = useTranslation()
  const [groupDialog, setGroupDialog] = useState<{ id: number; name: string } | null>(null)
  const [editing, setEditing] = useState(false)
  const [editForm, setEditForm] = useState({
    description: series.description ?? "",
    aired_date: series.aired_date?.slice(0, 10) ?? "",
    end_date: series.end_date?.slice(0, 10) ?? "",
  })

  const { data: links, refetch: refetchLinks } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getAnimeLinksRich(series.series_id)
      }),
    [series.series_id],
  )

  const { mutate: doUpdate, isLoading: saving } = useEffectMutation(
    (req: { description?: string | null; aired_date?: string | null; end_date?: string | null }) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.updateAnimeSeries(series.series_id, req)
      }),
  )

  const handleSave = () => {
    doUpdate({
      description: editForm.description || null,
      aired_date: editForm.aired_date || null,
      end_date: editForm.end_date || null,
    }).then(() => {
      toast.success(t("common.saved", "Saved"))
      setEditing(false)
    }).catch(() => {
      toast.error(t("common.saveFailed", "Save failed"))
    })
  }

  const passedLinks = (links ?? []).filter((l) => !l.filtered_flag)
  const filteredLinks = (links ?? []).filter((l) => l.filtered_flag)

  return (
    <>
      <FullScreenDialog
        open={open}
        onOpenChange={onOpenChange}
        title={`${series.anime_title} - S${series.series_no}`}
      >
        <div className="space-y-6">
          {/* Info section */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-medium text-muted-foreground">{t("dialog.info", "Info")}</h3>
              {!editing ? (
                <Button variant="ghost" size="sm" onClick={() => {
                  setEditForm({
                    description: series.description ?? "",
                    aired_date: series.aired_date?.slice(0, 10) ?? "",
                    end_date: series.end_date?.slice(0, 10) ?? "",
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
              <InfoItem label={t("animeSeries.animeTitle", "Anime")} value={series.anime_title} />
              <InfoItem
                label={t("animeSeries.season", "Aired Season")}
                value={`${series.season.year} ${series.season.season}`}
              />
              <InfoItem
                label={t("animeSeries.episodes", "Episodes")}
                value={`${series.episode_downloaded} / ${series.episode_found}`}
              />
              <InfoItem
                label={t("animeSeries.subscriptions", "Subscriptions")}
                value={
                  series.subscriptions.length > 0
                    ? series.subscriptions.map((s) => s.name ?? `#${s.subscription_id}`).join(", ")
                    : "-"
                }
              />
              {editing ? (
                <>
                  <EditItem
                    label={t("animeSeries.airedDate", "Aired")}
                    type="date"
                    value={editForm.aired_date}
                    onChange={(v) => setEditForm((f) => ({ ...f, aired_date: v }))}
                  />
                  <EditItem
                    label={t("animeSeries.endDate", "Ended")}
                    type="date"
                    value={editForm.end_date}
                    onChange={(v) => setEditForm((f) => ({ ...f, end_date: v }))}
                  />
                  <div className="col-span-full">
                    <EditItem
                      label={t("animeSeries.description", "Description")}
                      value={editForm.description}
                      onChange={(v) => setEditForm((f) => ({ ...f, description: v }))}
                    />
                  </div>
                </>
              ) : (
                <>
                  {series.aired_date && (
                    <InfoItem label={t("animeSeries.airedDate", "Aired")} value={series.aired_date.slice(0, 10)} />
                  )}
                  {series.end_date && (
                    <InfoItem label={t("animeSeries.endDate", "Ended")} value={series.end_date.slice(0, 10)} />
                  )}
                  {series.description && (
                    <div className="col-span-full">
                      <InfoItem label={t("animeSeries.description", "Description")} value={series.description} />
                    </div>
                  )}
                </>
              )}
            </div>
          </div>

          {/* Main tabs */}
          <Tabs defaultValue="details">
            <TabsList>
              <TabsTrigger value="details">{t("dialog.details", "Details")}</TabsTrigger>
              <TabsTrigger value="links">{t("dialog.animeLinks", "Anime Links")}</TabsTrigger>
            </TabsList>

            <TabsContent value="details" className="mt-4">
              {/* Sub-tabs for filter rules and parsers */}
              <Tabs defaultValue="filters">
                <TabsList variant="line">
                  <TabsTrigger value="filters">{t("dialog.filterRules", "Filter Rules")}</TabsTrigger>
                  <TabsTrigger value="parsers">{t("dialog.parsers", "Parsers")}</TabsTrigger>
                </TabsList>
                <TabsContent value="filters" className="mt-4">
                  <FilterRuleEditor
                    targetType="anime_series"
                    targetId={series.series_id}
                    onRulesChange={refetchLinks}
                  />
                </TabsContent>
                <TabsContent value="parsers" className="mt-4">
                  <ParserEditor
                    createdFromType="anime_series"
                    createdFromId={series.series_id}
                  />
                </TabsContent>
              </Tabs>
            </TabsContent>

            <TabsContent value="links" className="mt-4">
              <div className="rounded-md border divide-y text-sm font-mono">
                {passedLinks.map((link) => (
                  <LinkRow key={link.link_id} link={link} passed onGroupClick={setGroupDialog} />
                ))}
                {filteredLinks.map((link) => (
                  <LinkRow key={link.link_id} link={link} passed={false} onGroupClick={setGroupDialog} />
                ))}
                {(links ?? []).length === 0 && (
                  <div className="px-3 py-4 text-center text-muted-foreground font-sans">
                    {t("animeSeries.noLinks", "No links for this season.")}
                  </div>
                )}
              </div>
            </TabsContent>
          </Tabs>
        </div>
      </FullScreenDialog>

      {/* Stacked dialog for subtitle group */}
      {groupDialog && (
        <SubtitleGroupDialog
          groupId={groupDialog.id}
          groupName={groupDialog.name}
          open={!!groupDialog}
          onOpenChange={(open) => {
            if (!open) setGroupDialog(null)
          }}
        />
      )}
    </>
  )
}

function InfoItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="text-sm font-medium">{value}</p>
    </div>
  )
}

function EditItem({
  label,
  value,
  onChange,
  type = "text",
}: {
  label: string
  value: string
  onChange: (v: string) => void
  type?: string
}) {
  return (
    <div>
      <p className="text-xs text-muted-foreground mb-1">{label}</p>
      <Input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="h-8 text-sm"
      />
    </div>
  )
}

function LinkRow({
  link,
  passed,
  onGroupClick,
}: {
  link: AnimeLinkRich
  passed: boolean
  onGroupClick: (g: { id: number; name: string }) => void
}) {
  const dl = link.download
  return (
    <div
      className={cn(
        "flex items-center gap-2 px-3 py-2",
        passed
          ? "bg-green-50 text-green-900 dark:bg-green-950/30 dark:text-green-300"
          : "bg-red-50 text-red-900 dark:bg-red-950/30 dark:text-red-300",
      )}
    >
      <span className="shrink-0 w-4 text-center font-bold">{passed ? "+" : "-"}</span>
      <span className="w-12 shrink-0">Ep{link.episode_no}</span>
      <button
        type="button"
        className="shrink-0 underline cursor-pointer hover:opacity-80"
        onClick={() => onGroupClick({ id: link.group_id, name: link.group_name })}
      >
        {link.group_name}
      </button>
      <span className="flex-1 truncate text-xs opacity-70">{link.title ?? ""}</span>
      <span className="shrink-0">
        {passed && dl ? (
          <DownloadBadge status={dl.status} progress={dl.progress} />
        ) : passed ? (
          <Badge variant="outline" className="text-xs">pending</Badge>
        ) : (
          <span className="text-xs opacity-60">filtered</span>
        )}
      </span>
    </div>
  )
}

function DownloadBadge({ status, progress }: { status: string; progress?: number | null }) {
  if (status === "completed") {
    return <Badge className="bg-green-600 text-white text-xs">completed</Badge>
  }
  if (status === "downloading") {
    return (
      <Badge variant="outline" className="text-xs">
        {progress != null ? `${Math.round(progress)}%` : "downloading"}
      </Badge>
    )
  }
  if (status === "failed" || status === "no_downloader") {
    return <Badge variant="destructive" className="text-xs">{status}</Badge>
  }
  return <Badge variant="secondary" className="text-xs">{status}</Badge>
}
