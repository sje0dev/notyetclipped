<script setup lang="ts">
import { Download, ThumbsUp, ThumbsDown, Bookmark, Trash2, RefreshCw, Save, LoaderCircle, AudioLines } from '@lucide/vue'
import type { Clip } from '~/types'
const props = defineProps<{ clipId: string | null }>()
const emit = defineEmits<{ close: []; changed: [] }>()
const { toast } = useToast()
const clip = ref<Clip | null>(null)
const title = ref('')
const error = ref('')
const loading = ref(false)
const busy = ref(false)
const confirmDelete = ref(false)
const clipEvent = useState<{ id: string | null; at: number }>('clip-event', () => ({ id: null, at: 0 }))
watch(clipEvent, (event) => {
  if (event.id === props.clipId && clip.value?.status !== 'ready' && !busy.value && !loading.value) void load()
})
let request = 0
async function load() {
  if (!props.clipId) return
  const current = ++request
  loading.value = true
  error.value = ''
  try {
    const value = await api<Clip>(`/clips/${props.clipId}`)
    if (current === request) { clip.value = value; title.value = value.title }
  } catch (e) { if (current === request) error.value = errorMessage(e) }
  finally { if (current === request) loading.value = false }
}
watch(() => props.clipId, () => { clip.value = null; confirmDelete.value = false; request++; void load() })
async function update(body: object) {
  if (!clip.value) return
  busy.value = true
  try { clip.value = await api<Clip>(`/clips/${clip.value.id}`, { method: 'PATCH', body }); emit('changed') }
  catch (e) { toast(errorMessage(e), true) }
  finally { busy.value = false }
}
async function remove() {
  if (!clip.value) return
  busy.value = true
  try { await api(`/clips/${clip.value.id}`, { method: 'DELETE' }); toast('Clip wird aus S3 und der Bibliothek gelöscht.'); emit('changed'); emit('close') }
  catch (e) { toast(errorMessage(e), true) }
  finally { busy.value = false }
}
async function retry() {
  busy.value = true
  try { await api(`/clips/${props.clipId}/retry`, { method: 'POST' }); toast('Clip erneut eingeplant.'); emit('changed'); await load() }
  catch (e) { toast(errorMessage(e), true) }
  finally { busy.value = false }
}
</script>
<template>
  <AppModal :open="!!clipId" title="Dein Moment" wide @close="!busy && emit('close')">
    <div v-if="loading" class="flex justify-center py-20"><LoaderCircle class="animate-spin text-lime-300" :size="26" /></div>
    <p v-else-if="error" role="alert" class="text-sm text-red-300">{{ error }} <button class="underline" @click="load">Erneut laden</button></p>
    <div v-else-if="clip" class="space-y-5">
      <video v-if="clip.video_url" :key="clip.video_url" :src="clip.video_url" :poster="clip.thumbnail_url || undefined" controls playsinline preload="metadata" class="aspect-video w-full rounded-xl bg-black" @error="error = 'Video konnte nicht geladen werden. Bitte Link erneuern.'" />
      <div v-else class="flex aspect-video flex-col items-center justify-center gap-4 rounded-xl bg-studio-950"><AudioLines :size="36" class="text-studio-700" /><StatusBadge :status="clip.status" /><p class="max-w-md px-6 text-center text-xs text-studio-400">{{ clip.error || 'Dein Clip wird verarbeitet. Sobald der Upload abgeschlossen ist, ist die Vorschau verfügbar.' }}</p><button class="btn btn-secondary text-xs" @click="load"><RefreshCw :size="14" />Status aktualisieren</button></div>
      <div class="flex gap-2"><input v-model="title" class="input flex-1" aria-label="Clip-Titel" maxlength="200" /><button class="btn btn-secondary" :disabled="busy || !title.trim() || title === clip.title" aria-label="Titel speichern" @click="update({ title })"><Save :size="16" /></button></div>
      <div class="flex flex-wrap items-center gap-3 text-xs text-studio-400"><span class="font-semibold text-lime-300">@{{ clip.streamer_login }}</span><span>{{ dateTime(clip.created_at) }}</span><span>{{ duration(clip.duration) }}</span><span>{{ bytes(clip.size_bytes) }}</span></div>
      <p class="rounded-lg border divider bg-studio-950 p-3 text-xs leading-relaxed text-studio-400">{{ clip.reason }}<span v-if="clip.trigger === 'audio'" class="mt-1 block text-[10px]">Score = Stärke des Audiosignals. Deine Bewertung wird gespeichert.</span></p>
      <div class="flex flex-wrap items-center gap-2 border-t divider pt-5"><button class="btn btn-secondary px-3" :class="{ 'text-lime-300': clip.rating === 1 }" :disabled="busy" aria-label="Guter Treffer" :aria-pressed="clip.rating === 1" @click="update({ rating: clip.rating === 1 ? 0 : 1 })"><ThumbsUp :size="16" /></button><button class="btn btn-secondary px-3" :class="{ 'text-red-300': clip.rating === -1 }" :disabled="busy" aria-label="Uninteressanter Treffer" :aria-pressed="clip.rating === -1" @click="update({ rating: clip.rating === -1 ? 0 : -1 })"><ThumbsDown :size="16" /></button><button class="btn btn-secondary px-3" :class="{ 'text-lime-300': clip.favorite }" :disabled="busy" aria-label="Favorit umschalten" :aria-pressed="clip.favorite" @click="update({ favorite: !clip.favorite })"><Bookmark :size="16" :fill="clip.favorite ? 'currentColor' : 'none'" /></button><button class="btn btn-ghost px-3" aria-label="Clip löschen" :disabled="busy" @click="confirmDelete = !confirmDelete"><Trash2 :size="16" /></button><button v-if="clip.status === 'failed'" class="btn btn-secondary ml-auto" :disabled="busy" @click="retry"><RefreshCw :size="15" />Erneut versuchen</button><a v-if="clip.download_url" :href="clip.download_url" class="btn btn-primary ml-auto"><Download :size="16" />MP4 herunterladen</a></div>
      <div v-if="confirmDelete" class="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-red-400/20 bg-red-400/5 p-4"><p class="text-xs text-red-200">Diesen Clip samt S3-Dateien endgültig löschen?</p><button class="btn btn-danger text-xs" :disabled="busy" @click="remove">Ja, löschen</button></div>
    </div>
  </AppModal>
</template>
