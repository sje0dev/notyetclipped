import type { NitroFetchOptions } from 'nitropack'

export function api<T>(path: string, options: NitroFetchOptions<string> = {}) {
  const headers = new Headers(options.headers)
  headers.set('x-requested-with', 'notyetclipped')
  return $fetch<T>(`/api${path}`, {
    ...options,
    headers,
  })
}

export function useRealtimeState() {
  return useState('realtime', () => ({ connected: false, reconnecting: false, revision: 0 }))
}

export function useSignalFeed() {
  return useState<Record<string, import('~/types').TriggerSignals>>('signal-feed', () => ({}))
}

export function usePolling(callback: () => unknown, interval = 15000, fallbackOnly = true) {
  const realtime = useRealtimeState()
  let timer: ReturnType<typeof setInterval> | undefined
  let running = false
  onMounted(() => {
    timer = setInterval(async () => {
      if (document.hidden || running || (fallbackOnly && realtime.value.connected)) return
      running = true
      try { await callback() } catch { /* Pages display their fetch errors. */ }
      finally { running = false }
    }, interval)
  })
  onBeforeUnmount(() => clearInterval(timer))
}

export function useToast() {
  const messages = useState<{ id: number; text: string; error: boolean }[]>('toasts', () => [])
  function toast(text: string, error = false) {
    const id = Date.now() + Math.random()
    messages.value.push({ id, text, error })
    setTimeout(() => { messages.value = messages.value.filter(item => item.id !== id) }, 6000)
  }
  return { messages, toast }
}
