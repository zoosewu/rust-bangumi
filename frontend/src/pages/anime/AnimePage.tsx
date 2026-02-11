import { useState } from "react"
import { useNavigate } from "react-router-dom"
import { useTranslation } from "react-i18next"
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
  const { t } = useTranslation()
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
    { key: "anime_id", header: t("common.id"), render: (item) => String(item.anime_id) },
    { key: "title", header: t("common.name"), render: (item) => String(item.title) },
    {
      key: "created_at",
      header: t("anime.created"),
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
          {t("common.delete")}
        </Button>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("anime.title")}</h1>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          {t("anime.addAnime")}
        </Button>
      </div>

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
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
            <DialogTitle>{t("anime.addAnime")}</DialogTitle>
          </DialogHeader>
          <Input
            placeholder={t("anime.animeTitle")}
            value={newTitle}
            onChange={(e) => setNewTitle(e.target.value)}
          />
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              {t("common.cancel")}
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
              {creating ? t("common.creating") : t("common.create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirm */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title={t("anime.deleteAnime")}
        description={t("anime.deleteConfirm", { title: deleteTarget?.title })}
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
