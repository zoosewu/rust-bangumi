import { describe, it, expect } from "vitest"
import { stringifyValue } from "./useTableSearch"

describe("stringifyValue", () => {
  it("returns empty string for null", () => {
    expect(stringifyValue(null)).toBe("")
  })

  it("returns empty string for undefined", () => {
    expect(stringifyValue(undefined)).toBe("")
  })

  it("returns string as-is", () => {
    expect(stringifyValue("hello")).toBe("hello")
  })

  it("converts number to string", () => {
    expect(stringifyValue(42)).toBe("42")
  })

  it("converts boolean to string", () => {
    expect(stringifyValue(true)).toBe("true")
    expect(stringifyValue(false)).toBe("false")
  })

  it("joins array elements with space", () => {
    expect(stringifyValue(["a", "b", "c"])).toBe("a b c")
  })

  it("handles nested arrays", () => {
    expect(stringifyValue(["a", ["b", "c"]])).toBe("a b c")
  })

  it("stringifies plain object values joined by space", () => {
    const result = stringifyValue({ name: "Naruto", id: 1 })
    expect(result).toContain("Naruto")
    expect(result).toContain("1")
  })

  it("handles nested objects", () => {
    const result = stringifyValue({ season: { year: 2024, name: "Spring" } })
    expect(result).toContain("2024")
    expect(result).toContain("Spring")
  })

  it("handles object with null values", () => {
    const result = stringifyValue({ name: "Test", value: null })
    expect(result).toContain("Test")
  })

  it("handles mixed array of objects and primitives", () => {
    const result = stringifyValue([{ title: "One Piece" }, 42])
    expect(result).toContain("One Piece")
    expect(result).toContain("42")
  })
})

// useTableSearch hook behaviour tested via the pure stringifyValue logic above.
// The hook itself uses useMemo which requires a React environment;
// the search logic is fully covered by stringifyValue tests.
