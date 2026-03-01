import { useMemo } from "react"

/**
 * Recursively stringify a value to a flat string for searching.
 * Handles primitives, arrays, and plain objects.
 */
function stringifyValue(val: unknown): string {
  if (val === null || val === undefined) return ""
  if (typeof val === "string") return val
  if (typeof val === "number" || typeof val === "boolean") return String(val)
  if (Array.isArray(val)) return val.map(stringifyValue).join(" ")
  if (typeof val === "object") return Object.values(val as Record<string, unknown>).map(stringifyValue).join(" ")
  return ""
}

/**
 * Generic client-side search hook.
 * Returns items matching the query (any field, case-insensitive), capped at 50.
 * If query is empty/whitespace, returns first 50 items unchanged.
 */
export function useTableSearch<T>(data: T[], query: string): T[] {
  return useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return data.slice(0, 50)
    return data
      .filter((item) => stringifyValue(item).toLowerCase().includes(q))
      .slice(0, 50)
  }, [data, query])
}
