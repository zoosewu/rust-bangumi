import { useState, useEffect } from "react"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

interface RegexInputProps {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  /** 套用到外層 wrapper div（用於版面：flex-1、w-full 等） */
  className?: string
  /** 套用到內層 input 元素（用於尺寸：h-7、text-xs 等） */
  inputClassName?: string
}

export function RegexInput({
  value,
  onChange,
  placeholder,
  className,
  inputClassName,
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
    <div className={cn("w-full", className)}>
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={cn("font-mono text-sm", error && "border-destructive", inputClassName)}
      />
      {error && <p className="text-xs text-destructive mt-1">{error}</p>}
    </div>
  )
}
