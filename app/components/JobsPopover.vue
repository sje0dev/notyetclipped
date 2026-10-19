<script setup lang="ts">
import { ListTodo, X, ArrowRight, RefreshCw } from '@lucide/vue'
import type { Job, AiStatus } from '~/types'
const root = ref<HTMLElement>()
const button = ref<HTMLButtonElement>()
const open = ref(false)
const route = useRoute()
const { data, error, refresh: refreshData } = await useFetch<Job[]>('/api/jobs', { key: 'jobs', server: false })
const refresh = () => refreshData()
const { data: ai, refresh: refreshAi } = await useFetch<AiStatus>('/api/ai', { key: 'ai', server: false })
const active = computed(() => data.value?.filter(j => j.active) || [])
const aiState = computed(() => !ai.value ? 'Lade Status …' : ai.value.paused_reason || (!ai.value.settings.enabled ? 'ausgeschaltet' : active.value.some(j => j.kind === 'ai') ? 'Analyse läuft' : ai.value.requests_day >= ai.value.settings.daily_limit ? 'Tageslimit erreicht' : ai.value.requests_hour >= ai.value.settings.hourly_limit ? 'Stundenlimit erreicht' : 'wartet auf fälligen Kanal / Budget / freien Clip-Abstand'))
const now = ref(Date.now())
watch(open, value => { if (value) { void refresh(); void refreshAi(); now.value = Date.now() } })
watch(() => route.fullPath, () => { open.value = false })
usePolling(async () => {
  now.value = Date.now()
  if (open.value) await Promise.all([route.path === '/jobs' ? Promise.resolve() : refresh(), refreshAi()])
}, 5000, false)
function close() { open.value = false; button.value?.focus() }
function outside(event: PointerEvent) { if (!root.value?.contains(event.target as Node)) open.value = false }
onMounted(() => document.addEventListener('pointerdown', outside))
onBeforeUnmount(() => document.removeEventListener('pointerdown', outside))
</script>
<template>
  <div ref="root" class="relative" @keydown.esc.stop="close">
    <button ref="button" class="icon-btn relative" aria-label="Jobs und Logs anzeigen" :aria-expanded="open" aria-controls="jobs-popover" @click="open = !open"><ListTodo :size="19" /><span v-if="active.length" class="absolute -right-1 -top-1 rounded-full bg-accent px-1 text-[9px] text-white">{{ active.length }}</span></button>
    <section v-if="open" id="jobs-popover" aria-label="Jobs und Logs" class="panel fixed right-3 top-[72px] z-50 w-[min(600px,calc(100vw-24px))] overflow-hidden shadow-2xl sm:absolute sm:right-0 sm:top-11">
      <div class="flex items-center gap-3 border-b divider p-4"><h2 class="min-w-0 flex-1 truncate text-sm font-semibold">Jobs · {{ active.length }} aktiv</h2><button class="icon-btn" aria-label="Jobs schließen" @click="close"><X :size="16" /></button></div>
      <div class="max-h-[60dvh] overflow-y-auto p-4">
        <p v-if="error" class="hint">Jobliste konnte nicht geladen werden. <button class="underline" @click="refresh">Erneut laden</button></p>
        <div v-if="ai" class="mb-4 rounded-lg bg-studio-950 p-3 text-xs text-studio-400">KI: {{ aiState }}<p class="mt-1">{{ ai.requests_hour }}/{{ ai.settings.hourly_limit }} Anfragen/h · {{ ai.scheduler?.active_streamers || 0 }} KI-Kanäle</p></div>
        <template v-for="group in [{ title: 'Aktiv & wartend', rows: active }, { title: 'Kürzlich abgeschlossen', rows: (data || []).filter(j => !j.active).slice(0, 20) }]" :key="group.title">
          <h3 class="my-3 text-[11px] font-medium uppercase tracking-wide text-studio-400">{{ group.title }}</h3><p v-if="!group.rows.length" class="hint">Keine Jobs.</p>
          <NuxtLink v-for="job in group.rows" :key="job.id" :to="jobLink(job.id)" class="mb-2 flex w-full items-start gap-3 rounded-lg border divider p-3 text-left hover:bg-studio-850" @click="open = false">
            <RefreshCw :size="14" class="mt-0.5 shrink-0" :class="job.active ? 'text-accent' : 'text-studio-400'" /><span class="min-w-0 flex-1"><span class="block truncate text-xs">{{ job.label }}</span><span class="mt-1 block text-[11px]" :class="job.status === 'failed' ? 'text-red-300' : 'text-studio-400'">{{ jobStatusLabels[job.status] || job.status }} · {{ jobDuration(job, now) }}</span><span v-if="job.message" class="mt-1 block line-clamp-2 text-[11px] text-studio-400">{{ job.message }}</span></span><span class="text-[10px] text-accent">Logs →</span>
          </NuxtLink>
        </template>
      </div>
      <NuxtLink to="/jobs" class="flex items-center justify-between border-t divider px-4 py-3 text-xs font-medium text-accent hover:bg-studio-850" @click="open = false">Alle Jobs anzeigen<ArrowRight :size="15" /></NuxtLink>
    </section>
  </div>
</template>
