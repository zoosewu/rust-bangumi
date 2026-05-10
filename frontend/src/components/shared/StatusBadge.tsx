import { useTranslation } from "react-i18next"
import { TagBadge, type TagBadgeTone } from "@/components/shared/TagBadge"

const statusTone: Record<string, TagBadgeTone> = {
  pending: "warning",
  parsed: "success",
  no_match: "muted",
  failed: "danger",
  skipped: "info",
  eliminated: "muted",
  conflict: "warning",
}

const statusLabelKey: Record<string, string> = {
  pending: "tags.status.pending",
  parsed: "tags.status.parsed",
  no_match: "tags.status.noMatch",
  failed: "tags.status.failed",
  skipped: "tags.status.skipped",
  eliminated: "tags.status.eliminated",
  conflict: "tags.status.conflict",
}

export function StatusBadge({ status }: { status: string }) {
  const { t } = useTranslation()
  const labelKey = statusLabelKey[status]

  return (
    <TagBadge tone={statusTone[status] ?? "neutral"}>
      {labelKey ? t(labelKey) : status}
    </TagBadge>
  )
}
