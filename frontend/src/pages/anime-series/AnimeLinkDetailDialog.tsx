import { useTranslation } from "react-i18next"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Badge } from "@/components/ui/badge"
import { CopyButton } from "@/components/shared/CopyButton"
import type { AnimeLinkRich } from "@/schemas/anime"

interface AnimeLinkDetailDialogProps {
  link: AnimeLinkRich
  allLinks: readonly AnimeLinkRich[]
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function AnimeLinkDetailDialog({
  link,
  allLinks,
  open,
  onOpenChange,
}: AnimeLinkDetailDialogProps) {
  const { t } = useTranslation()

  const conflictingLinks = allLinks.filter((l) =>
    link.conflicting_link_ids.includes(l.link_id)
  )

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>
            {t("animeLink.detail", "Anime Link Detail")}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 text-sm">
          {/* This link info */}
          <div className="space-y-1">
            <p className="text-xs text-muted-foreground font-sans">
              {t("animeLink.thisLink", "This Link")}
            </p>
            <div className="rounded border p-2 font-mono text-xs space-y-1">
              <div className="flex items-center gap-2">
                <span className="text-muted-foreground">Ep{link.episode_no}</span>
                <span className="font-semibold">{link.group_name}</span>
                {link.conflict_flag && (
                  <Badge variant="destructive" className="text-xs">
                    {t("animeLink.conflict", "Conflict")}
                  </Badge>
                )}
              </div>
              <p className="opacity-70 truncate">{link.title ?? "-"}</p>
              <div className="flex items-center gap-1">
                <span className="truncate opacity-60">{link.url}</span>
                <CopyButton text={link.url} />
              </div>
            </div>
          </div>

          {/* Conflicting links */}
          {conflictingLinks.length > 0 && (
            <div className="space-y-1">
              <p className="text-xs text-muted-foreground font-sans">
                {t("animeLink.conflictsWith", "Conflicts With")}
              </p>
              <div className="rounded border divide-y font-mono text-xs">
                {conflictingLinks.map((cl) => (
                  <div key={cl.link_id} className="p-2 space-y-1">
                    <div className="flex items-center gap-2">
                      <span className="text-muted-foreground">Ep{cl.episode_no}</span>
                      <span className="font-semibold">{cl.group_name}</span>
                    </div>
                    <p className="opacity-70 truncate">{cl.title ?? "-"}</p>
                    <div className="flex items-center gap-1">
                      <span className="truncate opacity-60">{cl.url}</span>
                      <CopyButton text={cl.url} />
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
