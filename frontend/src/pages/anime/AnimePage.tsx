import { useState } from "react"
import { useNavigate } from "react-router-dom"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { useEffectMutation } from "@/hooks/useEffectMutation"
import { DataTable } from "@/components/shared/DataTable"
import type { Column } from "@/components/shared/DataTable"
import { ConfirmDialog } from "@/components/shared/ConfirmDialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Plus } from "lucide-react"

export default function AnimePage() {
  const navigate = useNavigate()
  const [createOpen, setCreateOpen] = useState(false)
  const [newTitle, setNewTitle] = useState("")
  const [deleteTarget, setDeleteTarget] = useState<{
    id: number
    title: string
  } | null>(null)

  const { data: animes, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getAnimes
      }),
    [],
  )

  const { mutate: createAnime, isLoading: creating } = useEffectMutation(
    (title: string) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createAnime(title)
      }),
  )

  const { mutate: deleteAnime, isLoading: deleting } = useEffectMutation(
    (id: number) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.deleteAnime(id)
      }),
  )

  const columns: Column<Record<string, unknown>>[] = [
    { key: "anime_id", header: "ID", render: (item) => String(item.anime_id) },
    { key: "title", header: "Title", render: (item) => String(item.title) },
    {
      key: "created_at",
      header: "Created",
      render: (item) => String(item.created_at).slice(0, 10),
    },
    {
      key: "actions",
      header: "",
      render: (item) => (
        <Button
          variant="ghost"
          size="sm"
          className="text-destructive"
          onClick={(e) => {
            e.stopPropagation()
            setDeleteTarget({
              id: item.anime_id as number,
              title: item.title as string,
            })
          }}
        >
          Delete
        </Button>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Anime</h1>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          Add Anime
        </Button>
      </div>

      {isLoading ? (
        <p className="text-muted-foreground">Loading...</p>
      ) : (
        <DataTable
          columns={columns}
          data={(animes ?? []) as unknown as Record<string, unknown>[]}
          keyField="anime_id"
          onRowClick={(item) => navigate(`/anime/${item.anime_id}`)}
        />
      )}

      {/* Create Dialog */}
      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Add Anime</DialogTitle>
          </DialogHeader>
          <Input
            placeholder="Anime title"
            value={newTitle}
            onChange={(e) => setNewTitle(e.target.value)}
          />
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              Cancel
            </Button>
            <Button
              disabled={!newTitle.trim() || creating}
              onClick={() => {
                createAnime(newTitle.trim()).then(() => {
                  setNewTitle("")
                  setCreateOpen(false)
                  refetch()
                })
              }}
            >
              {creating ? "Creating..." : "Create"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirm */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title="Delete Anime"
        description={`Are you sure you want to delete "${deleteTarget?.title}"?`}
        loading={deleting}
        onConfirm={() => {
          if (deleteTarget) {
            deleteAnime(deleteTarget.id).then(() => {
              setDeleteTarget(null)
              refetch()
            })
          }
        }}
      />
    </div>
  )
}
