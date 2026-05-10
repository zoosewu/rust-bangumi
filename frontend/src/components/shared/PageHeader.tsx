import type { ReactNode } from "react"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"

interface PageHeaderProps {
  title: string
  badge?: ReactNode
  actions?: ReactNode
}

export function PageHeader({ title, badge, actions }: PageHeaderProps) {
  return (
    <div className="flex items-center justify-between min-h-9">
      <div className="flex min-w-0 items-center gap-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <h1 className="max-w-[720px] truncate text-2xl font-bold">{title}</h1>
          </TooltipTrigger>
          <TooltipContent className="max-w-[720px] break-words whitespace-normal">
            {title}
          </TooltipContent>
        </Tooltip>
        {badge}
      </div>
      {actions}
    </div>
  )
}
