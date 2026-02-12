import { cn } from "@/lib/utils"

interface DiffItem {
  id: string | number
  label: React.ReactNode
  passed: boolean
  extra?: React.ReactNode
}

interface DiffListProps {
  items: DiffItem[]
  className?: string
}

export function DiffList({ items, className }: DiffListProps) {
  return (
    <div className={cn("rounded-md border divide-y text-sm font-mono", className)}>
      {items.map((item) => (
        <div
          key={item.id}
          className={cn(
            "flex items-center gap-2 px-3 py-2",
            item.passed
              ? "bg-green-50 text-green-900 dark:bg-green-950/30 dark:text-green-300"
              : "bg-red-50 text-red-900 dark:bg-red-950/30 dark:text-red-300",
          )}
        >
          <span className="shrink-0 w-4 text-center font-bold">
            {item.passed ? "+" : "-"}
          </span>
          <span className="flex-1 truncate">{item.label}</span>
          {item.extra && <span className="shrink-0">{item.extra}</span>}
        </div>
      ))}
      {items.length === 0 && (
        <div className="px-3 py-4 text-center text-muted-foreground">
          No items
        </div>
      )}
    </div>
  )
}
