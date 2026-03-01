import type { ReactNode } from "react"

interface PageHeaderProps {
  title: string
  badge?: ReactNode
  actions?: ReactNode
}

export function PageHeader({ title, badge, actions }: PageHeaderProps) {
  return (
    <div className="flex items-center justify-between min-h-9">
      <div className="flex items-center gap-2">
        <h1 className="text-2xl font-bold">{title}</h1>
        {badge}
      </div>
      {actions}
    </div>
  )
}
