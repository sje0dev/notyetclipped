export default defineEventHandler(async (event) => {
  const path = getRouterParam(event, 'path') || ''
  const method = getMethod(event)
  if (!['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    if (getHeader(event, 'x-requested-with') !== 'notyetclipped') {
      throw createError({ statusCode: 403, statusMessage: 'Missing application header' })
    }
    const origin = getHeader(event, 'origin')
    if (origin && origin !== getRequestURL(event).origin) {
      throw createError({ statusCode: 403, statusMessage: 'Cross-origin request rejected' })
    }
  }
  const base = useRuntimeConfig(event).apiBase.replace(/\/$/, '')
  // The upstream host is fixed configuration, never user input.
  const query = getRequestURL(event).search
  return proxyRequest(event, `${base}/api/${path}${query}`, {
    headers: { 'x-requested-with': 'notyetclipped' },
  })
})
