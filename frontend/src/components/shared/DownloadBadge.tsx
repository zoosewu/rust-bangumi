import { Badge } from "@/components/ui/badge"

interface DownloadBadgeProps {
  status: string
  progress?: number | null
}

export function DownloadBadge({ status, progress }: DownloadBadgeProps) {
  if (status === "completed") {
    return <Badge className="bg-green-600 text-white text-xs">completed</Badge>
  }
  if (status === "downloading") {
    return (
      <Badge variant="outline" className="text-xs">
        {progress != null ? `${Math.round(progress)}%` : "downloading"}
      </Badge>
    )
  }
  if (status === "failed" || status === "no_downloader") {
    return <Badge variant="destructive" className="text-xs">{status}</Badge>
  }
  return <Badge variant="secondary" className="text-xs">{status}</Badge>
}
