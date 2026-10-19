import type { ContentRule, SemanticTrigger, TriggerMetric, TriggerProfile } from '~/types'

export const semanticDefault = (): SemanticTrigger => ({ enabled: false, stories: true, visual: true, interval_seconds: 90, min_confidence: 0.8, guidance: '', exclusions: '', context_seconds: 120, language: 'auto', use_feedback: true, rules: [] })
export const contentPresets: Omit<ContentRule, 'id' | 'enabled'>[] = [
  { name: 'Konter & Schlagabtausch', kind: 'story', description: 'Ein unerwarteter trockener Konter, ein treffender Roast unter Freunden oder ein kurzer, absurder Dialog. Die ursprüngliche Frage oder Behauptung, den Konter und die direkte Reaktion mitnehmen. Gewöhnliche Beleidigungen ohne Pointe auslassen.' },
  { name: 'Story mit Pointe', kind: 'story', description: 'Eine persönlich erzählte Geschichte mit verständlichem Aufbau, überraschender Wendung und abgeschlossener Pointe. Am Beginn erklären, wer oder was gemeint ist. Nicht mitten im Satz beginnen und keinen noch offenen Erzählbogen abschneiden.' },
  { name: 'Fails & Eigentore', kind: 'visual', description: 'Ein klar sichtbarer, unerwarteter Fehlschlag mit nachvollziehbarer Ursache: Selbstüberschätzung, missglückter Plan oder ein ironisches Eigentor. Den Versuch davor und die Reaktion danach zeigen. Keine zufälligen Tode ohne Besonderheit; nur in den Bildern belegte Ereignisse.' },
  { name: 'Clutch & Comeback', kind: 'visual', description: 'Eine sichtbar schwierige Situation wird unerwartet gerettet, ein knappes Duell gewonnen oder ein außergewöhnliches Ziel erreicht. Gefahr, entscheidenden Moment und Ergebnis müssen erkennbar sein. Keine angeblichen Erfolge aus bloßen Zurufen ableiten.' },
  { name: 'Überraschung & Enthüllung', kind: 'story', description: 'Eine Information, ein Geständnis oder eine Enthüllung verändert die Situation und löst eine echte, überraschende Reaktion aus. Erwartung davor, Enthüllung und Reaktion gehören zusammen. Alltägliche Ankündigungen und bloßen Clickbait auslassen.' },
  { name: 'Eskalation & gespielte Empörung', kind: 'story', description: 'Eine kleine Situation eskaliert zu einer absurden Tirade, übertriebenem Rollenspiel oder gespielter Empörung mit komödiantischer Steigerung. Auslöser, stärkste Steigerung und Abschluss sichern. Lautstärke oder monotones Schimpfen allein zählen nicht.' },
  { name: 'Emotionale Momente', kind: 'story', description: 'Ein ehrlicher, berührender oder besonders herzlicher Austausch: Dank, Wiedersehen, unerwartete Unterstützung oder eine persönliche Erkenntnis mit klarem Abschluss. Den Anlass mitnehmen. Gewöhnliche Begrüßungen und routinemäßige Spenden-Danksagungen auslassen.' },
  { name: 'Prank & Reaktion', kind: 'visual', description: 'Ein sichtbarer Streich, eine Falle oder eine überraschende Aktion mit verständlicher Vorbereitung und unmittelbarer Reaktion der Beteiligten. Den tatsächlichen Ausgang zeigen; über einen geplanten Streich zu reden ist noch kein visueller Prank.' },
]
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
