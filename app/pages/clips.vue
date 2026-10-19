<script setup lang="ts">
import { Search, SlidersHorizontal, Clapperboard, Bookmark, Trash2, ChevronLeft, ChevronRight, CheckCheck } from '@lucide/vue'
import type { Clip, Streamer } from '~/types'
const search = ref('')
const debounced = ref('')
const filter = ref('all')
const channel = ref('')
const sort = ref('newest')
const favorites = ref(false)
const page = ref(0)
const selected = ref<string[]>([])
const selectedClip = ref<string | null>(null)
const confirmDelete = ref(false)
const deleting = ref(false)
const { toast } = useToast()
let debounce: ReturnType<typeof setTimeout>
watch(search, value => { clearTimeout(debounce); debounce = setTimeout(() => { debounced.value = value }, 300) })
onBeforeUnmount(() => clearTimeout(debounce))
watch([debounced, filter, channel, sort, favorites], () => { page.value = 0; selected.value = [] })
const query = computed(() => ({ search: debounced.value, status: filter.value, streamer_id: channel.value || undefined, sort: sort.value, favorite: favorites.value, offset: page.value * 24, limit: 24 }))
const { data, error, refresh } = await useFetch<{ items: Clip[]; total: number }>('/api/clips', { key: 'clips', query, timeout: 8000 })
const { data: streamers } = await useFetch<Streamer[]>('/api/streamers', { key: 'streamers', timeout: 8000 })
usePolling(refresh)
watch(data, value => { selected.value = selected.value.filter(id => value?.items.some(clip => clip.id === id)) })
function select(id: string) { selected.value = selected.value.includes(id) ? selected.value.filter(value => value !== id) : [...selected.value, id] }
function selectAll() { selected.value = selected.value.length === data.value?.items.length ? [] : (data.value?.items.map(clip => clip.id) || []) }
async function removeSelected() {
  deleting.value = true
  const results = await Promise.allSettled(selected.value.map(id => api(`/clips/${id}`, { method: 'DELETE' })))
  const failures = results.filter(result => result.status === 'rejected').length
  toast(failures ? `${failures} Clips konnten nicht gelöscht werden.` : 'Ausgewählte Clips werden gelöscht.', !!failures)
  selected.value = []
  confirmDelete.value = false
  deleting.value = false
  await refresh()
}
</script>
<template>
  <div>
    <div class="mb-8 flex items-end justify-between gap-4"><div><p class="eyebrow mb-2">The Good Stuff</p><h1 class="page-title">Deine Clip-Bibliothek<span class="text-lime-300">.</span></h1><p class="mt-2 text-sm text-studio-400">Gesammelte Highlights. Bereit für deinen nächsten Edit.</p></div><span class="hidden rounded-lg border divider bg-studio-900 px-3 py-2 text-xs text-studio-400 sm:block">{{ data?.total || 0 }} Clips</span></div>
    <div class="panel mb-6 flex flex-wrap items-center gap-3 p-3"><div class="relative min-w-48 flex-1"><Search :size="16" class="pointer-events-none absolute left-3 top-3 text-studio-400" /><input v-model="search" class="input pl-9" placeholder="Titel oder Streamer suchen …" aria-label="Clips durchsuchen" /></div><select v-model="channel" class="input w-auto min-w-36" aria-label="Nach Streamer filtern"><option value="">Alle Streamer</option><option v-for="streamer in streamers" :key="streamer.id" :value="streamer.id">{{ streamer.login }}</option></select><select v-model="filter" class="input w-auto" aria-label="Nach Status filtern"><option value="all">Jeder Status</option><option value="ready">Bereit</option><option value="collecting">Nachlauf</option><option value="queued">Warteschlange</option><option value="cutting">Schnitt</option><option value="uploading">Upload</option><option value="retry">Neuer Versuch</option><option value="failed">Fehler</option></select><button class="btn btn-secondary px-3" :class="{ 'border-lime-300/40 text-lime-300': favorites }" :aria-pressed="favorites" aria-label="Nur Favoriten anzeigen" @click="favorites = !favorites"><Bookmark :size="17" :fill="favorites ? 'currentColor' : 'none'" /></button></div>
    <div class="mb-5 flex flex-wrap items-center justify-between gap-3"><div class="flex items-center gap-3"><button class="flex items-center gap-1.5 text-xs text-studio-400 hover:text-zinc-100" :disabled="!data?.items.length" @click="selectAll"><CheckCheck :size="14" />{{ selected.length ? `${selected.length} ausgewählt` : 'Seite auswählen' }}</button><button v-if="selected.length" class="flex items-center gap-1 text-xs text-red-300" @click="confirmDelete = true"><Trash2 :size="13" />Löschen</button></div><div class="flex items-center gap-2 text-xs text-studio-400"><SlidersHorizontal :size="13" /><select v-model="sort" aria-label="Sortierung" class="rounded-md border-0 bg-transparent py-1 pl-0 pr-7 text-xs text-studio-400 focus:ring-lime-300"><option value="newest">Neueste zuerst</option><option value="score">Höchster Audio-Score</option></select></div></div>
    <ConnectionError v-if="error" @retry="refresh" />
    <div v-else-if="data?.items.length" class="grid gap-5 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4"><ClipCard v-for="clip in data.items" :key="clip.id" :clip="clip" selectable :selected="selected.includes(clip.id)" @select="select" @open="selectedClip = $event" @changed="refresh" /></div>
    <div v-else class="panel flex flex-col items-center justify-center px-6 py-24 text-center"><Clapperboard :size="40" class="mb-5 text-studio-700" /><h2 class="text-lg font-semibold">Noch keine Clips in dieser Ansicht.</h2><p class="mt-2 max-w-sm text-sm leading-relaxed text-studio-400">{{ debounced || filter !== 'all' || channel || favorites ? 'Passe deine Filter an, um weitere Momente zu finden.' : 'Füge einen Streamer hinzu. Automatisch erkannte und manuell gesicherte Momente erscheinen hier.' }}</p><NuxtLink v-if="!debounced && filter === 'all' && !channel && !favorites" to="/streamers" class="btn btn-secondary mt-6">Zur Watchlist</NuxtLink></div>
    <div v-if="data && data.total > 24" class="mt-7 flex items-center justify-between text-xs text-studio-400"><span>Seite {{ page + 1 }} von {{ Math.ceil(data.total / 24) }}</span><div class="flex gap-2"><button class="btn btn-secondary" :disabled="page === 0" aria-label="Vorherige Seite" @click="page--; selected = []"><ChevronLeft :size="16" /></button><button class="btn btn-secondary" :disabled="(page + 1) * 24 >= data.total" aria-label="Nächste Seite" @click="page++; selected = []"><ChevronRight :size="16" /></button></div></div>
    <ClipPlayer :clip-id="selectedClip" @close="selectedClip = null" @changed="refresh" />
    <AppModal :open="confirmDelete" title="Clips löschen" @close="!deleting && (confirmDelete = false)"><p class="text-sm text-studio-400">{{ selected.length }} Clips samt Video- und Vorschaudateien endgültig aus S3 löschen?</p><div class="mt-6 flex justify-end gap-2"><button class="btn btn-ghost" :disabled="deleting" @click="confirmDelete = false">Abbrechen</button><button class="btn btn-danger" :disabled="deleting" @click="removeSelected">{{ deleting ? 'Wird eingeplant …' : 'Endgültig löschen' }}</button></div></AppModal>
  </div>
</template>
