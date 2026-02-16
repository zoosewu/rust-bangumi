import { useTranslation } from "react-i18next"
import { Button } from "@/components/ui/button"
import { Pencil, Save, X } from "lucide-react"

interface InfoSectionProps {
  children: React.ReactNode
  cols?: 2 | 3 | 4
  editing?: boolean
  saving?: boolean
  onEdit?: () => void
  onSave?: () => void
  onCancel?: () => void
}

export function InfoSection({
  children,
  cols = 4,
  editing,
  saving,
  onEdit,
  onSave,
  onCancel,
}: InfoSectionProps) {
  const { t } = useTranslation()

  const gridClass =
    cols === 2
      ? "grid grid-cols-2 gap-4"
      : cols === 3
        ? "grid grid-cols-2 md:grid-cols-3 gap-4"
        : "grid grid-cols-2 md:grid-cols-4 gap-4"

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-muted-foreground">{t("dialog.info", "Info")}</h3>
        {onEdit && (
          !editing ? (
            <Button variant="ghost" size="sm" onClick={onEdit}>
              <Pencil className="h-3.5 w-3.5 mr-1" />
              {t("common.edit", "Edit")}
            </Button>
          ) : (
            <div className="flex gap-1">
              <Button variant="ghost" size="sm" onClick={onCancel} disabled={saving}>
                <X className="h-3.5 w-3.5 mr-1" />
                {t("common.cancel", "Cancel")}
              </Button>
              <Button size="sm" onClick={onSave} disabled={saving}>
                <Save className="h-3.5 w-3.5 mr-1" />
                {t("common.save", "Save")}
              </Button>
            </div>
          )
        )}
      </div>
      <div className={gridClass}>{children}</div>
    </div>
  )
}
