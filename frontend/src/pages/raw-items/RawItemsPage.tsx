import { useState, useMemo, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { toast } from "sonner"
import { RotateCw } from "lucide-react"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { StatusBadge } from "@/components/shared/StatusBadge"
import { Badge } from "@/components/ui/badge"
import { DownloadBadge } from "@/components/shared/DownloadBadge"
import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { EntityLink } from "@/components/shared/EntityLink"
import { RawItemDialog } from "./RawItemDialog"
import { PageHeader } from "@/components/shared/PageHeader"
import { SearchBar } from "@/components/shared/SearchBar"
import { TitleCell } from "@/components/shared/TitleCell"
import { formatDateTime } from "@/lib/datetime"

const STATUSES = ["all", "pending", "parsed", "no_match", "failed", "skipped"]
const PAGE_SIZE = 50
const RETRYABLE_DOWNLOAD = new Set(["failed", "downloader_error", "no_downloader"])

export default function RawItemsPage() {
  const { t } = useTranslation()
  const [status, setStatus] = useState("all")
  const [offset, setOffset] = useState(0)
  const [selectedItemId, setSelectedItemId] = useState<number | null>(null)
  const [rawSearch, setRawSearch] = useState("")
  const [debouncedSearch, setDebouncedSearch] = useState("")
  const [refreshKey, setRefreshKey] = useState(0)

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(rawSearch)
      setOffset(0)
    }, 300)
    return () => clearTimeout(timer)
  }, [rawSearch])

  const { data: items, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getRawItems({
          status: status === "all" ? undefined : status,
          limit: PAGE_SIZE,
          offset,
          search: debouncedSearch || undefined,
        })
      }),
    [status, offset, debouncedSearch, refreshKey],
  )

  const retryOne = useEffectMutation((id: number) =>
    Effect.gen(function* () {
      const api = yield* CoreApi
      return yield* api.retryDownload(id)
    }),
  )

  const handleRetry = async (downloadId: number) => {
    try {
      const r = await retryOne.mutate(downloadId)
      if (r.status === "dispatched") {
        toast.success(t("rawItems.retrySuccess"))
      } else {
        toast.warning(t("rawItems.retryNoDownloader"))
      }
      setRefreshKey((k) => k + 1)
    } catch {
      toast.error(t("rawItems.retryFailed"))
    }
  }

  const { data: subscriptions } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getSubscriptions
      }),
    [],
  )

  const { data: parsers } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getParsers()
      }),
    [],
  )

  const subMap = useMemo(() => {
    const m = new Map<number, string>()
    for (const s of subscriptions ?? []) {
      m.set(s.subscription_id, s.name ?? `#${s.subscription_id}`)
    }
    return m
  }, [subscriptions])

  const parserMap = useMemo(() => {
    const m = new Map<number, string>()
    for (const p of parsers ?? []) {
      m.set(p.parser_id, p.name)
    }
    return m
  }, [parsers])

  const columns: Column<Record<string, unknown>>[] = [
    {
      key: "retry",
      header: t("rawItems.retry"),
      headClassName: "w-20",
      cellClassName: "w-20",
      render: (item) => {
        const dl = item.download as { download_id: number; status: string; progress: number | null } | null | undefined
        if (!dl || !RETRYABLE_DOWNLOAD.has(dl.status)) return "-"
        return (
          <Button
            variant="outline"
            size="sm"
            className="h-6 px-2"
            disabled={retryOne.isLoading}
            onClick={(e) => {
              e.stopPropagation()
              handleRetry(dl.download_id)
            }}
          >
            <RotateCw className="h-3 w-3 mr-1" />
            {t("rawItems.retry")}
          </Button>
        )
      },
    },
    {
      key: "download",
      header: t("rawItems.download"),
      render: (item) => {
        const dl = item.download as { download_id: number; status: string; progress: number | null } | null | undefined
        if (!dl) return "-"
        return <DownloadBadge status={dl.status} progress={dl.progress} />
      },
    },
    {
      key: "title",
      header: t("rawItems.itemTitle"),
      headClassName: "min-w-[520px]",
      cellClassName: "min-w-[520px]",
      render: (item) => <TitleCell value={String(item.title)} mono />,
    },
    {
      key: "status",
      header: t("common.status"),
      render: (item) => (
        <div className="flex items-center gap-1.5">
          {Boolean(item.conflict_flag) ? (
            <Badge variant="outline" className="text-xs px-1.5 py-0 bg-amber-100 text-amber-800 dark:bg-amber-950/40 dark:text-amber-300 border-amber-200 dark:border-amber-800">
              {t("rawItems.conflict")}
            </Badge>
          ) : (
            <StatusBadge status={String(item.status)} />
          )}
        </div>
      ),
    },
    {
      key: "subscription_id",
      header: t("rawItems.subscriptionSource"),
      render: (item) => {
        const id = Number(item.subscription_id)
        return (
          <EntityLink
            type="subscription"
            id={id}
            name={subMap.get(id)}
          />
        )
      },
    },
    {
      key: "parser_id",
      header: t("rawItems.parser"),
      render: (item) => {
        if (item.parser_id == null) return "-"
        const id = Number(item.parser_id)
        return (
          <EntityLink
            type="parser"
            id={id}
            name={parserMap.get(id)}
          />
        )
      },
    },
    {
      key: "created_at",
      header: t("rawItems.created"),
      render: (item) => formatDateTime(String(item.created_at)),
    },
  ]

  return (
    <div className="space-y-6">
      <PageHeader
        title={t("rawItems.title")}
        actions={
          <Select
            value={status}
            onValueChange={(v) => {
              setStatus(v)
              setOffset(0)
            }}
          >
            <SelectTrigger className="w-[150px]">
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
        }
      />

      <SearchBar value={rawSearch} onChange={setRawSearch} />

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : debouncedSearch && (items ?? []).length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("common.noResults")}</p>
      ) : (
        <>
          <DataTable
            columns={columns}
            data={(items ?? []) as unknown as Record<string, unknown>[]}
            keyField="item_id"
            onRowClick={(row) => setSelectedItemId(Number(row.item_id))}
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
              {t("common.showing", { from: offset + 1, to: offset + (items?.length ?? 0) })}
            </span>
            <Button
              variant="outline"
              size="sm"
              disabled={(items?.length ?? 0) < PAGE_SIZE}
              onClick={() => setOffset(offset + PAGE_SIZE)}
            >
              {t("common.next")}
            </Button>
          </div>
        </>
      )}

      {selectedItemId != null && (
        <RawItemDialog
          itemId={selectedItemId}
          open={selectedItemId != null}
          onOpenChange={(open) => {
            if (!open) setSelectedItemId(null)
          }}
          subMap={subMap}
          parserMap={parserMap}
        />
      )}
    </div>
  )
}
