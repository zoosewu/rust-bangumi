import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { Button } from "@/components/ui/button"

interface ConfirmDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description: string
  onConfirm: () => void
  loading?: boolean
  confirmLabel?: string
  confirmLoadingLabel?: string
  confirmVariant?: "destructive" | "default"
  children?: React.ReactNode
  size?: "full" | "md" | "sm"
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  onConfirm,
  loading,
  confirmLabel = "Confirm",
  confirmLoadingLabel,
  confirmVariant = "destructive",
  children,
  size = "sm",
}: ConfirmDialogProps) {
  return (
    <FullScreenDialog
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      description={description}
      size={size}
      footer={
        <>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={loading}>
            Cancel
          </Button>
          <Button variant={confirmVariant} onClick={onConfirm} disabled={loading}>
            {loading ? (confirmLoadingLabel ?? `${confirmLabel}...`) : confirmLabel}
          </Button>
        </>
      }
    >
      {children ?? <div />}
    </FullScreenDialog>
  )
}
