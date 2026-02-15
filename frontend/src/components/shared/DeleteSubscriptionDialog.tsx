import { useTranslation } from "react-i18next"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { AlertTriangle } from "lucide-react"

interface DeleteSubscriptionDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  subscriptionName: string
  onDeactivate: () => void
  onPurge: () => void
  loading?: boolean
}

export function DeleteSubscriptionDialog({
  open,
  onOpenChange,
  subscriptionName,
  onDeactivate,
  onPurge,
  loading,
}: DeleteSubscriptionDialogProps) {
  const { t } = useTranslation()

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("subscriptions.deleteSubscription")}</DialogTitle>
          <DialogDescription>
            {subscriptionName}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          <div className="rounded-md border p-3 space-y-1">
            <p className="text-sm text-muted-foreground">
              {t("subscriptions.deactivateDesc")}
            </p>
            <Button
              variant="outline"
              className="w-full"
              onClick={onDeactivate}
              disabled={loading}
            >
              {t("subscriptions.deactivate")}
            </Button>
          </div>

          <div className="rounded-md border border-destructive/30 bg-destructive/5 p-3 space-y-1">
            <div className="flex items-start gap-2">
              <AlertTriangle className="h-4 w-4 text-destructive mt-0.5 shrink-0" />
              <p className="text-sm text-destructive">
                {t("subscriptions.purgeDesc")}
              </p>
            </div>
            <Button
              variant="destructive"
              className="w-full"
              onClick={onPurge}
              disabled={loading}
            >
              {t("subscriptions.purgeDelete")}
            </Button>
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)} disabled={loading}>
            {t("common.cancel")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
