/** 以 1024 為單位的檔案大小，例如 433.7 MB */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"]
  const exp =
    bytes > 0 ? Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1) : 0
  const value = bytes / 1024 ** exp
  return `${exp === 0 ? value : value.toFixed(1)} ${units[exp]}`
}
