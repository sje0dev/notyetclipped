export default defineNuxtPlugin((nuxtApp) => {
  const realtime = useRealtimeState()
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
    }, 350)
  }

  function connect() {
    if (stopped) return
    socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/ws`)
    socket.onmessage = (message) => {
      let event: { type: string; id?: string }
      try { event = JSON.parse(message.data) } catch { return }
      if (event.type === 'hello') {
        attempts = 0
        realtime.value = { connected: true, reconnecting: false }
      }
      if (['hello', 'resync', 'system.updated'].includes(event.type)) schedule(['dashboard', 'streamers', 'clips', 'health', 'storage'])
      else if (event.type === 'stream.updated') schedule(['dashboard', 'streamers', 'clips'])
      else if (event.type === 'clip.updated') {
        clipEvent.value = { id: event.id || null, at: Date.now() }
        schedule(['dashboard', 'clips', 'storage'])
      }
      else if (event.type === 'settings.updated') schedule(['dashboard', 'health'])
    }
    socket.onerror = () => socket?.close()
    socket.onclose = () => {
      realtime.value = { connected: false, reconnecting: true }
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
