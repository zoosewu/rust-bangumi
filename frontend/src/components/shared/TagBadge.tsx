import type { ComponentProps } from "react"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

export type TagBadgeTone =
  | "neutral"
  | "muted"
  | "info"
  | "success"
  | "warning"
  | "danger"

const toneClasses: Record<TagBadgeTone, string> = {
  neutral: "border-border bg-background text-foreground",
  muted: "border-slate-200 bg-slate-100 text-slate-700 dark:border-slate-800 dark:bg-slate-900/40 dark:text-slate-300",
  info: "border-sky-200 bg-sky-100 text-sky-800 dark:border-sky-800 dark:bg-sky-950/40 dark:text-sky-300",
  success: "border-emerald-200 bg-emerald-100 text-emerald-800 dark:border-emerald-800 dark:bg-emerald-950/40 dark:text-emerald-300",
  warning: "border-amber-200 bg-amber-100 text-amber-800 dark:border-amber-800 dark:bg-amber-950/40 dark:text-amber-300",
  danger: "border-red-200 bg-red-100 text-red-800 dark:border-red-800 dark:bg-red-950/40 dark:text-red-300",
}

interface TagBadgeProps extends Omit<ComponentProps<typeof Badge>, "variant"> {
  tone?: TagBadgeTone
}

export function TagBadge({
  tone = "neutral",
  className,
  ...props
}: TagBadgeProps) {
  return (
    <Badge
      variant="outline"
      className={cn("h-5 rounded-md px-1.5 py-0 text-xs leading-none", toneClasses[tone], className)}
      {...props}
    />
  )
}
