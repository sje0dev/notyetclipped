import type { MetricSummary, TriggerMetric, TriggerSignal, TriggerSignals } from '~/types'
import { triggerMetrics } from './triggers'

// Merge HTTP recovery snapshots and pushed batches without losing newer data.
export function mergeSignals(previous: TriggerSignals | undefined, incoming: TriggerSignals): TriggerSignals {
  if (!previous?.recording) return incoming
  if (!incoming.recording) return previous
  if (previous.recording.id !== incoming.recording.id) {
    return incoming.recording.started_at >= previous.recording.started_at ? incoming : previous
  }
  // Normal live batches append in order; avoid rebuilding/sorting a Map per channel/segment.
  const append = incoming.frames.length && incoming.frames[0]!.time_ms > (previous.frames.at(-1)?.time_ms ?? -Infinity)
    && incoming.frames.every((frame, i) => !i || frame.time_ms > incoming.frames[i - 1]!.time_ms)
  let ordered: TriggerSignal[]
  if (append) ordered = [...previous.frames, ...incoming.frames]
  else {
    const frames = new Map(previous.frames.map(frame => [frame.time_ms, frame]))
    incoming.frames.forEach(frame => frames.set(frame.time_ms, frame))
    ordered = [...frames.values()].sort((a, b) => a.time_ms - b.time_ms)
  }
  const cutoff = (ordered.at(-1)?.time_ms || 0) - 300_000
  const newer = (incoming.recording.trigger_state.updated_at || '') >= (previous.recording.trigger_state.updated_at || '')
  return { ...(newer ? incoming : previous), frames: ordered.filter(frame => frame.time_ms > cutoff).slice(-1200) }
}

export function summarizeSignals(frames: TriggerSignal[]): Record<TriggerMetric, MetricSummary | null> {
  return Object.fromEntries(Object.keys(triggerMetrics).map(key => {
    const metric = key as TriggerMetric
    const values = frames.map(frame => frame[metric]).filter((value): value is number => value != null && Number.isFinite(value)).sort((a, b) => a - b)
    if (!values.length) return [metric, null]
    const percentile = (p: number) => {
      const index = (values.length - 1) * p, lower = Math.floor(index)
      return values[lower]! + (values[Math.ceil(index)]! - values[lower]!) * (index - lower)
    }
    return [metric, { mean: values.reduce((sum, value) => sum + value, 0) / values.length, low: percentile(.1), high: percentile(.9), count: values.length }]
  })) as Record<TriggerMetric, MetricSummary | null>
}

export function signalNumber(value: number | null | undefined) {
  return value == null || !Number.isFinite(value) ? '—' : value.toLocaleString('de-DE', { minimumFractionDigits: 1, maximumFractionDigits: 1 })
}
