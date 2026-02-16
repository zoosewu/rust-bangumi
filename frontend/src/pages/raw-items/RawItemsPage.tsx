import { useState, useMemo } from "react"
import { Link } from "react-router-dom"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { StatusBadge } from "@/components/shared/StatusBadge"
import { DownloadBadge } from "@/components/shared/DownloadBadge"
import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { RawItemDialog } from "./RawItemDialog"

const STATUSES = ["all", "pending", "parsed", "no_match", "failed", "skipped"]
const PAGE_SIZE = 50

export default function RawItemsPage() {
  const { t } = useTranslation()
  const [status, setStatus] = useState("all")
  const [offset, setOffset] = useState(0)
  const [selectedItemId, setSelectedItemId] = useState<number | null>(null)

  const { data: items, isLoading } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getRawItems({
          status: status === "all" ? undefined : status,
          limit: PAGE_SIZE,
          offset,
        })
      }),
    [status, offset],
  )

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
      key: "item_id",
      header: t("common.id"),
      render: (item) => String(item.item_id),
    },
    {
      key: "title",
      header: t("rawItems.itemTitle"),
      render: (item) => (
        <span className="text-sm font-mono truncate max-w-[400px] block">
          {String(item.title)}
        </span>
      ),
    },
    {
      key: "status",
      header: t("common.status"),
      render: (item) => <StatusBadge status={String(item.status)} />,
    },
    {
      key: "download",
      header: t("rawItems.download"),
      render: (item) => {
        const dl = item.download as { status: string; progress: number | null } | null | undefined
        if (!dl) return "-"
        return <DownloadBadge status={dl.status} progress={dl.progress} />
      },
    },
    {
      key: "subscription_id",
      header: t("rawItems.subscriptionSource"),
      render: (item) => {
        const id = Number(item.subscription_id)
        const name = subMap.get(id) ?? `#${id}`
        return (
          <Link
            to="/subscriptions"
            className="text-primary underline cursor-pointer"
            onClick={(e) => e.stopPropagation()}
          >
            {name}
          </Link>
        )
      },
    },
    {
      key: "parser_id",
      header: t("rawItems.parser"),
      render: (item) => {
        if (item.parser_id == null) return "-"
        const id = Number(item.parser_id)
        const name = parserMap.get(id) ?? `#${id}`
        return (
          <Link
            to="/parsers"
            className="text-primary underline cursor-pointer"
            onClick={(e) => e.stopPropagation()}
          >
            {name}
          </Link>
        )
      },
    },
    {
      key: "created_at",
      header: t("rawItems.created"),
      render: (item) => String(item.created_at).slice(0, 19).replace("T", " "),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("rawItems.title")}</h1>
        <div className="flex items-center gap-4">
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
        </div>
      </div>

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
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
