<script setup lang="ts">
import { Plus, Search, Radio, Trash2, ArrowUpRight } from '@lucide/vue'
import type { Streamer } from '~/types'
const { data, error, refresh } = await useFetch<Streamer[]>('/api/streamers', { key: 'streamers', timeout: 8000 })
usePolling(refresh)
const { toast } = useToast()
const search = ref('')
const filter = ref('all')
const formOpen = ref(false)
const editing = ref<Streamer | null>(null)
const removing = ref<Streamer | null>(null)
const busy = ref(false)
const filtered = computed(() => (data.value || []).filter(s => s.login.includes(search.value.toLowerCase()) && (filter.value === 'all' || (filter.value === 'live' ? s.status === 'recording' : !s.enabled))))
function add() { editing.value = null; formOpen.value = true }
function edit(streamer: Streamer) { editing.value = streamer; formOpen.value = true }
async function remove() {
  if (!removing.value) return
  busy.value = true
  try { await api(`/streamers/${removing.value.id}`, { method: 'DELETE' }); removing.value = null; await refresh(); toast('Streamer aus der Watchlist entfernt.') }
  catch (e) { toast(errorMessage(e), true) }
  finally { busy.value = false }
}
</script>
<template>
  <div>
    <div class="mb-7 flex flex-wrap items-center justify-between gap-4"><div><h1 class="page-title">Streamer</h1><p class="mt-2 text-sm text-studio-400">Aufnahmen verwalten und passende Trigger für jeden Kanal festlegen.</p></div><button class="btn btn-primary" @click="add"><Plus :size="16" />Streamer hinzufügen</button></div>
    <div class="mb-7 flex flex-wrap items-center justify-between gap-4 border-b divider"><nav class="flex gap-6" aria-label="Streamer-Ansichten"><button v-for="tab in [{ id: 'all', label: 'Alle Streamer' }, { id: 'live', label: 'Live' }, { id: 'paused', label: 'Pausiert' }]" :key="tab.id" class="tab" :class="{ 'tab-active': filter === tab.id }" :aria-pressed="filter === tab.id" @click="filter = tab.id">{{ tab.label }}<span class="rounded-full bg-studio-800 px-2 py-0.5 text-[11px]">{{ (data || []).filter(s => tab.id === 'all' || (tab.id === 'live' ? s.status === 'recording' : !s.enabled)).length }}</span></button></nav><div class="relative mb-3 w-full sm:w-64"><Search :size="15" class="pointer-events-none absolute left-3 top-3 text-studio-400" /><input v-model="search" class="input pl-9" placeholder="Streamer suchen …" aria-label="Streamer suchen" /></div></div>
    <ConnectionError v-if="error" @retry="refresh" />
    <div v-else-if="filtered.length" class="grid gap-5 md:grid-cols-2 xl:grid-cols-3"><div v-for="streamer in filtered" :key="streamer.id"><StreamerCard :streamer="streamer" @edit="edit" @changed="refresh" /><div class="mt-2 flex items-center justify-between px-1 text-[10px] text-studio-400"><span>Priorität {{ streamer.priority }} <span class="mx-1.5">·</span> {{ streamer.pre_roll }}s davor / {{ streamer.post_roll }}s danach</span><button class="icon-btn size-6" :aria-label="`${streamer.login} entfernen`" @click="removing = streamer"><Trash2 :size="12" /></button></div></div><button class="panel flex min-h-[290px] flex-col items-center justify-center gap-3 border-dashed bg-transparent text-studio-400 transition hover:border-lime-300/40 hover:bg-lime-300/3 hover:text-lime-300" @click="add"><Plus :size="26" /><span class="text-sm">Weiteren Streamer hinzufügen</span></button></div>
    <div v-else class="panel flex flex-col items-center justify-center px-6 py-20 text-center"><Radio :size="36" class="mb-5 text-studio-700" /><h2 class="text-lg font-semibold">{{ search || filter !== 'all' ? 'Keine passenden Streamer.' : 'Wen behalten wir im Auge?' }}</h2><p class="mt-2 max-w-sm text-sm leading-relaxed text-studio-400">{{ search || filter !== 'all' ? 'Versuche einen anderen Suchbegriff oder Filter.' : 'Füge Twitch-Kanäle hinzu. Der Worker prüft regelmäßig, wer live ist, und startet den Puffer automatisch.' }}</p><button v-if="!search && filter === 'all'" class="btn btn-primary mt-6" @click="add">Ersten Streamer hinzufügen<ArrowUpRight :size="16" /></button></div>
    <StreamerForm :open="formOpen" :streamer="editing" @close="formOpen = false" @saved="refresh" />
    <AppModal :open="!!removing" title="Streamer entfernen" @close="!busy && (removing = null)"><p class="text-sm leading-relaxed text-studio-400">Die Überwachung von <strong class="text-zinc-200">@{{ removing?.login }}</strong> wird beendet. Bereits gespeicherte Clips bleiben in deiner Bibliothek.</p><div class="mt-6 flex justify-end gap-2"><button class="btn btn-ghost" :disabled="busy" @click="removing = null">Abbrechen</button><button class="btn btn-danger" :disabled="busy" @click="remove"><Trash2 :size="15" />Entfernen</button></div></AppModal>
  </div>
</template>
