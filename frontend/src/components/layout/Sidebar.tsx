import { NavLink } from "react-router-dom"
import { useTranslation } from "react-i18next"
import { cn } from "@/lib/utils"
import {
  LayoutDashboard,
  Film,
  Rss,
  FileText,
  Download,
  Filter,
  FileCode,
  AlertTriangle,
  Users,
} from "lucide-react"

const navItems = [
  { to: "/", icon: LayoutDashboard, labelKey: "sidebar.dashboard" },
  { to: "/anime", icon: Film, labelKey: "sidebar.anime" },
  { to: "/subtitle-groups", icon: Users, labelKey: "sidebar.subtitleGroups" },
  { to: "/subscriptions", icon: Rss, labelKey: "sidebar.subscriptions" },
  { to: "/raw-items", icon: FileText, labelKey: "sidebar.rawItems" },
  { to: "/downloads", icon: Download, labelKey: "sidebar.downloads" },
  { to: "/filters", icon: Filter, labelKey: "sidebar.filters" },
  { to: "/parsers", icon: FileCode, labelKey: "sidebar.parsers" },
  { to: "/conflicts", icon: AlertTriangle, labelKey: "sidebar.conflicts" },
]

export function Sidebar() {
  const { t } = useTranslation()

  return (
    <aside className="w-60 border-r bg-card h-screen sticky top-0 flex flex-col">
      <div className="p-4 font-bold text-lg border-b">{t("sidebar.title")}</div>
      <nav className="flex-1 p-2 space-y-1">
        {navItems.map(({ to, icon: Icon, labelKey }) => (
          <NavLink
            key={to}
            to={to}
            end={to === "/"}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors",
                isActive
                  ? "bg-primary text-primary-foreground"
                  : "hover:bg-accent text-muted-foreground hover:text-foreground",
              )
            }
          >
            <Icon className="h-4 w-4" />
            {t(labelKey)}
          </NavLink>
        ))}
      </nav>
    </aside>
  )
}
