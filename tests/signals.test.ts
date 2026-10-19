import { describe, expect, test } from 'bun:test'
import { mergeSignals, summarizeSignals } from '../app/utils/signals'
import type { TriggerSignal, TriggerSignals } from '../app/types'

const frame = (time_ms: number, messages: number | null = null): TriggerSignal => ({ time_ms, audio_level: -30, audio_rise: 2, baseline_db: -32, calibrated_seconds: 90, chat_messages: messages, chat_users: null, chat_rate: null })
const snapshot = (frames: TriggerSignal[], updated = '2026-10-19T10:05:00Z', id = 'recording-1', started = '2026-10-19T10:00:00Z'): TriggerSignals => ({ recording: { id, started_at: started, ended_at: null, trigger_state: { updated_at: updated, signal: frames.at(-1) } }, frames, hourly_count: 1, last_trigger_at: null })

describe('live measurement recovery', () => {
  test('a late HTTP response fills history without reverting pushed readings or duplicating samples', () => {
    const pushed = snapshot([frame(300000, 20), frame(300250, 30)])
    const http = snapshot([frame(299750, 10), frame(300000, 20)], '2026-10-19T10:04:59Z')
    const merged = mergeSignals(pushed, http)
    expect(merged.frames.map(frame => frame.time_ms)).toEqual([299750, 300000, 300250])
    expect(merged.recording?.trigger_state.signal?.chat_messages).toBe(30)
  })
  test('keeps only five minutes and resets the timeline on a new recording', () => {
    const old = snapshot([frame(0), frame(1000), frame(300000)])
    const merged = mergeSignals(old, snapshot([frame(301000)]))
    expect(merged.frames.map(frame => frame.time_ms)).toEqual([300000, 301000])
    const next = snapshot([frame(0)], '2026-10-19T11:00:02Z', 'recording-2', '2026-10-19T11:00:00Z')
    expect(mergeSignals(merged, next)).toEqual(next)
    expect(mergeSignals(next, merged)).toEqual(next)
  })
  test('counts real zeros, excludes missing chat observations, and handles empty histories', () => {
    const summary = summarizeSignals([frame(0, null), frame(250, 0), frame(500, 10)])
    expect(summary.chat_messages?.mean).toBe(5)
    expect(summary.chat_messages?.count).toBe(2)
    expect(summary.chat_users).toBeNull()
    expect(summarizeSignals([]).audio_level).toBeNull()
  })
})
