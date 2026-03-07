import * as React from "react"
import { XIcon } from "lucide-react"
import { Dialog as DialogPrimitive } from "radix-ui"
import { cn } from "@/lib/utils"
import { ScrollArea } from "@/components/ui/scroll-area"

type DialogSize = "full" | "md" | "sm"

interface FullScreenDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description?: React.ReactNode
  children: React.ReactNode
  footer?: React.ReactNode
  subHeader?: React.ReactNode
  size?: DialogSize
}

const sizeClasses: Record<DialogSize, string> = {
  full: "inset-4",
  md: "inset-[5vh_auto] left-1/2 -translate-x-1/2 w-full max-w-3xl max-h-[90vh]",
  sm: "inset-[15vh_auto] left-1/2 -translate-x-1/2 w-full max-w-md max-h-[70vh]",
}

export function FullScreenDialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  subHeader,
  size = "full",
}: FullScreenDialogProps) {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay
          className={cn(
            "fixed inset-0 z-50 bg-black/50",
            "data-[state=open]:animate-in data-[state=closed]:animate-out",
            "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
          )}
        />
        <DialogPrimitive.Content
          className={cn(
            "fixed z-50 flex flex-col rounded-lg border bg-background shadow-lg outline-none",
            "data-[state=open]:animate-in data-[state=closed]:animate-out",
            "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
            "data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
            sizeClasses[size],
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b px-6 py-4 shrink-0">
            <div>
              <DialogPrimitive.Title className="text-lg font-semibold">
                {title}
              </DialogPrimitive.Title>
              {description && (
                <DialogPrimitive.Description className="text-sm text-muted-foreground mt-0.5">
                  {description}
                </DialogPrimitive.Description>
              )}
            </div>
            <DialogPrimitive.Close className="rounded-sm opacity-70 transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring">
              <XIcon className="h-5 w-5" />
              <span className="sr-only">Close</span>
            </DialogPrimitive.Close>
          </div>

          {/* Sub-header (e.g. step indicator) */}
          {subHeader && (
            <div className="shrink-0 border-b px-6 py-3">
              {subHeader}
            </div>
          )}

          {/* Body */}
          <ScrollArea className="flex-1 min-h-0 px-6 py-4">
            {children}
          </ScrollArea>

          {/* Footer */}
          {footer && (
            <div className="shrink-0 border-t px-6 py-4 flex justify-end gap-2">
              {footer}
            </div>
          )}
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}
