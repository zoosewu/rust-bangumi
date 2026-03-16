import { describe, it, expect } from "vitest"

/**
 * 測試 FilterRulePanel 中刪除規則後 selectedIdx 的更新邏輯。
 * 此邏輯已抽取為純函數以便測試。
 */
function updateSelectedIdxAfterDelete(
  selectedIdx: number | null,
  deletedIdx: number,
): number | null {
  if (selectedIdx === deletedIdx) return null
  if (selectedIdx !== null && selectedIdx > deletedIdx) return selectedIdx - 1
  return selectedIdx
}

describe("updateSelectedIdxAfterDelete", () => {
  it("returns null when the deleted item was selected", () => {
    expect(updateSelectedIdxAfterDelete(2, 2)).toBeNull()
  })

  it("returns null when selectedIdx is already null", () => {
    expect(updateSelectedIdxAfterDelete(null, 1)).toBeNull()
  })

  it("decrements selectedIdx when a preceding item is deleted", () => {
    // rules = [A, B, C], selectedIdx=2 (C), delete B (idx=1) → C moves to idx=1
    expect(updateSelectedIdxAfterDelete(2, 1)).toBe(1)
  })

  it("decrements selectedIdx when the first item is deleted and selected is last", () => {
    // rules = [A, B, C], selectedIdx=2 (C), delete A (idx=0) → C moves to idx=1
    expect(updateSelectedIdxAfterDelete(2, 0)).toBe(1)
  })

  it("keeps selectedIdx unchanged when a following item is deleted", () => {
    // rules = [A, B, C], selectedIdx=0 (A), delete C (idx=2) → A stays at idx=0
    expect(updateSelectedIdxAfterDelete(0, 2)).toBe(0)
  })

  it("keeps selectedIdx unchanged when selectedIdx equals 0 and idx=1 is deleted", () => {
    expect(updateSelectedIdxAfterDelete(0, 1)).toBe(0)
  })

  it("handles single-item list: delete only item that was selected", () => {
    expect(updateSelectedIdxAfterDelete(0, 0)).toBeNull()
  })
})
