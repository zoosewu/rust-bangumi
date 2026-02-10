import { useState } from "react"
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

const STATUSES = ["all", "pending", "parsed", "no_match", "failed", "skipped"]
const PAGE_SIZE = 50

export default function RawItemsPage() {
  const [status, setStatus] = useState("all")
  const [offset, setOffset] = useState(0)

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

  const columns: Column<Record<string, unknown>>[] = [
    {
      key: "item_id",
      header: "ID",
      render: (item) => String(item.item_id),
    },
    {
      key: "title",
      header: "Title",
      render: (item) => (
        <span className="text-sm font-mono truncate max-w-[400px] block">
          {String(item.title)}
        </span>
      ),
    },
    {
      key: "status",
      header: "Status",
      render: (item) => <StatusBadge status={String(item.status)} />,
    },
    {
      key: "subscription_id",
      header: "Sub ID",
      render: (item) => String(item.subscription_id),
    },
    {
      key: "parser_id",
      header: "Parser",
      render: (item) => (item.parser_id != null ? String(item.parser_id) : "-"),
    },
    {
      key: "created_at",
      header: "Created",
      render: (item) => String(item.created_at).slice(0, 19).replace("T", " "),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Raw Items</h1>
        <div className="flex items-center gap-4">
          <Select
            value={status}
            onValueChange={(v) => {
              setStatus(v)
              setOffset(0)
            }}
          >
            <SelectTrigger className="w-[150px]">
              <SelectValue placeholder="Status" />
            </SelectTrigger>
            <SelectContent>
              {STATUSES.map((s) => (
                <SelectItem key={s} value={s}>
                  {s === "all" ? "All Statuses" : s}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      {isLoading ? (
        <p className="text-muted-foreground">Loading...</p>
      ) : (
        <>
          <DataTable
            columns={columns}
            data={(items ?? []) as unknown as Record<string, unknown>[]}
            keyField="item_id"
          />
          <div className="flex items-center justify-between">
            <Button
              variant="outline"
              size="sm"
              disabled={offset === 0}
              onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
            >
              Previous
            </Button>
            <span className="text-sm text-muted-foreground">
              Showing {offset + 1} - {offset + (items?.length ?? 0)}
            </span>
            <Button
              variant="outline"
              size="sm"
              disabled={(items?.length ?? 0) < PAGE_SIZE}
              onClick={() => setOffset(offset + PAGE_SIZE)}
            >
              Next
            </Button>
          </div>
        </>
      )}
    </div>
  )
}
