import { cn } from "@/lib/utils"
import { Check, X, Plus, Minus } from "lucide-react"
import { StatusBadge } from "@/components/shared/StatusBadge"
import type { PreviewItem } from "@/schemas/common"

interface PreviewItemWithStatus extends PreviewItem {
  status?: string
}

interface PreviewPanel {
  passed_items: readonly PreviewItemWithStatus[]
  filtered_items: readonly PreviewItemWithStatus[]
}

interface FilterPreviewPanelProps {
  before: PreviewPanel
  after?: PreviewPanel | null
  className?: string
}

type ItemState = "passed" | "filtered" | "newly-passed" | "newly-filtered"

interface MergedItem {
  item_id: number
  title: string
  status?: string
  beforeState: "passed" | "filtered"
  afterState: ItemState
}

function mergeItems(
  before: FilterPreviewPanelProps["before"],
  after?: FilterPreviewPanelProps["after"],
): MergedItem[] {
  const beforePassedSet = new Set(before.passed_items.map((i) => i.item_id))
  const afterPassedSet = after
    ? new Set(after.passed_items.map((i) => i.item_id))
    : null

  const allItems = new Map<number, { title: string; status?: string; beforeState: "passed" | "filtered" }>()
  for (const item of before.passed_items) {
    allItems.set(item.item_id, { title: item.title, status: item.status, beforeState: "passed" })
  }
  for (const item of before.filtered_items) {
    allItems.set(item.item_id, { title: item.title, status: item.status, beforeState: "filtered" })
  }
  // Include items only in after (shouldn't happen normally, but be safe)
  if (after) {
    for (const item of [...after.passed_items, ...after.filtered_items]) {
      if (!allItems.has(item.item_id)) {
        allItems.set(item.item_id, { title: item.title, status: item.status, beforeState: "filtered" })
      }
    }
  }

  const result: MergedItem[] = []
  for (const [item_id, { title, status, beforeState }] of allItems) {
    let afterState: ItemState
    if (!afterPassedSet) {
      afterState = beforeState
    } else if (beforeState === "passed" && afterPassedSet.has(item_id)) {
      afterState = "passed"
    } else if (beforeState === "filtered" && !afterPassedSet.has(item_id)) {
      afterState = "filtered"
    } else if (beforeState === "filtered" && afterPassedSet.has(item_id)) {
      afterState = "newly-passed"
    } else {
      afterState = "newly-filtered"
    }
    result.push({ item_id, title, status, beforeState, afterState })
  }

  // Sort: passed first, then filtered; within each group sort by title
  result.sort((a, b) => {
    const order = { passed: 0, filtered: 1 }
    const ao = order[a.beforeState]
    const bo = order[b.beforeState]
    if (ao !== bo) return ao - bo
    return a.title.localeCompare(b.title)
  })

  return result
}

export function FilterPreviewPanel({
  before,
  after,
  className,
}: FilterPreviewPanelProps) {
  const items = mergeItems(before, after)
  const hasAfter = !!after
  const hasChanges = hasAfter && items.some((i) => i.afterState === "newly-passed" || i.afterState === "newly-filtered")

  if (items.length === 0) {
    return (
      <div className={cn("rounded-md border text-sm", className)}>
        <div className="px-3 py-4 text-center text-muted-foreground">No items</div>
      </div>
    )
  }

  return (
    <div className={cn("rounded-md border text-sm font-mono", className)}>
      {/* Header */}
      {hasAfter && hasChanges && (
        <div className="grid grid-cols-2 divide-x border-b text-xs text-muted-foreground font-sans">
          <div className="px-3 py-1.5">Before</div>
          <div className="px-3 py-1.5">After</div>
        </div>
      )}

      {/* Items */}
      <div className="divide-y">
        {items.map((item) => (
          <FilterPreviewRow
            key={item.item_id}
            item={item}
            showAfter={hasAfter && hasChanges}
          />
        ))}
      </div>
    </div>
  )
}

function FilterPreviewRow({
  item,
  showAfter,
}: {
  item: MergedItem
  showAfter: boolean
}) {
  const beforeCell = (
    <div
      className={cn(
        "flex items-center gap-2 px-3 py-1.5 min-w-0",
        item.beforeState === "passed"
          ? "text-foreground"
          : "text-muted-foreground/60",
      )}
    >
      {item.beforeState === "passed" ? (
        <Check className="h-3.5 w-3.5 shrink-0 text-emerald-500" />
      ) : (
        <X className="h-3.5 w-3.5 shrink-0 text-muted-foreground/40" />
      )}
      <span className="truncate">{item.title}</span>
      {item.status && <StatusBadge status={item.status} />}
    </div>
  )

  if (!showAfter) {
    return beforeCell
  }

  const changed = item.afterState === "newly-passed" || item.afterState === "newly-filtered"
  const afterPassed = item.afterState === "passed" || item.afterState === "newly-passed"

  const afterCell = (
    <div
      className={cn(
        "flex items-center gap-2 px-3 py-1.5 min-w-0",
        !changed && afterPassed && "text-foreground",
        !changed && !afterPassed && "text-muted-foreground/60",
        item.afterState === "newly-passed" &&
          "bg-emerald-50 text-emerald-900 dark:bg-emerald-950/30 dark:text-emerald-300",
        item.afterState === "newly-filtered" &&
          "bg-red-50 text-red-900 dark:bg-red-950/30 dark:text-red-300",
      )}
    >
      {item.afterState === "newly-passed" ? (
        <Plus className="h-3.5 w-3.5 shrink-0" />
      ) : item.afterState === "newly-filtered" ? (
        <Minus className="h-3.5 w-3.5 shrink-0" />
      ) : afterPassed ? (
        <Check className="h-3.5 w-3.5 shrink-0 text-emerald-500" />
      ) : (
        <X className="h-3.5 w-3.5 shrink-0 text-muted-foreground/40" />
      )}
      <span className="truncate">{item.title}</span>
      {item.status && <StatusBadge status={item.status} />}
    </div>
  )

  return (
    <div className="grid grid-cols-2 divide-x">
      {beforeCell}
      {afterCell}
    </div>
  )
}
