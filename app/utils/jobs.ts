import type { Job } from '~/types'

export const jobStatusLabels: Record<string, string> = {
  recording: 'Aufnahme läuft', connecting: 'Verbinden', waiting: 'Wartet', collecting: 'Nachlauf sammeln',
  queued: 'In Warteschlange', preparing: 'Sprache & Bilder', analyzing: 'ChatGPT wertet aus',
  cutting: 'Clip schneiden', uploading: 'S3-Upload', rendering: 'Hochkant rendern', ready: 'Fertig',
  clipped: 'Moment gesichert', skipped: 'Übersprungen', failed: 'Fehler', retry: 'Wiederholung wartet',
  deleting: 'Wird gelöscht', finished: 'Beendet',
}

export const jobKindLabels: Record<Job['kind'], string> = {
  recording: 'Aufnahme', clip: 'Clip-Export', ai: 'KI-Analyse', edit: 'Hochkant',
}

export function jobDuration(job: Job, now: number): string {
  if (!job.active && !job.finished_at) return '—'
  const end = job.active ? now : Date.parse(job.finished_at!)
  return duration(Math.max(0, (end - Date.parse(job.created_at)) / 1000))
}

export function jobLink(id: string) {
  return { path: '/jobs', query: { job: id }, hash: '#job-logs' }
}
