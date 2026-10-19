<script setup lang="ts">
import { Plus, Radio, Clapperboard, HardDrive, Workflow, ArrowUpRight, ArrowRight, CircleDot } from '@lucide/vue'
import type { Dashboard, Streamer } from '~/types'
const { data, error, status, refresh } = await useFetch<Dashboard>('/api/dashboard', { key: 'dashboard', timeout: 8000 })
usePolling(refresh)
const formOpen = ref(false)
const editing = ref<Streamer | null>(null)
const selectedClip = ref<string | null>(null)
const stats = computed(() => [
  { label: 'Aktive Aufnahmen', value: data.value?.stats.recording ?? '—', sub: `${data.value?.stats.streamers || 0} Streamer verbunden`, icon: Radio, color: 'text-mint bg-mint/10' },
  { label: 'Clips heute', value: data.value?.stats.clips_today ?? '—', sub: `${data.value?.stats.clips_ready || 0} Clips in der Bibliothek`, icon: Clapperboard, color: 'text-accent bg-accent/10' },
  { label: 'In Verarbeitung', value: data.value?.stats.pending ?? '—', sub: data.value?.stats.failed ? `${data.value.stats.failed} fehlgeschlagen` : 'Schnitt und Upload', icon: Workflow, color: 'text-lavender bg-lavender/10' },
  { label: 'S3-Speicher', value: data.value ? bytes(data.value.stats.stored_bytes) : '—', sub: 'Privat gespeicherte Clips', icon: HardDrive, color: 'text-amber-300 bg-amber-300/10' },
])
const trendSeries = computed(() => [
  { name: 'Automatisch', data: (data.value?.trend || []).map(day => ({ x: new Date(`${day.day}T12:00:00Z`).toLocaleDateString('de-DE', { weekday: 'short', timeZone: 'UTC' }), y: day.automatic })) },
  { name: 'Manuell', data: (data.value?.trend || []).map(day => ({ x: new Date(`${day.day}T12:00:00Z`).toLocaleDateString('de-DE', { weekday: 'short', timeZone: 'UTC' }), y: day.manual })) },
])
const weekTotal = computed(() => (data.value?.trend || []).reduce((sum, day) => sum + day.automatic + day.manual, 0))
const automaticTotal = computed(() => (data.value?.trend || []).reduce((sum, day) => sum + day.automatic, 0))
function edit(streamer: Streamer) { editing.value = streamer; formOpen.value = true }
function add() { editing.value = null; formOpen.value = true }
</script>
<template>
  <div>
    <div class="mb-7 flex flex-wrap items-center justify-between gap-4"><div><h1 class="page-title">Übersicht</h1><p class="mt-2 text-sm text-studio-400">Deine Aufnahmen, Automationen und letzten Clips im Überblick.</p></div><button class="btn btn-primary" @click="add"><Plus :size="16" />Streamer hinzufügen</button></div>
    <div class="mb-7 grid grid-cols-2 gap-4 xl:grid-cols-4"><div v-for="stat in stats" :key="stat.label" class="panel p-5"><div class="flex items-center justify-between gap-2 text-xs text-studio-400"><span>{{ stat.label }}</span><span class="flex size-8 shrink-0 items-center justify-center rounded-lg" :class="stat.color"><component :is="stat.icon" :size="16" /></span></div><div class="mt-3 text-3xl font-semibold tracking-tight">{{ stat.value }}</div><p class="mt-2 text-[11px] text-studio-400">{{ stat.sub }}</p></div></div>
    <ConnectionError v-if="error" @retry="refresh" />
    <div v-else-if="status === 'pending' && !data" class="panel p-16 text-center text-sm text-studio-400">Studio wird geladen …</div>
    <template v-else>
      <section class="panel mb-7 grid overflow-hidden lg:grid-cols-[minmax(0,1fr)_260px]"><div class="min-w-0 p-5 pb-2"><div class="flex flex-wrap items-center justify-between gap-3"><div><h2 class="text-sm font-semibold">Deine Clip-Woche</h2><p class="mt-1 text-[11px] text-studio-400">Erstellte Clips · letzte 7 Tage · UTC</p></div><div class="flex gap-4 text-[10px] text-studio-400"><span class="chart-legend before:bg-accent">Automatisch</span><span class="chart-legend before:bg-mint">Manuell</span></div></div><StudioChart :series="trendSeries" type="bar" :datetime="false" :height="205" :min="0" unit="Clips" label="Automatische und manuelle Clips der letzten sieben Tage" /></div><div class="flex flex-col justify-center border-t divider bg-linear-to-br from-accent/[.06] to-lavender/[.04] p-6 lg:border-l lg:border-t-0"><p class="text-[11px] text-studio-400">Momente festgehalten</p><p class="mt-2 text-4xl font-semibold tracking-tight">{{ weekTotal }}</p><p class="mt-2 text-xs text-studio-400"><span class="font-medium text-mint">{{ weekTotal ? Math.round(automaticTotal / weekTotal * 100) : 0 }} %</span> automatisch erkannt</p><div class="my-5 h-1.5 overflow-hidden rounded-full bg-studio-800"><div class="h-full rounded-full bg-linear-to-r from-accent to-lavender" :style="{ width: `${weekTotal ? automaticTotal / weekTotal * 100 : 0}%` }" /></div><NuxtLink to="/streamers" class="flex items-center justify-between text-xs font-medium text-accent">Streamer-Trigger verfeinern<ArrowRight :size="14" /></NuxtLink></div></section>
      <section class="mb-8"><div class="mb-4 flex items-center justify-between"><h2 class="text-sm font-semibold">Deine Streamer <span class="ml-1 text-xs font-normal text-studio-400">{{ data?.streamers.length || 0 }}</span></h2><NuxtLink to="/streamers" class="flex items-center gap-1.5 text-xs text-studio-400 hover:text-zinc-200">Alle anzeigen<ArrowUpRight :size="13" /></NuxtLink></div><div v-if="data?.streamers.length" class="grid gap-4 md:grid-cols-2 2xl:grid-cols-3"><StreamerCard v-for="streamer in data.streamers.slice(0, 3)" :key="streamer.id" :streamer="streamer" @edit="edit" @changed="refresh" /></div><div v-else class="panel flex flex-col items-center gap-3 p-10 text-center"><Radio :size="25" class="text-studio-400" /><h3 class="text-sm font-medium">Noch keine Streamer</h3><p class="text-xs text-studio-400">Füge einen Twitch-Kanal hinzu. Die Aufnahme startet, sobald er live ist.</p><button class="btn btn-secondary mt-2" @click="add"><Plus :size="14" />Streamer hinzufügen</button></div></section>
      <div>
        <section><div class="mb-4 flex items-center justify-between"><h2 class="text-sm font-semibold">Letzte Clips</h2><NuxtLink to="/clips" class="flex items-center gap-1.5 text-xs text-studio-400 hover:text-zinc-200">Zur Bibliothek<ArrowUpRight :size="13" /></NuxtLink></div><div v-if="data?.clips.length" class="grid gap-4 sm:grid-cols-2"><ClipCard v-for="clip in data.clips.slice(0, 4)" :key="clip.id" :clip="clip" @open="selectedClip = $event" @changed="refresh" /></div><div v-else class="panel flex min-h-56 flex-col items-center justify-center gap-3 p-8 text-center"><Clapperboard :size="24" class="text-studio-400" /><h3 class="text-sm">Noch keine Clips</h3><p class="max-w-xs text-xs leading-relaxed text-studio-400">Automatisch ausgelöste und manuell gesicherte Clips erscheinen hier.</p></div></section>
      </div>
    </template>
    <StreamerForm :open="formOpen" :streamer="editing" @close="formOpen = false" @saved="refresh" /><ClipPlayer :clip-id="selectedClip" @close="selectedClip = null" @changed="refresh" />
  </div>
</template>
