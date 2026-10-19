<script setup lang="ts">
import { ListTodo, X, Download, ArrowLeft, RefreshCw } from '@lucide/vue'
interface Job { id: string; kind: string; label: string; status: string; active: boolean; created_at: string; finished_at: string | null; message: string | null }
const root = ref<HTMLElement>()
const button = ref<HTMLButtonElement>()
const open = ref(false)
const selected = ref<Job | null>(null)
const logs = ref<{ id: number; level: string; message: string; created_at: string }[]>([])
const logError = ref('')
const { data, error, refresh: refreshData } = await useFetch<Job[]>('/api/jobs', { key: 'jobs', server: false })
const refresh = () => refreshData()
const { data: ai, refresh: refreshAi } = await useFetch<import('~/types').AiStatus>('/api/ai', { key: 'ai', server: false })
const active = computed(() => data.value?.filter(j => j.active) || [])
const aiState = computed(() => !ai.value ? 'Lade Status …' : ai.value.paused_reason || (!ai.value.settings.enabled ? 'ausgeschaltet' : active.value.some(j => j.kind === 'ai') ? 'Analyse läuft' : ai.value.requests_day >= ai.value.settings.daily_limit ? 'Tageslimit erreicht' : ai.value.requests_hour >= ai.value.settings.hourly_limit ? 'Stundenlimit erreicht' : 'wartet auf fälligen Kanal / Budget / freien Clip-Abstand'))
const labels: Record<string, string> = { recording: 'Aufnahme läuft', connecting: 'Verbinden', waiting: 'Wartet', collecting: 'Nachlauf sammeln', queued: 'In Warteschlange', preparing: 'Sprache & Bilder', analyzing: 'ChatGPT wertet aus', cutting: 'Clip schneiden', uploading: 'S3-Upload', rendering: 'Hochkant rendern', ready: 'Fertig', clipped: 'Moment gesichert', skipped: 'Übersprungen', failed: 'Fehler', retry: 'Wiederholung wartet', deleting: 'Wird gelöscht', finished: 'Beendet' }
const now = ref(Date.now())
let timer: ReturnType<typeof setInterval> | undefined
let loadingLogs = false
async function loadLogs() {
  if (!selected.value || loadingLogs) return
  const id = selected.value.id; loadingLogs = true
  try { const result = await api<typeof logs.value>(`/jobs/${id}/logs`); if (selected.value?.id === id) { logs.value = result; logError.value = '' } }
  catch (e) { logError.value = errorMessage(e) }
  finally { loadingLogs = false }
}
watch(selected, () => { logs.value = []; void loadLogs() })
watch(open, value => { if (value) { void refresh(); void refreshAi(); void loadLogs() } })
function close() { open.value = false; button.value?.focus() }
function outside(event: PointerEvent) { if (!root.value?.contains(event.target as Node)) open.value = false }
function elapsed(job: Job) { const latest = data.value?.find(j => j.id === job.id) || job; return duration(Math.max(0, ((latest.finished_at ? Date.parse(latest.finished_at) : now.value) - Date.parse(latest.created_at)) / 1000)) }
onMounted(() => {
  document.addEventListener('pointerdown', outside)
  timer = setInterval(() => { now.value = Date.now(); if (open.value && !document.hidden) { void refresh(); void refreshAi(); void loadLogs() } }, 5000)
})
onBeforeUnmount(() => { clearInterval(timer); document.removeEventListener('pointerdown', outside) })
</script>
<template>
  <div ref="root" class="relative" @keydown.esc.stop="close">
    <button ref="button" class="icon-btn relative" aria-label="Jobs und Logs anzeigen" :aria-expanded="open" aria-controls="jobs-popover" @click="open = !open"><ListTodo :size="19" /><span v-if="active.length" class="absolute -right-1 -top-1 rounded-full bg-accent px-1 text-[9px] text-white">{{ active.length }}</span></button>
    <section v-if="open" id="jobs-popover" aria-label="Jobs und Logs" class="panel fixed right-3 top-[72px] z-50 w-[min(600px,calc(100vw-24px))] overflow-hidden shadow-2xl sm:absolute sm:right-0 sm:top-11">
      <div class="flex items-center gap-3 border-b divider p-4"><button v-if="selected" class="icon-btn" aria-label="Zurück zur Jobliste" @click="selected = null"><ArrowLeft :size="16" /></button><h2 class="min-w-0 flex-1 truncate text-sm font-semibold">{{ selected ? selected.label : `Jobs · ${active.length} aktiv` }}</h2><a v-if="selected" :href="`/api/jobs/${selected.id}/logs?download=true`" class="icon-btn" aria-label="Job-Log herunterladen"><Download :size="16" /></a><button class="icon-btn" aria-label="Jobs schließen" @click="close"><X :size="16" /></button></div>
      <div class="max-h-[70dvh] overflow-y-auto p-4">
        <template v-if="selected"><p class="mb-3 text-xs text-studio-400">{{ labels[(data?.find(j => j.id === selected?.id) || selected).status] || selected.status }} · {{ elapsed(selected) }} · letzte 500 Protokolleinträge</p><p v-if="logError" class="text-xs text-red-300">{{ logError }}</p><p v-else-if="!logs.length" class="hint">Noch keine Protokolleinträge. Wartende Jobs starten, sobald Kapazität frei ist. Ältere Jobs enthalten erst seit dem Update Protokolle.</p><div class="space-y-3 font-mono text-[11px]"><div v-for="line in logs" :key="line.id" class="break-words rounded-lg bg-studio-950 p-3" :class="line.level === 'error' ? 'text-red-300' : 'text-studio-300'"><span class="mb-1 block text-[10px] text-studio-400">{{ dateTime(line.created_at) }} · {{ line.level }}</span>{{ line.message }}</div></div></template>
        <template v-else>
          <p v-if="error" class="hint">Jobliste konnte nicht geladen werden. <button class="underline" @click="refresh">Erneut laden</button></p>
          <div v-if="ai" class="mb-4 rounded-lg bg-studio-950 p-3 text-xs text-studio-400">KI: {{ aiState }}<p class="mt-1">{{ ai.requests_hour }}/{{ ai.settings.hourly_limit }} Anfragen/h · {{ ai.scheduler?.active_streamers || 0 }} KI-Kanäle</p></div>
          <template v-for="group in [{ title: 'Aktiv & wartend', rows: active }, { title: 'Kürzlich abgeschlossen', rows: (data || []).filter(j => !j.active).slice(0, 20) }]" :key="group.title"><h3 class="my-3 text-[11px] font-medium uppercase tracking-wide text-studio-400">{{ group.title }}</h3><p v-if="!group.rows.length" class="hint">Keine Jobs.</p><button v-for="job in group.rows" :key="job.id" class="mb-2 flex w-full items-start gap-3 rounded-lg border divider p-3 text-left hover:bg-studio-850" @click="selected = job"><RefreshCw :size="14" class="mt-0.5 shrink-0" :class="job.active ? 'text-accent' : 'text-studio-400'" /><span class="min-w-0 flex-1"><span class="block truncate text-xs">{{ job.label }}</span><span class="mt-1 block text-[11px]" :class="job.status === 'failed' ? 'text-red-300' : 'text-studio-400'">{{ labels[job.status] || job.status }} · {{ elapsed(job) }}</span><span v-if="job.message" class="mt-1 block line-clamp-2 text-[11px] text-studio-400">{{ job.message }}</span></span><span class="text-[10px] text-accent">Logs →</span></button></template>
        </template>
      </div>
    </section>
  </div>
</template>
