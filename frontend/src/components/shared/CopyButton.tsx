import { useState } from "react"
import { Copy, Check } from "lucide-react"
import { Button } from "@/components/ui/button"

interface CopyButtonProps {
  text: string
  label?: string
  copiedLabel?: string
}

export function CopyButton({ text, label, copiedLabel }: CopyButtonProps) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation()
    try {
      await navigator.clipboard.writeText(text)
    } catch {
      const el = document.createElement("textarea")
      el.value = text
      el.style.cssText = "position:fixed;opacity:0;pointer-events:none;"
      document.body.appendChild(el)
      el.select()
      document.execCommand("copy")
      el.remove()
    }
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  if (label !== undefined) {
    return (
      <Button onClick={handleCopy}>
        {copied ? (
          <>
            <Check className="h-4 w-4 mr-1" />
            {copiedLabel ?? label}
          </>
        ) : (
          <>
            <Copy className="h-4 w-4 mr-1" />
            {label}
          </>
        )}
      </Button>
    )
  }

  return (
    <button
      type="button"
      onClick={handleCopy}
      className="shrink-0 p-1 rounded hover:bg-black/5 dark:hover:bg-white/10 transition-colors cursor-pointer"
    >
      {copied ? (
        <Check className="h-3.5 w-3.5 text-green-600" />
      ) : (
        <Copy className="h-3.5 w-3.5 opacity-40 hover:opacity-70" />
      )}
    </button>
  )
}
