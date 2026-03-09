/**
 * Format an ISO 8601 datetime string using the browser's local timezone.
 * Returns empty string for null/undefined/invalid dates.
 */
export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return ""
  const d = new Date(iso)
  if (isNaN(d.getTime())) return iso
  return d.toLocaleString()
}

/**
 * Format an ISO 8601 datetime string as date only (YYYY-MM-DD) in local timezone.
 */
export function formatDate(iso: string | null | undefined): string {
  if (!iso) return ""
  const d = new Date(iso)
  if (isNaN(d.getTime())) return iso.slice(0, 10)
  const year = d.getFullYear()
  const month = String(d.getMonth() + 1).padStart(2, "0")
  const day = String(d.getDate()).padStart(2, "0")
  return `${year}-${month}-${day}`
}
