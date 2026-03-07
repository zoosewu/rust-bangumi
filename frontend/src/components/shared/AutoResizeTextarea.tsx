import { useLayoutEffect, useRef } from "react"
import { Textarea } from "@/components/ui/textarea"
import { cn } from "@/lib/utils"

const MAX_AUTO_LINES = 20

interface AutoResizeTextareaProps {
  value: string
  onChange?: (e: React.ChangeEvent<HTMLTextAreaElement>) => void
  onBlur?: (e: React.FocusEvent<HTMLTextAreaElement>) => void
  placeholder?: string
  className?: string
  readOnly?: boolean
}

/**
 * 自動調整高度的多行文字輸入框。
 * - 最多自動展開到 MAX_AUTO_LINES 行
 * - 超過後出現捲軸，使用者仍可手動拖曳調整大小
 */
export function AutoResizeTextarea({
  value,
  onChange,
  onBlur,
  placeholder,
  className,
  readOnly,
}: AutoResizeTextareaProps) {
  const ref = useRef<HTMLTextAreaElement>(null)

  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    el.style.height = "auto"
    const style = getComputedStyle(el)
    const lineHeight = parseFloat(style.lineHeight) || 20
    const paddingY =
      parseFloat(style.paddingTop) + parseFloat(style.paddingBottom)
    const maxH = lineHeight * MAX_AUTO_LINES + paddingY
    const newH = Math.min(el.scrollHeight, maxH)
    el.style.height = newH + "px"
    el.style.overflowY = el.scrollHeight > maxH ? "auto" : "hidden"
  }, [value])

  return (
    <Textarea
      ref={ref}
      value={value}
      onChange={onChange}
      onBlur={onBlur}
      placeholder={placeholder}
      readOnly={readOnly}
      className={cn("resize-y [field-sizing:normal]", className)}
    />
  )
}
