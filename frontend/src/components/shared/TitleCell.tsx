import { cn } from "@/lib/utils"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"

interface TitleCellProps {
  value: string
  className?: string
  mono?: boolean
}

export function TitleCell({ value, className, mono = false }: TitleCellProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          className={cn(
            "block max-w-[640px] truncate",
            mono ? "font-mono text-sm" : "font-medium",
            className,
          )}
        >
          {value}
        </span>
      </TooltipTrigger>
      <TooltipContent className="max-w-[720px] break-words whitespace-normal">
        {value}
      </TooltipContent>
    </Tooltip>
  )
}
