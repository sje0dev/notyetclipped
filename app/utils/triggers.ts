import type { SemanticTrigger, TriggerMetric, TriggerProfile } from '~/types'

export const semanticDefault = (): SemanticTrigger => ({ enabled: false, stories: true, visual: true, interval_seconds: 90, min_confidence: 0.8, guidance: '' })
export const clipSource = (trigger: string) => ({ story: 'Story & Gespräch', visual: 'Video-Moment', rule: 'Audio & Chat', audio: 'Audio · früher', manual: 'Manuell' })[trigger] || trigger

export const triggerMetrics: Record<TriggerMetric, { label: string; unit: string; min: number; max: number; step: number; value: number; hint: string }> = {
  audio_rise: { label: 'Audioanstieg', unit: 'dB', min: 3, max: 30, step: 0.5, value: 8, hint: 'Über dem üblichen aktiven Pegel der letzten zwei Minuten.' },
  audio_level: { label: 'Audiopegel', unit: 'dBFS', min: -60, max: 0, step: 1, value: -24, hint: 'Absolute Lautstärke. Näher an 0 bedeutet lauter.' },
  chat_messages: { label: 'Chatnachrichten', unit: '/ 10s', min: 1, max: 10000, step: 1, value: 5, hint: 'Nachrichten im zuletzt beobachteten 10-Sekunden-Fenster.' },
  chat_users: { label: 'Aktive Chatter', unit: '/ 10s', min: 1, max: 10000, step: 1, value: 3, hint: 'Verschiedene Personen. Einzelne Spammer zählen nur einmal.' },
  chat_rate: { label: 'Chat-Anstieg', unit: '× normal', min: 1, max: 30, step: 0.5, value: 2, hint: 'Aktuelle Nachrichtenrate im Verhältnis zu den vorherigen zwei Minuten. Nach 70s verfügbar.' },
}

export function triggerPreset(kind: 'combined' | 'audio' | 'chat'): TriggerProfile {
  const base: TriggerProfile = { version: 1, warmup_seconds: 60, cooldown_seconds: 120, hourly_limit: 6, rules: [], semantic: semanticDefault() }
  if (kind === 'combined') base.rules = [{ name: 'Reaktion mit Chat', enabled: true, hold_seconds: 1.5, conditions: [
    { metric: 'audio_rise', op: 'gte', value: 8 }, { metric: 'audio_level', op: 'gte', value: -24 },
    { metric: 'chat_messages', op: 'gte', value: 5 }, { metric: 'chat_users', op: 'gte', value: 3 },
  ] }]
  if (kind === 'audio') base.rules = [{ name: 'Deutliche Audioreaktion', enabled: true, hold_seconds: 2, conditions: [
    { metric: 'audio_rise', op: 'gte', value: 12 }, { metric: 'audio_level', op: 'gte', value: -18 },
  ] }]
  if (kind === 'chat') base.rules = [{ name: 'Chat reagiert', enabled: true, hold_seconds: 1, conditions: [
    { metric: 'chat_rate', op: 'gte', value: 3 }, { metric: 'chat_messages', op: 'gte', value: 10 }, { metric: 'chat_users', op: 'gte', value: 5 },
  ] }]
  return base
}
