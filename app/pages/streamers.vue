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
    <div class="mb-8 flex flex-wrap items-center justify-between gap-4"><div><p class="eyebrow mb-2">Capture Control</p><h1 class="page-title">Deine Watchlist<span class="text-lime-300">.</span></h1><p class="mt-2 text-sm text-studio-400">Alle Kanäle an einem Ort. Bereit für den nächsten großen Moment.</p></div><button class="btn btn-primary" @click="add"><Plus :size="17" />Streamer hinzufügen</button></div>
    <div class="mb-6 flex flex-wrap justify-between gap-4"><div class="flex gap-1 rounded-lg border divider bg-studio-900 p-1"><button v-for="tab in [{ id: 'all', label: 'Alle Streamer' }, { id: 'live', label: 'Live' }, { id: 'paused', label: 'Pausiert' }]" :key="tab.id" class="rounded-md px-3 py-2 text-xs transition" :class="filter === tab.id ? 'bg-studio-800 font-medium text-zinc-100' : 'text-studio-400 hover:text-zinc-200'" @click="filter = tab.id">{{ tab.label }}</button></div><div class="relative w-full sm:w-64"><Search :size="15" class="pointer-events-none absolute left-3 top-3 text-studio-400" /><input v-model="search" class="input pl-9" placeholder="Streamer suchen …" aria-label="Streamer suchen" /></div></div>
    <ConnectionError v-if="error" @retry="refresh" />
    <div v-else-if="filtered.length" class="grid gap-5 md:grid-cols-2 xl:grid-cols-3"><div v-for="streamer in filtered" :key="streamer.id"><StreamerCard :streamer="streamer" @edit="edit" @changed="refresh" /><div class="mt-2 flex items-center justify-between px-1 text-[10px] text-studio-400"><span>Priorität {{ streamer.priority }} <span class="mx-1.5">·</span> {{ streamer.pre_roll }}s davor / {{ streamer.post_roll }}s danach</span><button class="icon-btn size-6" :aria-label="`${streamer.login} entfernen`" @click="removing = streamer"><Trash2 :size="12" /></button></div></div><button class="panel flex min-h-[290px] flex-col items-center justify-center gap-3 border-dashed bg-transparent text-studio-400 transition hover:border-lime-300/40 hover:bg-lime-300/3 hover:text-lime-300" @click="add"><Plus :size="26" /><span class="text-sm">Weiteren Streamer hinzufügen</span></button></div>
    <div v-else class="panel flex flex-col items-center justify-center px-6 py-20 text-center"><Radio :size="36" class="mb-5 text-studio-700" /><h2 class="text-lg font-semibold">{{ search || filter !== 'all' ? 'Keine passenden Streamer.' : 'Wen behalten wir im Auge?' }}</h2><p class="mt-2 max-w-sm text-sm leading-relaxed text-studio-400">{{ search || filter !== 'all' ? 'Versuche einen anderen Suchbegriff oder Filter.' : 'Füge Twitch-Kanäle hinzu. Der Worker prüft regelmäßig, wer live ist, und startet den Puffer automatisch.' }}</p><button v-if="!search && filter === 'all'" class="btn btn-primary mt-6" @click="add">Ersten Streamer hinzufügen<ArrowUpRight :size="16" /></button></div>
    <StreamerForm :open="formOpen" :streamer="editing" @close="formOpen = false" @saved="refresh" />
    <AppModal :open="!!removing" title="Streamer entfernen" @close="!busy && (removing = null)"><p class="text-sm leading-relaxed text-studio-400">Die Überwachung von <strong class="text-zinc-200">@{{ removing?.login }}</strong> wird beendet. Bereits gespeicherte Clips bleiben in deiner Bibliothek.</p><div class="mt-6 flex justify-end gap-2"><button class="btn btn-ghost" :disabled="busy" @click="removing = null">Abbrechen</button><button class="btn btn-danger" :disabled="busy" @click="remove"><Trash2 :size="15" />Entfernen</button></div></AppModal>
  </div>
</template>
