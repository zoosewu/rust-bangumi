import type { AnimeLinkRich } from "@/schemas/anime"

/**
 * 一個版本在這一集的採用狀態：
 * - used：採用中（會下載 / 已下載），手動指定的版本一律是這個
 * - unused：同集選了別的版本
 * - filtered：被過濾規則排除
 */
export type LinkUsage = "used" | "unused" | "filtered"

type UsageFields = Pick<AnimeLinkRich, "filtered_flag" | "link_status" | "manual_choice">

export function linkUsage(link: UsageFields): LinkUsage {
  if (link.manual_choice) return "used"
  if (link.filtered_flag) return "filtered"
  return link.link_status === "resolved" ? "unused" : "used"
}
