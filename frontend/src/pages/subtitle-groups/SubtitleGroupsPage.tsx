import { useState } from "react"
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
import { Label } from "@/components/ui/label"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Plus, Trash2 } from "lucide-react"
import { SubtitleGroupDialog } from "./SubtitleGroupDialog"

export default function SubtitleGroupsPage() {
  const { t } = useTranslation()
  const [createOpen, setCreateOpen] = useState(false)
  const [newName, setNewName] = useState("")
  const [selectedGroup, setSelectedGroup] = useState<{ id: number; name: string } | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<{ id: number; name: string } | null>(null)

  const { data: groups, isLoading, refetch } = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getSubtitleGroups
      }),
    [],
  )

  const { mutate: createGroup, isLoading: creating } = useEffectMutation(
    (name: string) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.createSubtitleGroup(name)
      }),
  )

  const { mutate: deleteGroup, isLoading: deleting } = useEffectMutation(
    (groupId: number) =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.deleteSubtitleGroup(groupId)
      }),
  )

  const columns: Column<Record<string, unknown>>[] = [
    {
      key: "group_id",
      header: t("common.id"),
      render: (item) => String(item.group_id),
    },
    {
      key: "group_name",
      header: t("subtitleGroups.groupName"),
      render: (item) => String(item.group_name),
    },
    {
      key: "created_at",
      header: t("rawItems.created"),
      render: (item) => String(item.created_at).slice(0, 19).replace("T", " "),
    },
    {
      key: "actions",
      header: "",
      render: (item) => (
        <Button
          variant="ghost"
          size="sm"
          onClick={(e) => {
            e.stopPropagation()
            setDeleteTarget({ id: item.group_id as number, name: item.group_name as string })
          }}
        >
          <Trash2 className="h-4 w-4 text-destructive" />
        </Button>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("subtitleGroups.title")}</h1>
        <Button size="sm" onClick={() => setCreateOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          {t("subtitleGroups.addGroup")}
        </Button>
      </div>

      {isLoading ? (
        <p className="text-muted-foreground">{t("common.loading")}</p>
      ) : groups && groups.length > 0 ? (
        <DataTable
          columns={columns}
          data={groups as unknown as Record<string, unknown>[]}
          keyField="group_id"
          onRowClick={(row) => setSelectedGroup({ id: row.group_id as number, name: row.group_name as string })}
        />
      ) : (
        <p className="text-sm text-muted-foreground">{t("subtitleGroups.noGroups")}</p>
      )}

      {selectedGroup && (
        <SubtitleGroupDialog
          groupId={selectedGroup.id}
          groupName={selectedGroup.name}
          open={!!selectedGroup}
          onOpenChange={(open) => {
            if (!open) {
              setSelectedGroup(null)
              refetch()
            }
          }}
        />
      )}

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subtitleGroups.addGroup")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div>
              <Label>{t("subtitleGroups.groupName")}</Label>
              <Input
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder={t("subtitleGroups.groupName")}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              disabled={!newName.trim() || creating}
              onClick={() => {
                createGroup(newName.trim()).then(() => {
                  setNewName("")
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

      <ConfirmDialog
        open={deleteTarget !== null}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title={t("subtitleGroups.deleteGroup")}
        description={t("subtitleGroups.deleteConfirm", { name: deleteTarget?.name })}
        loading={deleting}
        onConfirm={() => {
          if (deleteTarget !== null) {
            deleteGroup(deleteTarget.id).then(() => {
              setDeleteTarget(null)
              refetch()
            })
          }
        }}
      />
    </div>
  )
}
