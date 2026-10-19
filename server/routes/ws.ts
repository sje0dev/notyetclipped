const upstreams = new Map<string, WebSocket>()

export default defineWebSocketHandler({
  open(peer) {
    const origin = peer.request.headers.get('origin')
    if (origin && new URL(origin).host !== peer.request.headers.get('host')) {
      peer.close(1008, 'Origin rejected')
      return
    }
    const base = useRuntimeConfig().apiBase.replace(/^http/, 'ws').replace(/\/$/, '')
    const upstream = new WebSocket(`${base}/api/ws`)
    upstreams.set(peer.id, upstream)
    upstream.onmessage = (event) => { peer.send(String(event.data)) }
    upstream.onclose = () => { upstreams.delete(peer.id); peer.close(1012, 'Backend disconnected') }
    upstream.onerror = () => { upstream.close(); peer.close(1011, 'Backend unavailable') }
  },
  message(peer, message) {
    const upstream = upstreams.get(peer.id)
    if (upstream?.readyState === WebSocket.OPEN && message.text() === 'ping') upstream.send('ping')
  },
  close(peer) {
    const upstream = upstreams.get(peer.id)
    upstreams.delete(peer.id)
    upstream?.close()
  },
  error(peer) {
    const upstream = upstreams.get(peer.id)
    upstreams.delete(peer.id)
    upstream?.close()
  },
})
