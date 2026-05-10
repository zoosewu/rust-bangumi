export type RawItemStatusKind = "eliminated" | "conflict" | string

interface RawItemStatusInput {
  filter_passed?: boolean | null
  conflict_flag?: boolean
  status: string
}

export function getRawItemStatusKind(item: RawItemStatusInput): RawItemStatusKind {
  if (item.filter_passed === false) return "eliminated"
  if (item.conflict_flag) return "conflict"
  return item.status
}
