import { useState, useEffect } from "react"
import { NavLink } from "react-router-dom"
import { useTranslation } from "react-i18next"
import { Effect } from "effect"
import { cn } from "@/lib/utils"
import { CoreApi } from "@/services/CoreApi"
import { AppRuntime } from "@/runtime/AppRuntime"
import {
  LayoutDashboard,
  Film,
  Rss,
  RefreshCw,
  Clock,
  Users,
  Library,
  ScanText,
  Filter,
  ChevronDown,
  ChevronRight,
  Search,
  Settings,
} from "lucide-react"

const mainNavItems = [
  { to: "/", icon: LayoutDashboard, labelKey: "sidebar.dashboard", hasBadge: false },
  { to: "/subscriptions", icon: Rss, labelKey: "sidebar.subscriptions", hasBadge: false },
  { to: "/search", icon: Search, labelKey: "sidebar.search", hasBadge: false },
  { to: "/anime", icon: Film, labelKey: "sidebar.animeSeries", hasBadge: false },
  { to: "/raw-items", icon: RefreshCw, labelKey: "sidebar.rawItems", hasBadge: false },
  { to: "/pending", icon: Clock, labelKey: "sidebar.pending", hasBadge: true },
  { to: "/settings", icon: Settings, labelKey: "sidebar.settings", hasBadge: false },
]

const otherNavItems = [
  { to: "/anime-works", icon: Library, labelKey: "sidebar.anime" },
  { to: "/subtitle-groups", icon: Users, labelKey: "sidebar.subtitleGroups" },
  { to: "/parsers", icon: ScanText, labelKey: "sidebar.parsers" },
  { to: "/filters", icon: Filter, labelKey: "sidebar.filters" },
]

const STORAGE_KEY = "sidebar.others.expanded"

export function Sidebar() {
  const { t } = useTranslation()
  const [pendingCount, setPendingCount] = useState(0)

  useEffect(() => {
    const fetchCount = () => {
      AppRuntime.runPromise(
        Effect.flatMap(CoreApi, (api) => api.getPendingAiResults({ status: "pending" })),
      ).then((items) => setPendingCount(items.length)).catch(() => {})
    }
    fetchCount()
    const id = setInterval(fetchCount, 30000)
    return () => clearInterval(id)
  }, [])

  const [othersExpanded, setOthersExpanded] = useState(() => {
    try {
      return localStorage.getItem(STORAGE_KEY) !== "false"
    } catch {
      return true
    }
  })

  const toggleOthers = () => {
    setOthersExpanded((prev) => {
      const next = !prev
      try { localStorage.setItem(STORAGE_KEY, String(next)) } catch {}
      return next
    })
  }

  const navLinkClass = ({ isActive }: { isActive: boolean }) =>
    cn(
      "flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors",
      isActive
        ? "bg-primary text-primary-foreground"
        : "hover:bg-accent text-muted-foreground hover:text-foreground",
    )

  return (
    <aside className="w-60 border-r bg-card h-screen sticky top-0 flex flex-col">
      <div className="p-4 font-bold text-lg border-b">{t("sidebar.title")}</div>
      <nav className="flex-1 p-2 space-y-1 overflow-y-auto">
        {/* Main nav */}
        {mainNavItems.map(({ to, icon: Icon, labelKey, hasBadge }) => (
          <NavLink key={to} to={to} end={to === "/"} className={navLinkClass}>
            <Icon className="h-4 w-4" />
            <span className="flex-1">{t(labelKey)}</span>
            {hasBadge && pendingCount > 0 && (
              <span className="bg-primary/15 text-primary text-xs font-medium rounded-full px-1.5 min-w-[1.25rem] text-center leading-5">
                {pendingCount > 99 ? "99+" : pendingCount}
              </span>
            )}
          </NavLink>
        ))}

        {/* Divider + Others toggle */}
        <div className="pt-2 pb-1">
          <button
            type="button"
            onClick={toggleOthers}
            className="flex items-center gap-2 w-full px-3 py-1 text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
          >
            {othersExpanded ? (
              <ChevronDown className="h-3 w-3" />
            ) : (
              <ChevronRight className="h-3 w-3" />
            )}
            {t("sidebar.others")}
          </button>
        </div>

        {/* Others (collapsible) */}
        {othersExpanded &&
          otherNavItems.map(({ to, icon: Icon, labelKey }) => (
            <NavLink key={to} to={to} className={navLinkClass}>
              <Icon className="h-4 w-4" />
              {t(labelKey)}
            </NavLink>
          ))}
      </nav>
    </aside>
  )
}
