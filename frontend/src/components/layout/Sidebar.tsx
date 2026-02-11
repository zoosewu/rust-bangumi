import { NavLink } from "react-router-dom"
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
} from "lucide-react"

const navItems = [
  { to: "/", icon: LayoutDashboard, label: "Dashboard" },
  { to: "/anime", icon: Film, label: "Anime" },
  { to: "/subscriptions", icon: Rss, label: "Subscriptions" },
  { to: "/raw-items", icon: FileText, label: "Raw Items" },
  { to: "/downloads", icon: Download, label: "Downloads" },
  { to: "/filters", icon: Filter, label: "Filters" },
  { to: "/parsers", icon: FileCode, label: "Parsers" },
  { to: "/conflicts", icon: AlertTriangle, label: "Conflicts" },
]

export function Sidebar() {
  return (
    <aside className="w-60 border-r bg-card h-screen sticky top-0 flex flex-col">
      <div className="p-4 font-bold text-lg border-b">Bangumi</div>
      <nav className="flex-1 p-2 space-y-1">
        {navItems.map(({ to, icon: Icon, label }) => (
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
            {label}
          </NavLink>
        ))}
      </nav>
    </aside>
  )
}
