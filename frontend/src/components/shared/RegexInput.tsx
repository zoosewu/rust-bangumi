import { useState, useEffect } from "react"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

interface RegexInputProps {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  className?: string
}

export function RegexInput({
  value,
  onChange,
  placeholder,
  className,
}: RegexInputProps) {
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!value) {
      setError(null)
      return
    }
    try {
      new RegExp(value)
      setError(null)
    } catch (e) {
      setError((e as Error).message)
    }
  }, [value])

  return (
    <div>
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={cn("font-mono text-sm", error && "border-destructive", className)}
      />
      {error && <p className="text-xs text-destructive mt-1">{error}</p>}
    </div>
  )
}
