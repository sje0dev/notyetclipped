import tailwindcss from '@tailwindcss/vite'

export default defineNuxtConfig({
  compatibilityDate: '2026-10-19',
  devtools: { enabled: false },
  nitro: { preset: 'bun', experimental: { websocket: true } },
  css: ['~/assets/css/main.css'],
  vite: { plugins: [tailwindcss()] },
  runtimeConfig: {
    apiBase: 'http://127.0.0.1:8000',
  },
  app: {
    head: {
      title: 'NotYetClipped — Catch the moment.',
      htmlAttrs: { lang: 'de', class: 'dark' },
      meta: [{ name: 'description', content: 'Dein privates Studio für automatische Twitch-Highlights.' }],
      link: [{ rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' }],
    },
  },
})
