import { useState } from "react"
import { NavLink } from "react-router-dom"
import { useTranslation } from "react-i18next"
import { cn } from "@/lib/utils"
import {
  LayoutDashboard,
  Film,
  Rss,
  RefreshCw,
  AlertTriangle,
  Users,
  Library,
  ScanText,
  Filter,
  ChevronDown,
  ChevronRight,
} from "lucide-react"

const mainNavItems = [
  { to: "/", icon: LayoutDashboard, labelKey: "sidebar.dashboard" },
  { to: "/subscriptions", icon: Rss, labelKey: "sidebar.subscriptions" },
  { to: "/anime", icon: Film, labelKey: "sidebar.animeSeries" },
  { to: "/raw-items", icon: RefreshCw, labelKey: "sidebar.rawItems" },
  { to: "/conflicts", icon: AlertTriangle, labelKey: "sidebar.conflicts" },
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
        {mainNavItems.map(({ to, icon: Icon, labelKey }) => (
          <NavLink key={to} to={to} end={to === "/"} className={navLinkClass}>
            <Icon className="h-4 w-4" />
            {t(labelKey)}
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
