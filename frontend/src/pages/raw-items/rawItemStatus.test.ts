import { describe, expect, it } from "vitest"
import { getRawItemStatusKind } from "./rawItemStatus"

describe("getRawItemStatusKind", () => {
  it("prioritizes filtered status over conflicts and parser status", () => {
    expect(getRawItemStatusKind({
      filter_passed: false,
      conflict_flag: true,
      status: "parsed",
    })).toBe("filtered")
  })

  it("shows conflict when item is not filtered", () => {
    expect(getRawItemStatusKind({
      filter_passed: true,
      conflict_flag: true,
      status: "parsed",
    })).toBe("conflict")
  })

  it("falls back to raw item parser status", () => {
    expect(getRawItemStatusKind({
      filter_passed: null,
      conflict_flag: false,
      status: "no_match",
    })).toBe("no_match")
  })
})
