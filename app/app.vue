<script setup lang="ts">
import { Aperture, LayoutDashboard, Radio, Clapperboard, Settings2, ArrowUpRight, Menu, X, Check, CircleAlert, Activity, ChevronRight } from '@lucide/vue'
import type { Health } from '~/types'
const route = useRoute()
const mobileNav = ref(false)
const { messages } = useToast()
const { data: health, refresh } = await useFetch<Health>('/api/health', { key: 'health', timeout: 7000, server: false })
usePolling(refresh, 30000, false)
const realtime = useRealtimeState()
const navigation = [
  { path: '/', label: 'Übersicht', icon: LayoutDashboard },
  { path: '/streamers', label: 'Streamer', icon: Radio },
  { path: '/clips', label: 'Clip-Bibliothek', icon: Clapperboard },
  { path: '/settings', label: 'Einstellungen', icon: Settings2 },
]
const currentPage = computed(() => navigation.find(item => item.path === route.path)?.label || 'Studio')
const healthy = computed(() => health.value?.database && health.value?.storage && health.value?.worker)
watch(() => route.path, () => { mobileNav.value = false })
</script>
<template>
  <div class="min-h-dvh">
    <div v-if="mobileNav" class="fixed inset-0 z-40 bg-black/70 lg:hidden" @click="mobileNav = false" />
    <aside class="fixed inset-y-0 left-0 z-50 flex w-[232px] flex-col border-r divider bg-[#141715] transition-transform lg:translate-x-0" :class="mobileNav ? 'translate-x-0' : '-translate-x-full max-lg:hidden'">
      <NuxtLink to="/" class="flex h-[85px] items-center gap-3 px-6" aria-label="NotYetClipped Startseite"><img src="/favicon.svg" alt="" class="size-9" /><div><span class="block text-[15px] font-bold tracking-tight">notyetclipped<span class="text-lime-300">.</span></span><span class="text-[9px] uppercase tracking-[.22em] text-studio-400">Your highlight studio</span></div></NuxtLink>
      <div class="px-4 pt-6"><p class="eyebrow mb-4 px-3">Workspace</p><nav class="space-y-1.5"><NuxtLink v-for="item in navigation" :key="item.path" :to="item.path" class="group flex items-center gap-3 rounded-lg px-3 py-3 text-[13px] font-medium transition" :class="route.path === item.path ? 'bg-lime-300/10 text-lime-300' : 'text-studio-400 hover:bg-studio-800/70 hover:text-zinc-200'"><component :is="item.icon" :size="18" :stroke-width="1.7" />{{ item.label }}<span v-if="route.path === item.path" class="ml-auto size-1.5 rounded-full bg-lime-300" /></NuxtLink></nav></div>
      <div class="mx-4 mt-auto rounded-xl border divider bg-studio-900 p-4"><div class="flex items-center gap-2 text-xs font-medium"><Activity :size="14" class="text-lime-300" />Systemstatus</div><div class="mt-4 space-y-2.5"><div v-for="service in [{ key: 'database' as const, label: 'PostgreSQL' }, { key: 'storage' as const, label: 'S3 Storage' }, { key: 'worker' as const, label: 'Capture Worker' }]" :key="service.key" class="flex items-center justify-between text-[11px] text-studio-400"><span>{{ service.label }}</span><span class="size-1.5 rounded-full" :class="health?.[service.key] ? 'bg-lime-300' : 'bg-amber-300/70'" :aria-label="health?.[service.key] ? 'Verbunden' : 'Nicht verbunden'" /></div></div><NuxtLink to="/settings" class="mt-4 flex items-center justify-between border-t divider pt-3 text-[11px] text-studio-400 hover:text-zinc-200">Konfiguration<ArrowUpRight :size="13" /></NuxtLink></div>
      <div class="mt-5 flex items-center gap-3 border-t divider px-6 py-5"><div class="flex size-8 items-center justify-center rounded-full bg-studio-800 text-[10px] font-bold text-lime-300">ME</div><div><p class="text-xs font-medium">Privates Studio</p><p class="mt-0.5 text-[10px] text-studio-400">Self-hosted · v0.1</p></div><button class="icon-btn ml-auto lg:hidden" aria-label="Menü schließen" @click="mobileNav = false"><X :size="18" /></button></div>
    </aside>
    <div class="lg:pl-[232px]">
      <header class="flex h-[72px] items-center justify-between border-b divider px-5 sm:px-8 xl:px-10">
        <div class="flex items-center gap-3 text-xs"><button class="icon-btn lg:hidden" aria-label="Menü öffnen" @click="mobileNav = true"><Menu :size="20" /></button><span class="hidden text-studio-400 sm:inline">Workspace</span><ChevronRight :size="12" class="hidden text-studio-700 sm:block" /><span class="text-zinc-300">{{ currentPage }}</span></div>
        <div class="flex items-center gap-4"><span class="flex items-center gap-2 text-[11px] text-studio-400"><span class="relative flex size-1.5"><span v-if="healthy" class="absolute inline-flex size-full animate-ping rounded-full bg-lime-300 opacity-40" /><span class="relative inline-flex size-1.5 rounded-full" :class="healthy ? 'bg-lime-300' : 'bg-amber-300'" /></span>{{ healthy ? 'System betriebsbereit' : 'Verbindung ausstehend' }}</span><span class="hidden h-4 w-px bg-studio-700 sm:block" /><div class="hs-dropdown relative inline-flex [--placement:bottom-right]"><button id="studio-menu" type="button" class="hs-dropdown-toggle icon-btn" aria-haspopup="menu" aria-expanded="false" aria-label="Studio-Menü"><Aperture :size="18" /></button><div class="hs-dropdown-menu z-50 mt-2 hidden min-w-48 rounded-xl border divider bg-studio-900 p-1.5 opacity-0 shadow-xl transition-[opacity,margin] hs-dropdown-open:opacity-100" role="menu" aria-labelledby="studio-menu"><NuxtLink to="/streamers" class="flex items-center gap-2 rounded-lg px-3 py-2.5 text-xs text-studio-400 hover:bg-studio-800 hover:text-zinc-100" role="menuitem"><Radio :size="14" />Watchlist öffnen</NuxtLink><NuxtLink to="/settings" class="flex items-center gap-2 rounded-lg px-3 py-2.5 text-xs text-studio-400 hover:bg-studio-800 hover:text-zinc-100" role="menuitem"><Settings2 :size="14" />Studioeinstellungen</NuxtLink></div></div></div>
      </header>
      <main class="mx-auto max-w-[1600px] px-5 pb-12 pt-8 sm:px-8 sm:pt-10 xl:px-10">
        <div v-if="health?.worker_details?.paused_reason" role="alert" class="mb-6 rounded-xl border border-amber-300/20 bg-amber-300/5 p-4 text-sm text-amber-200">{{ health.worker_details.paused_reason }}</div>
        <NuxtPage />
      </main>
      <footer class="mx-5 flex items-center justify-between border-t divider py-5 text-[10px] text-studio-400/70 sm:mx-8 xl:mx-10"><span>NOTYETCLIPPED <span class="mx-2 text-studio-700">/</span> Catch the moment.</span><span class="flex items-center gap-2"><span class="size-1 rounded-full" :class="realtime.connected ? 'bg-lime-300' : 'bg-studio-400'" />{{ realtime.connected ? 'WebSocket verbunden · Live-Updates' : 'Live-Verbindung wird aufgebaut' }}</span></footer>
    </div>
    <div class="fixed bottom-5 right-5 z-[100] flex max-w-sm flex-col gap-2" aria-live="polite"><div v-for="message in messages" :key="message.id" class="flex items-start gap-3 rounded-xl border bg-studio-850 px-4 py-3 text-sm shadow-xl" :class="message.error ? 'border-red-400/30 text-red-200' : 'border-lime-300/30 text-zinc-200'"><CircleAlert v-if="message.error" :size="18" class="mt-0.5 shrink-0" /><Check v-else :size="18" class="mt-0.5 shrink-0 text-lime-300" />{{ message.text }}</div></div>
  </div>
</template>
