<script setup lang="ts">
import { ListTodo, RefreshCw, Search, FileText } from '@lucide/vue'
import type { Job } from '~/types'
const route = useRoute()
const { data, error, status, refresh: refreshData } = await useFetch<Job[]>('/api/jobs', { key: 'jobs', server: false })
const refresh = () => refreshData()
const now = ref(Date.now())
usePolling(async () => { now.value = Date.now(); await refresh() }, 5000, false)
const filter = ref('all')
const kind = ref('')
const search = ref('')
const tabs = [{ id: 'all', label: 'Alle Jobs' }, { id: 'active', label: 'Aktiv & wartend' }, { id: 'finished', label: 'Abgeschlossen' }, { id: 'failed', label: 'Fehler' }]
const activeCount = computed(() => data.value?.filter(job => job.active).length || 0)
const selectedId = computed(() => typeof route.query.job === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(route.query.job) ? route.query.job.toLowerCase() : '')
const selectedJob = computed(() => data.value?.find(job => job.id === selectedId.value))
const filtered = computed(() => (data.value || []).filter(job =>
  (filter.value === 'all' || (filter.value === 'active' ? job.active : filter.value === 'failed' ? job.status === 'failed' : !job.active))
  && (!kind.value || job.kind === kind.value)
  && (!search.value.trim() || `${job.label} ${job.id} ${job.message || ''}`.toLocaleLowerCase('de').includes(search.value.trim().toLocaleLowerCase('de'))),
))
</script>
<template>
  <div>
    <div class="mb-7 flex flex-wrap items-start justify-between gap-4"><div><h1 class="page-title">Jobs</h1><p class="mt-2 text-sm text-studio-400">{{ activeCount }} aktiv · Aufnahmen, KI-Analysen und Exporte mit ihren Protokollen.</p></div><button class="btn btn-secondary" :disabled="status === 'pending'" @click="refresh"><RefreshCw :size="15" />Aktualisieren</button></div>
    <JobLogPanel v-if="selectedId" :key="selectedId" :job-id="selectedId" :job="selectedJob" :now="now" />
    <p v-else-if="route.query.job" role="alert" class="panel mb-6 p-5 text-sm text-red-300">Ungültiger Job-Link. Wähle unten einen Job aus.</p>
    <section class="panel overflow-hidden" aria-label="Jobliste">
      <nav class="tab-bar overflow-x-auto px-5" aria-label="Job-Ansichten"><button v-for="tab in tabs" :key="tab.id" class="tab shrink-0" :class="{ 'tab-active': filter === tab.id }" :aria-pressed="filter === tab.id" @click="filter = tab.id">{{ tab.label }}<span v-if="tab.id === 'active'" class="rounded bg-accent/10 px-1.5 py-0.5 text-[10px] text-accent">{{ activeCount }}</span></button></nav>
      <div class="flex flex-wrap items-center gap-3 border-b divider p-5"><div class="relative min-w-0 flex-1 basis-52"><Search :size="16" class="pointer-events-none absolute left-3 top-3 text-studio-400" /><input v-model="search" class="input pl-9" aria-label="Jobs durchsuchen" placeholder="Job, Streamer oder ID suchen …" /></div><select v-model="kind" class="input w-auto" aria-label="Nach Job-Typ filtern"><option value="">Alle Job-Typen</option><option v-for="(label, value) in jobKindLabels" :key="value" :value="value">{{ label }}</option></select></div>
      <ConnectionError v-if="error" @retry="refresh" />
      <p v-else-if="!data && status === 'pending'" class="p-8 text-sm text-studio-400">Jobs werden geladen …</p>
      <div v-else-if="!filtered.length" class="flex flex-col items-center gap-3 px-5 py-14 text-center"><ListTodo :size="28" class="text-studio-400" /><p class="text-sm text-studio-400">{{ data?.length ? 'Keine Jobs für diese Filter.' : 'Zurzeit keine aktiven oder kürzlich abgeschlossenen Jobs.' }}</p></div>
      <div v-else class="overflow-x-auto">
        <table class="w-full text-left text-xs"><thead class="border-b divider bg-studio-950/40 text-[10px] uppercase tracking-wide text-studio-400"><tr><th class="px-5 py-3 font-medium">Job</th><th class="px-4 py-3 font-medium">Status</th><th class="hidden px-4 py-3 font-medium md:table-cell">Erstellt</th><th class="px-4 py-3 font-medium">Laufzeit</th><th class="px-5 py-3"><span class="sr-only">Protokoll</span></th></tr></thead>
          <tbody><tr v-for="job in filtered" :key="job.id" class="border-b divider last:border-0 hover:bg-studio-850" :class="{ 'bg-accent/5': selectedId === job.id }">
            <td class="min-w-40 max-w-sm px-5 py-4"><NuxtLink :to="jobLink(job.id)" class="font-medium hover:text-accent">{{ job.label }}</NuxtLink><p class="mt-1 text-[10px] text-studio-400">{{ jobKindLabels[job.kind] }}</p><p v-if="job.message" class="mt-2 line-clamp-2 break-words text-[11px] text-studio-400" :title="job.message">{{ job.message }}</p></td>
            <td class="px-4 py-4" :class="job.status === 'failed' ? 'text-red-300' : job.active ? 'text-accent' : 'text-studio-400'">{{ jobStatusLabels[job.status] || job.status }}</td><td class="hidden whitespace-nowrap px-4 py-4 text-studio-400 md:table-cell">{{ dateTime(job.created_at) }}</td><td class="px-4 py-4 font-mono text-studio-400">{{ jobDuration(job, now) }}</td><td class="px-5 py-4"><NuxtLink :to="jobLink(job.id)" class="inline-flex items-center gap-1.5 whitespace-nowrap text-accent hover:underline" :aria-label="`Logs für ${job.label} öffnen`"><FileText :size="14" />Logs</NuxtLink></td>
          </tr></tbody>
        </table>
      </div>
      <p class="border-t divider px-5 py-3 text-[11px] text-studio-400">{{ filtered.length }} von {{ data?.length || 0 }} Jobs · Aktive Jobs und Abschlüsse der letzten Stunde, maximal 100 · Live-Aktualisierung</p>
    </section>
  </div>
</template>
