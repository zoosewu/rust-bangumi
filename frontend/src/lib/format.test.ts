import { describe, expect, it } from "vitest"
import { formatBytes } from "./format"

describe("formatBytes", () => {
  it("uses the largest whole unit", () => {
    expect(formatBytes(0)).toBe("0 B")
    expect(formatBytes(512)).toBe("512 B")
    expect(formatBytes(454767424)).toBe("433.7 MB")
    expect(formatBytes(3 * 1024 ** 4)).toBe("3.0 TB")
  })
})
