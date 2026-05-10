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
  neutral: "border-border bg-muted/30 text-muted-foreground",
  muted: "border-slate-500/20 bg-slate-500/8 text-slate-700 dark:text-slate-300",
  info: "border-sky-500/20 bg-sky-500/8 text-slate-700 dark:text-slate-300",
  success: "border-emerald-500/20 bg-emerald-500/8 text-slate-700 dark:text-slate-300",
  warning: "border-amber-500/20 bg-amber-500/8 text-slate-700 dark:text-slate-300",
  danger: "border-red-500/20 bg-red-500/8 text-slate-700 dark:text-slate-300",
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
