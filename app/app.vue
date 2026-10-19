<script setup lang="ts">
import { LayoutDashboard, Radio, Clapperboard, Settings2, Film, Menu, X, Check, CircleAlert, ChevronRight, PanelLeft, Search } from '@lucide/vue'
import type { Health } from '~/types'
const route = useRoute()
const mobileNav = ref(false)
const search = ref('')
function searchClips() { void navigateTo({ path: '/clips', query: { q: search.value } }); mobileNav.value = false }
const { messages } = useToast()
const { data: health, refresh } = await useFetch<Health>('/api/health', { key: 'health', timeout: 7000, server: false })
usePolling(refresh, 30000, false)
const realtime = useRealtimeState()
const navigation = [
  { path: '/', label: 'Übersicht', icon: LayoutDashboard },
  { path: '/streamers', label: 'Streamer', icon: Radio },
  { path: '/clips', label: 'Clip-Bibliothek', icon: Clapperboard },
  { path: '/edits', label: 'Schnittliste', icon: Film },
]
const currentPage = computed(() => route.path.startsWith('/streamers/') ? 'Streamer' : navigation.find(item => item.path === route.path)?.label || 'Einstellungen')
const activePath = computed(() => route.path.startsWith('/streamers/') ? '/streamers' : route.path)
watch(() => route.path, () => { mobileNav.value = false })
</script>
<template>
  <div class="min-h-dvh">
    <div v-if="mobileNav" class="fixed inset-0 z-40 bg-black/60 lg:hidden" @click="mobileNav = false" />
    <aside class="fixed inset-y-0 left-0 z-50 flex w-[248px] flex-col border-r divider bg-studio-900 transition-transform lg:translate-x-0" :class="mobileNav ? 'translate-x-0' : '-translate-x-full max-lg:hidden'">
      <div class="flex h-16 items-center gap-2.5 border-b divider px-5"><NuxtLink to="/" class="flex items-center gap-2.5" aria-label="NotYetClipped Startseite"><img src="/favicon.svg" alt="" class="size-7" /><span class="text-sm font-semibold tracking-tight">notyetclipped</span></NuxtLink><button type="button" class="icon-btn ml-auto lg:hidden" aria-label="Menü schließen" @click="mobileNav = false"><X :size="16" /></button></div>
      <form class="relative mx-4 mt-6" @submit.prevent="searchClips"><Search :size="16" class="pointer-events-none absolute left-3 top-3 text-studio-400" /><input v-model="search" class="input bg-studio-950 pl-9" aria-label="Clips im Studio suchen" placeholder="Clips suchen …" /></form>
       <div class="px-3 pt-8"><p class="mb-3 px-3 text-[11px] font-medium uppercase tracking-wider text-studio-400">Dein Studio</p><nav class="space-y-1.5"><NuxtLink v-for="item in navigation" :key="item.path" :to="item.path" class="flex items-center gap-3 rounded-lg px-3 py-3 text-sm transition-colors" :class="activePath === item.path ? 'bg-white/10 font-medium text-white' : 'text-studio-400 hover:bg-studio-850 hover:text-zinc-200'"><component :is="item.icon" :size="19" :stroke-width="1.7" :class="{ 'text-accent': activePath === item.path }" />{{ item.label }}</NuxtLink></nav></div>
      <div class="mt-auto px-3 pb-4"><NuxtLink to="/settings" class="flex items-center gap-3 rounded-lg px-3 py-2.5 text-[13px]" :class="route.path === '/settings' ? 'bg-accent/10 text-accent' : 'text-studio-400 hover:bg-studio-850'"><Settings2 :size="17" />Einstellungen</NuxtLink></div>
      <div class="flex items-center gap-3 border-t divider px-5 py-4"><div class="flex size-7 items-center justify-center rounded-full bg-studio-800 text-[10px] text-studio-400">ME</div><div><p class="text-xs">Lokales Studio</p><p class="mt-0.5 text-[10px] text-studio-400">Self-hosted</p></div></div>
    </aside>
    <div class="lg:pl-[248px]">
      <header class="sticky top-0 z-30 flex h-16 items-center justify-between border-b divider bg-studio-950/95 px-5 backdrop-blur sm:px-8">
        <div class="flex items-center gap-3 text-xs"><button class="icon-btn lg:hidden" aria-label="Menü öffnen" @click="mobileNav = true"><Menu :size="19" /></button><PanelLeft :size="16" class="hidden text-studio-400 lg:block" /><span class="hidden h-4 w-px bg-studio-700 lg:block" /><span class="hidden text-studio-400 sm:inline">Workspace</span><ChevronRight :size="12" class="hidden text-studio-400 sm:block" /><span>{{ currentPage }}</span></div>
        <div class="flex items-center gap-3"><JobsPopover /><ActivityPopover /><span class="flex items-center gap-2 rounded-full border divider px-2.5 py-1 text-[10px] text-studio-400" :title="realtime.connected ? 'Messwerte werden direkt per WebSocket übertragen' : 'Live-Verbindung wird wiederhergestellt'"><span class="size-1.5 rounded-full" :class="realtime.connected ? 'bg-mint' : 'bg-amber-300'" />{{ realtime.connected ? 'Live' : 'Verbinde …' }}</span></div>
      </header>
      <main class="mx-auto max-w-[1560px] px-5 py-8 sm:px-8">
        <div v-if="health?.worker_details?.paused_reason" role="alert" class="mb-6 rounded-lg border border-amber-300/20 bg-amber-300/5 p-4 text-sm text-amber-200">{{ health.worker_details.paused_reason }}</div>
        <NuxtPage />
      </main>
    </div>
    <div class="fixed right-5 top-20 z-[100] flex max-w-sm flex-col gap-2" aria-live="polite"><div v-for="message in messages" :key="message.id" class="flex items-start gap-3 rounded-xl border bg-studio-850 px-4 py-3 text-sm shadow-xl" :class="message.error ? 'border-red-400/30 text-red-200' : 'border-studio-700 text-zinc-200'"><CircleAlert v-if="message.error" :size="18" class="mt-0.5 shrink-0" /><Check v-else :size="18" class="mt-0.5 shrink-0 text-mint" />{{ message.text }}</div></div>
  </div>
</template>
