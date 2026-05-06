import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core"
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable"
import type { AiProvider, TestAiProviderResult } from "@/schemas/ai"
import { AiProviderRow } from "./AiProviderRow"

export interface AiProviderListProps {
  providers: readonly AiProvider[]
  onReorder: (ordered_ids: number[]) => Promise<void>
  onEdit: (p: AiProvider) => void
  onDelete: (id: number) => void
  onToggle: (id: number, is_enabled: boolean) => void
  onTest: (id: number) => Promise<TestAiProviderResult>
}

export function AiProviderList(props: AiProviderListProps) {
  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )

  const handleDragEnd = (e: DragEndEvent) => {
    if (!e.over || e.active.id === e.over.id) return
    const ids = props.providers.map((p) => p.id)
    const oldIdx = ids.indexOf(Number(e.active.id))
    const newIdx = ids.indexOf(Number(e.over.id))
    if (oldIdx < 0 || newIdx < 0) return
    void props.onReorder(arrayMove(ids, oldIdx, newIdx))
  }

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext
        items={props.providers.map((p) => p.id)}
        strategy={verticalListSortingStrategy}
      >
        <div className="space-y-2">
          {props.providers.map((p, i) => (
            <AiProviderRow
              key={p.id}
              provider={p}
              index={i}
              onEdit={props.onEdit}
              onDelete={props.onDelete}
              onToggle={props.onToggle}
              onTest={props.onTest}
            />
          ))}
        </div>
      </SortableContext>
    </DndContext>
  )
}
