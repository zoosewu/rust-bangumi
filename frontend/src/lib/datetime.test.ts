import { describe, it, expect } from "vitest"
import { formatDate, formatDateTime } from "./datetime"

describe("formatDate", () => {
  it("returns empty string for null", () => {
    expect(formatDate(null)).toBe("")
  })

  it("returns empty string for undefined", () => {
    expect(formatDate(undefined)).toBe("")
  })

  it("returns empty string for empty string", () => {
    expect(formatDate("")).toBe("")
  })

  it("formats a valid ISO date string to YYYY-MM-DD", () => {
    const result = formatDate("2024-03-15T10:30:00Z")
    expect(result).toMatch(/^\d{4}-\d{2}-\d{2}$/)
  })

  it("returns first 10 chars for invalid date string", () => {
    expect(formatDate("not-a-date-xx")).toBe("not-a-date")
  })

  it("handles date-only strings", () => {
    const result = formatDate("2024-06-01")
    expect(result).toMatch(/^\d{4}-\d{2}-\d{2}$/)
  })

  it("zero-pads month and day", () => {
    // Use a date that won't shift due to timezone (UTC noon)
    const result = formatDate("2024-01-05T12:00:00Z")
    const [, month, day] = result.split("-")
    expect(month).toHaveLength(2)
    expect(day).toHaveLength(2)
  })
})

describe("formatDateTime", () => {
  it("returns empty string for null", () => {
    expect(formatDateTime(null)).toBe("")
  })

  it("returns empty string for undefined", () => {
    expect(formatDateTime(undefined)).toBe("")
  })

  it("returns empty string for empty string", () => {
    expect(formatDateTime("")).toBe("")
  })

  it("returns the original string for an invalid date", () => {
    expect(formatDateTime("not-a-date")).toBe("not-a-date")
  })

  it("returns a non-empty string for a valid ISO datetime", () => {
    const result = formatDateTime("2024-03-15T10:30:00Z")
    expect(result).not.toBe("")
    expect(typeof result).toBe("string")
  })

  it("handles date-only ISO strings", () => {
    const result = formatDateTime("2024-06-01")
    expect(result).not.toBe("")
  })
})
