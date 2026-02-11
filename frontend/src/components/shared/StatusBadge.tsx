import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

const statusColors: Record<string, string> = {
  pending: "bg-yellow-100 text-yellow-800",
  parsed: "bg-green-100 text-green-800",
  no_match: "bg-gray-100 text-gray-800",
  failed: "bg-red-100 text-red-800",
  skipped: "bg-blue-100 text-blue-800",
}

export function StatusBadge({ status }: { status: string }) {
  return (
    <Badge variant="outline" className={cn("text-xs", statusColors[status])}>
      {status}
    </Badge>
  )
}
