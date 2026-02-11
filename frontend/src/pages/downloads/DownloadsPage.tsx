import { useState, useEffect, useRef } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { StatusBadge } from "@/components/shared/StatusBadge"
import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Label } from "@/components/ui/label"

const STATUSES = ["all", "downloading", "completed", "failed", "no_downloader", "downloader_error", "recovery"]
const PAGE_SIZE = 50

function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || bytes === 0) return "-"
  const units = ["B", "KB", "MB", "GB"]
  let i = 0
  let b = bytes
  while (b >= 1024 && i < units.length - 1) {
    b /= 1024
    i++
  }
  return `${b.toFixed(1)} ${units[i]}`
}

export default function DownloadsPage() {
  const { t } = useTranslation()
  const [status, setStatus] = useState("all")
  const [offset, setOffset] = useState(0)
  const [autoRefresh, setAutoRefresh] = useState(true)

  const { data: downloads, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getDownloads({
          status: status === "all" ? undefined : status,
          limit: PAGE_SIZE,
          offset,
        })
      }),
    [status, offset],
  )

  // Auto-refresh every 5 seconds
  const refetchRef = useRef(refetch)
  refetchRef.current = refetch
  useEffect(() => {
    if (!autoRefresh) return
    const interval = setInterval(() => refetchRef.current(), 5000)
    return () => clearInterval(interval)
  }, [autoRefresh])

  const columns: Column<Record<string, unknown>>[] = [
    {
      key: "download_id",
      header: t("common.id"),
      render: (item) => String(item.download_id),
    },
    {
      key: "title",
      header: t("rawItems.itemTitle"),
      render: (item) => (
        <span className="text-sm font-mono truncate max-w-[300px] block">
          {String(item.title ?? `Link #${item.link_id}`)}
        </span>
      ),
    },
    {
      key: "link_id",
      header: "Link ID",
      render: (item) => (
        <span className="text-sm text-muted-foreground">
          #{String(item.link_id)}
        </span>
      ),
    },
    {
      key: "status",
      header: t("common.status"),
      render: (item) => <StatusBadge status={String(item.status)} />,
    },
    {
      key: "progress",
      header: t("downloads.progress"),
      render: (item) => {
        const progress = item.progress as number | null
        if (progress == null) return "-"
        return (
          <div className="flex items-center gap-2 min-w-[120px]">
            <div className="flex-1 bg-gray-200 rounded-full h-2">
              <div
                className="bg-blue-600 h-2 rounded-full transition-all"
                style={{ width: `${Math.min(100, progress * 100)}%` }}
              />
            </div>
            <span className="text-xs text-muted-foreground w-10 text-right">
              {(progress * 100).toFixed(0)}%
            </span>
          </div>
        )
      },
    },
    {
      key: "size",
      header: t("downloads.size"),
      render: (item) => {
        const dl = item.downloaded_bytes as number | null
        const total = item.total_bytes as number | null
        if (total) return `${formatBytes(dl)} / ${formatBytes(total)}`
        if (dl) return formatBytes(dl)
        return "-"
      },
    },
    {
      key: "downloader_type",
      header: t("downloads.downloaderType"),
      render: (item) => String(item.downloader_type),
    },
    {
      key: "updated_at",
      header: t("downloads.updated"),
      render: (item) => String(item.updated_at).slice(0, 19).replace("T", " "),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("downloads.title")}</h1>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <Switch checked={autoRefresh} onCheckedChange={setAutoRefresh} />
            <Label className="text-sm">{t("downloads.autoRefresh")}</Label>
          </div>
          <Select
            value={status}
            onValueChange={(v) => {
              setStatus(v)
              setOffset(0)
            }}
          >
            <SelectTrigger className="w-[180px]">
              <SelectValue placeholder={t("common.status")} />
            </SelectTrigger>
            <SelectContent>
              {STATUSES.map((s) => (
                <SelectItem key={s} value={s}>
                  {s === "all" ? t("common.allStatuses") : s}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <>
          <DataTable
            columns={columns}
            data={(downloads ?? []) as unknown as Record<string, unknown>[]}
            keyField="download_id"
          />
          <div className="flex items-center justify-between">
            <Button
              variant="outline"
              size="sm"
              disabled={offset === 0}
              onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
            >
              {t("common.previous")}
            </Button>
            <span className="text-sm text-muted-foreground">
              {t("common.showing", { from: offset + 1, to: offset + (downloads?.length ?? 0) })}
            </span>
            <Button
              variant="outline"
              size="sm"
              disabled={(downloads?.length ?? 0) < PAGE_SIZE}
              onClick={() => setOffset(offset + PAGE_SIZE)}
            >
              {t("common.next")}
            </Button>
          </div>
        </>
      )}
    </div>
  )
}
