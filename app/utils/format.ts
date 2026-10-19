export function bytes(value: number | undefined): string {
  if (!value) return '0 MB'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1)
  return `${(value / 1024 ** index).toLocaleString('de-DE', { maximumFractionDigits: 1 })} ${units[index]}`
}

export function duration(seconds: number): string {
  return `${Math.floor(seconds / 60)}:${Math.floor(seconds % 60).toString().padStart(2, '0')}`
}

export function dateTime(value: string): string {
  return new Intl.DateTimeFormat('de-DE', { timeZone: 'Europe/Berlin', day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' }).format(new Date(value))
}

export function errorMessage(error: unknown): string {
  const data = (error as { data?: { detail?: string | { msg: string }[] } })?.data
  if (typeof data?.detail === 'string') return data.detail
  if (Array.isArray(data?.detail)) return data.detail.map(item => item.msg.replace('Value error, ', '')).join(' ')
  return 'Die Anfrage konnte nicht abgeschlossen werden. Bitte Verbindung prüfen.'
}
