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

const USAGE_ORDER: Record<LinkUsage, number> = { used: 0, unused: 1, filtered: 2 }

/** 採用中、未採用、被過濾依序排列；同一組內維持原本順序（集數） */
export function sortByUsage<T extends UsageFields>(links: readonly T[]): T[] {
  return [...links].sort((a, b) => USAGE_ORDER[linkUsage(a)] - USAGE_ORDER[linkUsage(b)])
}
