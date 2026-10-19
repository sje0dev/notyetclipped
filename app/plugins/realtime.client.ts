import type { Dashboard, SignalEvent, Streamer } from '~/types'

export default defineNuxtPlugin((nuxtApp) => {
  const realtime = useRealtimeState()
  const signals = useSignalFeed()
  const { data: dashboard } = useNuxtData<Dashboard>('dashboard')
  const { data: streamers } = useNuxtData<Streamer[]>('streamers')
  const clipEvent = useState<{ id: string | null; at: number }>('clip-event', () => ({ id: null, at: 0 }))
  let socket: WebSocket | undefined
  let reconnect: ReturnType<typeof setTimeout> | undefined
  let refreshTimer: ReturnType<typeof setTimeout> | undefined
  let attempts = 0
  let stopped = false
  const pending = new Set<string>()

  function schedule(keys: string[]) {
    keys.forEach(key => pending.add(key))
    if (refreshTimer) return
    refreshTimer = setTimeout(() => {
      const keys = [...pending]
      pending.clear()
      refreshTimer = undefined
      void nuxtApp.runWithContext(() => refreshNuxtData(keys))
    }, 100)
  }

  function connect() {
    if (stopped) return
    socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/ws`)
    socket.onmessage = (message) => {
      let event: { type: string; id?: string }
      try { event = JSON.parse(message.data) } catch { return }
      if (event.type === 'signals.updated') {
        const update = event as SignalEvent
        signals.value[update.id] = mergeSignals(signals.value[update.id], update)
        const apply = (streamer: Streamer): Streamer => {
          if (streamer.id !== update.id || streamer.recording_id !== update.recording?.id) return streamer
          return { ...streamer, audio_db: update.audio_db, buffer_seconds: update.buffer_seconds, trigger_state: update.recording.trigger_state }
        }
        if (streamers.value) streamers.value = streamers.value.map(apply)
        if (dashboard.value) dashboard.value = { ...dashboard.value, streamers: dashboard.value.streamers.map(apply) }
        return
      }
      if (event.type === 'hello') {
        attempts = 0
        realtime.value = { connected: true, reconnecting: false, revision: realtime.value.revision + 1 }
      }
      if (event.type === 'resync') realtime.value.revision++
      if (['hello', 'resync', 'system.updated'].includes(event.type)) schedule(['dashboard', 'streamers', 'clips', 'health', 'storage'])
      else if (event.type === 'stream.updated') schedule(['dashboard', 'streamers', 'clips'])
      else if (event.type === 'clip.updated') {
        clipEvent.value = { id: event.id || null, at: Date.now() }
        schedule(['dashboard', 'clips', 'storage'])
      }
      else if (event.type === 'settings.updated') schedule(['dashboard', 'health'])
      else if (event.type === 'ai.updated') schedule(['ai'])
    }
    socket.onerror = () => socket?.close()
    socket.onclose = () => {
      realtime.value = { ...realtime.value, connected: false, reconnecting: true }
      if (!stopped) reconnect = setTimeout(connect, Math.min(20000, 1000 * 2 ** Math.min(attempts++, 5)) + Math.random() * 500)
    }
  }
  function stop() {
    stopped = true
    clearTimeout(reconnect)
    clearTimeout(refreshTimer)
    socket?.close()
  }
  nuxtApp.hook('app:mounted', connect)
  window.addEventListener('beforeunload', stop, { once: true })
  import.meta.hot?.dispose(stop)
})
