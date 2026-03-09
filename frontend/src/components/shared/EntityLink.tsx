import { Link } from "react-router-dom"
import { cn } from "@/lib/utils"

/** 各實體類型對應的導航路由 */
const ENTITY_ROUTES: Record<string, string> = {
  anime_work: "/anime-works",
  anime: "/anime",
  subtitle_group: "/subtitle-groups",
  fetcher: "/subscriptions",
  subscription: "/subscriptions",
  parser: "/parsers",
}

interface EntityLinkProps {
  /** 實體類型，傳入 "global" 或 null/undefined 時顯示 Global 標籤 */
  type: string | null | undefined
  id?: number | null
  name?: string | null
  /**
   * 自訂點擊行為（e.g., 開啟 Dialog）。
   * 提供時不進行路由導航，改為呼叫此 callback。
   */
  onClick?: () => void
  className?: string
}

/**
 * 統一的實體來源連結元件。
 * - global：顯示靜態「Global」標籤
 * - 其他類型：預設導航至對應路由，傳入 onClick 時改為自訂行為
 */
export function EntityLink({ type, id, name, onClick, className }: EntityLinkProps) {
  if (!type || type === "global") {
    return (
      <span className={cn("text-xs text-muted-foreground", className)}>
        Global
      </span>
    )
  }

  const label = name ?? (id != null ? `#${id}` : "-")
  const baseClass = cn(
    "text-xs text-primary underline-offset-2 hover:underline cursor-pointer",
    className,
  )

  if (onClick) {
    return (
      <button
        type="button"
        className={baseClass}
        onClick={(e) => {
          e.stopPropagation()
          onClick()
        }}
      >
        {label}
      </button>
    )
  }

  const route = ENTITY_ROUTES[type]
  if (route) {
    return (
      <Link
        to={route}
        className={baseClass}
        onClick={(e) => e.stopPropagation()}
      >
        {label}
      </Link>
    )
  }

  return <span className={cn("text-xs", className)}>{label}</span>
}
