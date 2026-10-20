import { useState } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { ConfirmDialog } from "@/components/shared/ConfirmDialog"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card"
import { File, Folder, Loader2, Search, Trash2 } from "lucide-react"
import { toast } from "sonner"
import type { DownloadOrphan } from "@/schemas/download"
import { formatBytes } from "@/lib/format"

/** 下載目錄清理：列出沒有 torrent 或下載在用的項目，勾選後刪除 */
export function DownloadDirCleanupSection() {
  const { t } = useTranslation()
  const [orphans, setOrphans] = useState<DownloadOrphan[] | null>(null)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [confirming, setConfirming] = useState(false)

  const { mutate: run, isLoading } = useEffectMutation(
    (req: { apply: boolean; names?: string[] }) =>
      Effect.flatMap(CoreApi, (api) => api.downloadOrphans(req)),
  )

  const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e))

  const scan = () =>
    run({ apply: false }).then(
      (res) => {
        setOrphans(res.orphans)
        setSelected(new Set(res.orphans.map((o) => o.name)))
      },
      (e) => toast.error(t("settings.downloadCleanup.scanFailed"), { description: errorText(e) }),
    )

  const remove = () =>
    run({ apply: true, names: [...selected] }).then(
      (res) => {
        setConfirming(false)
        const results = res.results ?? []
        const removed = new Set(results.filter((r) => r.success).map((r) => r.name))
        const failed = results.filter((r) => !r.success)
        toast.success(t("settings.downloadCleanup.deleted", { count: removed.size }))
        failed.forEach((r) => toast.error(r.name, { description: r.error_message ?? undefined }))
        setOrphans((prev) => prev?.filter((o) => !removed.has(o.name)) ?? null)
        setSelected((prev) => new Set([...prev].filter((n) => !removed.has(n))))
      },
      (e) => {
        setConfirming(false)
        toast.error(t("settings.downloadCleanup.deleteFailed"), { description: errorText(e) })
      },
    )

  const toggle = (name: string) =>
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })

  const selectedBytes = (orphans ?? [])
    .filter((o) => selected.has(o.name))
    .reduce((sum, o) => sum + o.size_bytes, 0)

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.downloadCleanup.title")}</CardTitle>
        <CardDescription>{t("settings.downloadCleanup.description")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={scan} disabled={isLoading}>
            {isLoading && !confirming ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <Search className="size-4" />
            )}
            {t("settings.downloadCleanup.scan")}
          </Button>
          {orphans && orphans.length > 0 && (
            <Button
              variant="destructive"
              size="sm"
              onClick={() => setConfirming(true)}
              disabled={isLoading || selected.size === 0}
            >
              <Trash2 className="size-4" />
              {t("settings.downloadCleanup.deleteSelected", {
                count: selected.size,
                size: formatBytes(selectedBytes),
              })}
            </Button>
          )}
        </div>

        {orphans && orphans.length === 0 && (
          <p className="text-sm text-muted-foreground">{t("settings.downloadCleanup.empty")}</p>
        )}

        {orphans && orphans.length > 0 && (
          <ul className="divide-y rounded-md border text-sm">
            {orphans.map((o) => (
              <li key={o.name}>
                <label className="flex cursor-pointer items-center gap-2 px-3 py-2">
                  <input
                    type="checkbox"
                    checked={selected.has(o.name)}
                    onChange={() => toggle(o.name)}
                  />
                  {o.is_dir ? (
                    <Folder className="size-4 shrink-0 text-muted-foreground" />
                  ) : (
                    <File className="size-4 shrink-0 text-muted-foreground" />
                  )}
                  <span className="min-w-0 flex-1 break-all">{o.name}</span>
                  <span className="shrink-0 text-xs text-muted-foreground">
                    {formatBytes(o.size_bytes)} · {new Date(o.modified_at).toLocaleDateString()}
                  </span>
                </label>
              </li>
            ))}
          </ul>
        )}
      </CardContent>

      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={t("settings.downloadCleanup.confirmTitle")}
        description={t("settings.downloadCleanup.confirmDescription", {
          count: selected.size,
          size: formatBytes(selectedBytes),
        })}
        loading={isLoading}
        confirmLabel={t("common.delete", "Delete")}
        confirmLoadingLabel={t("common.deleting", "Deleting...")}
        onConfirm={remove}
      />
    </Card>
  )
}
