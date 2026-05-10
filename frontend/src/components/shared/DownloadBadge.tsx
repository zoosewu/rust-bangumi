import { useTranslation } from "react-i18next"
import { TagBadge, type TagBadgeTone } from "@/components/shared/TagBadge"

interface DownloadBadgeProps {
  status: string
  progress?: number | null
}

const downloadTone: Record<string, TagBadgeTone> = {
  completed: "success",
  downloading: "info",
  failed: "danger",
  downloader_error: "danger",
  no_downloader: "danger",
  cancelled: "muted",
  pending: "warning",
}

const downloadLabelKey: Record<string, string> = {
  completed: "tags.download.completed",
  downloading: "tags.download.downloading",
  failed: "tags.download.failed",
  downloader_error: "tags.download.downloaderError",
  no_downloader: "tags.download.noDownloader",
  cancelled: "tags.download.cancelled",
  pending: "tags.download.pending",
}

export function DownloadBadge({ status, progress }: DownloadBadgeProps) {
  const { t } = useTranslation()

  if (status === "downloading") {
    return (
      <TagBadge tone="info">
        {progress != null ? `${Math.round(progress)}%` : t("tags.download.downloading")}
      </TagBadge>
    )
  }

  const labelKey = downloadLabelKey[status]
  return (
    <TagBadge tone={downloadTone[status] ?? "neutral"}>
      {labelKey ? t(labelKey) : status}
    </TagBadge>
  )
}
