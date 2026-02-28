import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Trash2 } from "lucide-react"
import type { FilterRule } from "@/schemas/filter"

interface FilterRuleListProps {
  rules: readonly FilterRule[]
  onDeleteClick: (rule: FilterRule) => void
}

export function FilterRuleList({ rules, onDeleteClick }: FilterRuleListProps) {
  if (!rules.length) return null

  return (
    <div className="space-y-2">
      {rules.map((rule) => (
        <div
          key={rule.rule_id}
          className="flex items-center gap-2 rounded-md border px-3 py-2 text-sm"
        >
          <Badge variant={rule.is_positive ? "default" : "destructive"}>
            {rule.is_positive ? "include" : "exclude"}
          </Badge>
          <code className="flex-1 font-mono text-xs">{rule.regex_pattern}</code>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={() => onDeleteClick(rule)}
          >
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>
      ))}
    </div>
  )
}
