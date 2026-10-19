<script setup lang="ts">
import { Film, Download, Play, RefreshCw } from '@lucide/vue'
import type { EditJob } from '~/types'
const { data, error, refresh: refreshData } = await useFetch<EditJob[]>('/api/edits', { key: 'edits', server: false })
const refresh = () => refreshData()
usePolling(refresh, 30000, false)
const playing = ref<EditJob | null>(null)
const { toast } = useToast()
const busy = ref('')
const labels: Record<string, string> = { queued: 'Wartet auf freien Exportplatz', rendering: 'Hochkant wird gerendert', ready: 'Fertig', failed: 'Fehlgeschlagen' }
async function retry(id: string) { busy.value = id; try { await api(`/edits/${id}/retry`, { method: 'POST' }); await refresh() } catch (e) { toast(errorMessage(e), true) } finally { busy.value = '' } }
</script>
<template>
  <div>
    <div class="mb-7 flex items-start justify-between gap-4"><div><h1 class="page-title">Schnittliste</h1><p class="mt-2 text-sm text-studio-400">Separate Hochkant-Versionen deiner Clips. Die letzten 100 Aufträge.</p></div><button class="icon-btn" aria-label="Schnittliste aktualisieren" @click="refresh"><RefreshCw :size="17" /></button></div>
    <ConnectionError v-if="error" @retry="refresh" />
    <div v-else-if="!data?.length" class="panel flex flex-col items-center gap-4 p-12 text-center"><Film :size="30" class="text-studio-400" /><h2 class="text-sm font-semibold">Clips zur Bearbeitung schicken</h2><p class="max-w-md text-sm text-studio-400">Öffne einen fertigen Clip und wähle „Zur Bearbeitung“. Facecam und Chat legst du im Hochkant-Profil des jeweiligen Streamers fest.</p><NuxtLink to="/clips" class="btn btn-primary">Zur Clip-Bibliothek</NuxtLink></div>
    <div v-else class="space-y-3"><article v-for="edit in data" :key="edit.id" class="panel flex flex-wrap items-center gap-4 p-5"><Film :size="20" class="text-lavender" /><div class="min-w-0 flex-1 basis-48"><h2 class="truncate text-sm font-medium">{{ edit.title }}</h2><p class="mt-1 text-xs text-studio-400">@{{ edit.streamer_login }} · {{ dateTime(edit.created_at) }}</p><p class="mt-2 text-xs" :class="edit.status === 'failed' ? 'text-red-300' : edit.status === 'ready' ? 'text-mint' : 'text-accent'">{{ labels[edit.status] || edit.status }}</p><p v-if="edit.error" class="mt-2 break-words text-xs text-red-300">{{ edit.error }}</p><details class="mt-2 text-[11px] text-studio-400"><summary class="cursor-pointer">Verwendetes Layout</summary><p class="mt-2">720×1280 · {{ edit.profile.framing === 'fit' ? 'Vollbild einpassen' : 'Fläche füllen' }} · Facecam {{ edit.profile.facecam_enabled ? `${edit.profile.facecam_position === 'top' ? 'oben' : 'unten'} ${edit.profile.facecam_percent}%` : 'aus' }} · Chat {{ ({ keep: 'im Bild', hide: 'geschwärzt', panel: 'separat' })[edit.profile.chat_mode] }}</p></details></div><div class="flex flex-wrap gap-2"><NuxtLink :to="`/streamers/${edit.streamer_id}?tab=layout`" class="btn btn-ghost text-xs">Streamer-Layout</NuxtLink><button v-if="edit.status === 'failed'" class="btn btn-secondary text-xs" :disabled="!!busy" @click="retry(edit.id)"><RefreshCw :size="14" />Wiederholen</button><button v-if="edit.video_url" class="btn btn-secondary text-xs" @click="playing = edit"><Play :size="14" />Vorschau</button><a v-if="edit.download_url" :href="edit.download_url" class="btn btn-primary text-xs"><Download :size="14" />Hochkant-MP4</a></div></article></div>
    <AppModal :open="!!playing" title="Hochkant-Vorschau" @close="playing = null"><video v-if="playing?.video_url" :src="playing.video_url" controls playsinline class="mx-auto max-h-[70dvh] rounded-lg bg-black" /></AppModal>
  </div>
</template>
