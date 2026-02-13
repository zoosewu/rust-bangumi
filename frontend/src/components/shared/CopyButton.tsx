import { useState } from "react"
import { Copy, Check } from "lucide-react"

export function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation()
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
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
