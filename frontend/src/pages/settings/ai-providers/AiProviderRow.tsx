import { useSortable } from "@dnd-kit/sortable"
import { CSS } from "@dnd-kit/utilities"
import { GripVertical, Loader2, Pencil, Trash2 } from "lucide-react"
import { useState } from "react"
import { TagBadge } from "@/components/shared/TagBadge"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import type { AiProvider, TestAiProviderResult } from "@/schemas/ai"

export interface AiProviderRowProps {
  provider: AiProvider
  index: number
  onEdit: (p: AiProvider) => void
  onDelete: (id: number) => void
  onToggle: (id: number, is_enabled: boolean) => void
  onTest: (id: number) => Promise<TestAiProviderResult>
}

export function AiProviderRow({ provider, index, onEdit, onDelete, onToggle, onTest }: AiProviderRowProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: provider.id,
  })
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestAiProviderResult | null>(null)

  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : provider.is_enabled ? 1 : 0.5,
  }

  const handleTest = async () => {
    setTesting(true)
    setTestResult(null)
    try {
      setTestResult(await onTest(provider.id))
    } finally {
      setTesting(false)
    }
  }

  return (
    <div
      ref={setNodeRef}
      style={style}
      className="flex items-center gap-2 rounded border bg-card p-2"
    >
      <button
        type="button"
        {...attributes}
        {...listeners}
        className="cursor-grab text-muted-foreground hover:text-foreground"
        aria-label="拖曳重排"
      >
        <GripVertical className="size-4" />
      </button>
      <TagBadge>#{index + 1}</TagBadge>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="font-medium truncate">{provider.name}</span>
          <TagBadge tone="info">{provider.provider_kind}</TagBadge>
        </div>
        <div className="text-xs text-muted-foreground truncate">
          {provider.model_name || "(未設定 model)"}
        </div>
        {testResult && (
          <div className={`text-xs ${testResult.ok ? "text-green-600" : "text-destructive"}`}>
            {testResult.ok ? "✓ OK" : `✗ ${testResult.error ?? "失敗"}`}
          </div>
        )}
      </div>
      <Switch
        checked={provider.is_enabled}
        onCheckedChange={(v) => onToggle(provider.id, v)}
        aria-label="啟用"
      />
      <Button variant="outline" size="sm" disabled={testing} onClick={handleTest}>
        {testing && <Loader2 className="mr-1 size-3 animate-spin" />}
        測試
      </Button>
      <Button variant="outline" size="sm" onClick={() => onEdit(provider)} aria-label="編輯">
        <Pencil className="size-3" />
      </Button>
      <Button
        variant="destructive"
        size="sm"
        onClick={() => onDelete(provider.id)}
        aria-label="刪除"
      >
        <Trash2 className="size-3" />
      </Button>
    </div>
  )
}
