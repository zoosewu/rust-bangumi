import { useTranslation } from "react-i18next"
import { FullScreenDialog } from "@/components/shared/FullScreenDialog"
import { FilterRuleEditor } from "@/components/shared/FilterRuleEditor"
import { ParserEditor } from "@/components/shared/ParserEditor"
import { InfoSection } from "@/components/shared/InfoSection"
import { InfoItem } from "@/components/shared/InfoItem"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

interface SubtitleGroupDialogProps {
  groupId: number
  groupName: string
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function SubtitleGroupDialog({ groupId, groupName, open, onOpenChange }: SubtitleGroupDialogProps) {
  const { t } = useTranslation()

  return (
    <FullScreenDialog
      open={open}
      onOpenChange={onOpenChange}
      title={groupName}
    >
      <div className="space-y-6">
        {/* Group info */}
        <InfoSection cols={2}>
          <InfoItem label={t("common.id")} value={String(groupId)} />
          <InfoItem label={t("subtitleGroups.groupName", "Group Name")} value={groupName} />
        </InfoSection>

        {/* Sub-tabs for filter rules and parsers */}
        <Tabs defaultValue="filters">
          <TabsList variant="line">
            <TabsTrigger value="filters">{t("dialog.filterRules", "Filter Rules")}</TabsTrigger>
            <TabsTrigger value="parsers">{t("dialog.parsers", "Parsers")}</TabsTrigger>
          </TabsList>
          <TabsContent value="filters" className="mt-4">
            <FilterRuleEditor
              targetType="subtitle_group"
              targetId={groupId}
            />
          </TabsContent>
          <TabsContent value="parsers" className="mt-4">
            <ParserEditor
              createdFromType="subtitle_group"
              createdFromId={groupId}
            />
          </TabsContent>
        </Tabs>
      </div>
    </FullScreenDialog>
  )
}
