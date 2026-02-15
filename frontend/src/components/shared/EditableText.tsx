import { useState, useEffect, useRef } from "react"
import { Input } from "@/components/ui/input"
import { Pencil, Loader2 } from "lucide-react"

interface EditableTextProps {
  value: string
  onSave: (newValue: string) => Promise<void>
  placeholder?: string
  className?: string
}

export function EditableText({ value, onSave, placeholder, className }: EditableTextProps) {
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(value)
  const [saving, setSaving] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (editing) {
      setDraft(value)
      setTimeout(() => inputRef.current?.select(), 0)
    }
  }, [editing, value])

  const handleSave = async () => {
    const trimmed = draft.trim()
    if (!trimmed || trimmed === value) {
      setEditing(false)
      return
    }
    setSaving(true)
    try {
      await onSave(trimmed)
      setEditing(false)
    } finally {
      setSaving(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleSave()
    if (e.key === "Escape") setEditing(false)
  }

  if (editing) {
    return (
      <div className="flex items-center gap-1">
        <Input
          ref={inputRef}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={handleSave}
          disabled={saving}
          autoFocus
          className={`h-7 text-sm ${className ?? ""}`}
          placeholder={placeholder}
        />
        {saving && <Loader2 className="h-3 w-3 animate-spin" />}
      </div>
    )
  }

  return (
    <button
      type="button"
      className={`group inline-flex items-center gap-1 text-sm font-medium hover:text-primary transition-colors ${className ?? ""}`}
      onClick={() => setEditing(true)}
    >
      {value || <span className="text-muted-foreground">{placeholder}</span>}
      <Pencil className="h-3 w-3 opacity-0 group-hover:opacity-50 transition-opacity" />
    </button>
  )
}
