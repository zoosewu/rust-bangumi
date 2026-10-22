import { describe, expect, it } from "vitest"
import { linkUsage, sortByUsage } from "./linkUsage"

const link = (filtered_flag: boolean, link_status: string, manual_choice = false) => ({
  filtered_flag,
  link_status,
  manual_choice,
})

describe("linkUsage", () => {
  it("tells used, unused and filtered versions apart", () => {
    expect(linkUsage(link(false, "active"))).toBe("used")
    expect(linkUsage(link(false, "resolved"))).toBe("unused")
    expect(linkUsage(link(true, "active"))).toBe("filtered")
    expect(linkUsage(link(true, "resolved"))).toBe("filtered")
  })

  it("always shows a manually chosen version as used", () => {
    // 剛指定、畫面還拿著舊的 filtered_flag 時也一樣
    expect(linkUsage(link(true, "active", true))).toBe("used")
  })

  it("lists used versions first", () => {
    const sorted = sortByUsage([
      { ...link(true, "active"), id: "cr" },
      { ...link(false, "resolved"), id: "old" },
      { ...link(false, "active"), id: "abema" },
    ])
    expect(sorted.map((l) => l.id)).toEqual(["abema", "old", "cr"])
  })
})
