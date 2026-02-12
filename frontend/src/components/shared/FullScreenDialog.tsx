import * as React from "react"
import { XIcon } from "lucide-react"
import { Dialog as DialogPrimitive } from "radix-ui"
import { cn } from "@/lib/utils"
import { ScrollArea } from "@/components/ui/scroll-area"

interface FullScreenDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  children: React.ReactNode
}

export function FullScreenDialog({
  open,
  onOpenChange,
  title,
  children,
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
            "fixed inset-4 z-50 flex flex-col rounded-lg border bg-background shadow-lg outline-none",
            "data-[state=open]:animate-in data-[state=closed]:animate-out",
            "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
            "data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
          )}
        >
          <div className="flex items-center justify-between border-b px-6 py-4">
            <DialogPrimitive.Title className="text-lg font-semibold">
              {title}
            </DialogPrimitive.Title>
            <DialogPrimitive.Close className="rounded-sm opacity-70 transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring">
              <XIcon className="h-5 w-5" />
              <span className="sr-only">Close</span>
            </DialogPrimitive.Close>
          </div>
          <ScrollArea className="flex-1 px-6 py-4">
            {children}
          </ScrollArea>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}
