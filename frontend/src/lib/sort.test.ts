import { describe, expect, it } from "vitest"
import { sortByTitle } from "./sort"

describe("sortByTitle", () => {
  it("sorts by title with numbers in natural order, untitled last", () => {
    const sorted = sortByTitle([
      { title: "[G] Show - 10 (CR)" },
      { title: null },
      { title: "[G] Show - 2 (ABEMA)" },
      { title: "[G] Show - 2 (CR)" },
    ])
    expect(sorted.map((i) => i.title)).toEqual([
      "[G] Show - 2 (ABEMA)",
      "[G] Show - 2 (CR)",
      "[G] Show - 10 (CR)",
      null,
    ])
  })
})
