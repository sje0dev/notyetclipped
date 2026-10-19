<script setup lang="ts">
import { Download, RefreshCw, X, FileText } from '@lucide/vue'
import type { Job, JobLog } from '~/types'
const props = defineProps<{ jobId: string; job?: Job; now: number }>()
const { data, error, status, refresh: refreshData } = await useFetch<JobLog[]>(`/api/jobs/${props.jobId}/logs`, { key: `job-logs-${props.jobId}`, server: false, timeout: 8000 })
const refresh = () => refreshData()
usePolling(refresh, 5000, false)
function logTime(value: string) { return new Intl.DateTimeFormat('de-DE', { timeZone: 'Europe/Berlin', day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', second: '2-digit' }).format(new Date(value)) }
</script>
<template>
  <section id="job-logs" class="panel mb-6 scroll-mt-24 overflow-hidden" aria-labelledby="job-log-title">
    <div class="flex flex-wrap items-start justify-between gap-4 border-b divider p-5">
      <div class="min-w-0 flex-1"><h2 id="job-log-title" class="flex items-start gap-2 text-sm font-semibold"><FileText :size="17" class="mt-0.5 shrink-0 text-accent" /><span class="break-words">{{ job?.label || 'Job-Protokoll' }}</span></h2><p class="mt-2 break-all font-mono text-[10px] text-studio-400">{{ jobId }}</p><p v-if="job" class="mt-2 text-xs text-studio-400">{{ jobStatusLabels[job.status] || job.status }} · {{ jobDuration(job, now) }} · {{ dateTime(job.created_at) }}</p></div>
      <div class="flex items-center gap-2"><a :href="`/api/jobs/${jobId}/logs?download=true`" class="btn btn-secondary text-xs"><Download :size="14" />Log herunterladen</a><button class="icon-btn" :disabled="status === 'pending'" aria-label="Job-Log aktualisieren" @click="refresh"><RefreshCw :size="15" /></button><NuxtLink to="/jobs" class="icon-btn" aria-label="Job-Protokoll schließen"><X :size="16" /></NuxtLink></div>
    </div>
    <div class="p-5">
      <p v-if="job?.message" class="mb-4 break-words text-xs" :class="job.status === 'failed' ? 'text-red-300' : 'text-studio-400'">{{ job.message }}</p>
      <p class="mb-4 text-[11px] text-studio-400">Letzte {{ data?.length || 0 }} von maximal 500 Einträgen · Aktualisierung alle 5 Sekunden · Aufbewahrung 7 Tage</p>
      <p v-if="error" role="alert" class="text-xs text-red-300">Das Protokoll konnte nicht geladen werden. <button class="underline" @click="refresh">Erneut laden</button></p>
      <p v-else-if="!data && status === 'pending'" class="hint">Protokoll wird geladen …</p>
      <p v-else-if="!data?.length" class="hint">Keine Protokolleinträge vorhanden. Wartende Jobs protokollieren ihre Verarbeitung nach dem Start. Ältere Einträge werden nach sieben Tagen entfernt.</p>
      <ol v-if="data?.length" class="max-h-[60dvh] space-y-2 overflow-y-auto rounded-lg bg-studio-950 p-3 font-mono text-[11px]" aria-label="Protokolleinträge">
        <li v-for="line in data" :key="line.id" class="break-words border-b divider py-2 last:border-0" :class="line.level === 'error' ? 'text-red-300' : line.level === 'warn' ? 'text-amber-200' : 'text-studio-300'"><span class="mb-1 block text-[10px] text-studio-400"><time :datetime="line.created_at">{{ logTime(line.created_at) }}</time> · {{ line.level }}</span><span class="whitespace-pre-wrap">{{ line.message }}</span></li>
      </ol>
    </div>
  </section>
</template>
