import { useTranslation } from "react-i18next"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"

export default function FiltersPage() {
  const { t } = useTranslation()

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">{t("filters.title")}</h1>
      <FilterRuleEditor targetType="global" targetId={null} />
    </div>
  )
}
