const titleCollator = new Intl.Collator(undefined, { numeric: true })

/** 依名稱排序（自然排序：「- 2」排在「- 10」前面）；沒有名稱的排最後。不改動原陣列 */
export function sortByTitle<T extends { title: string | null }>(items: readonly T[]): T[] {
  return [...items].sort((a, b) => {
    if (a.title === null || b.title === null) {
      return Number(a.title === null) - Number(b.title === null)
    }
    return titleCollator.compare(a.title, b.title)
  })
}
