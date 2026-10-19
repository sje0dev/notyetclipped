<script setup lang="ts">
import { Save, RotateCcw, Film } from '@lucide/vue'
import type { Streamer, EditingSettings, CropRect } from '~/types'
const props = defineProps<{ streamer: Streamer }>()
const emit = defineEmits<{ dirty: [value: boolean] }>()
const { toast } = useToast()
const { data, error, refresh } = await useFetch<{ profile: EditingSettings; preview_url: string | null }>(`/api/streamers/${props.streamer.id}/editing`, { key: `editing-${props.streamer.id}`, server: false })
const draft = ref<EditingSettings>()
const saved = ref('')
const dirty = computed(() => !!draft.value && JSON.stringify(draft.value) !== saved.value)
watch(data, value => { if (value && !dirty.value) { draft.value = structuredClone(toRaw(value.profile)); saved.value = JSON.stringify(draft.value) } }, { immediate: true })
watch(dirty, value => emit('dirty', value), { immediate: true })
const busy = ref(false)
const failure = ref('')
const image = ref<HTMLImageElement>()
const canvas = ref<HTMLCanvasElement>()
const regions = [{ key: 'gameplay', label: 'Spiel / Hauptbild', color: '#6b9fff' }, { key: 'facecam', label: 'Facecam im Quellvideo', color: '#b8a1ff' }, { key: 'chat', label: 'Chat im Quellvideo', color: '#38d6bd' }] as const
const fields = [{ key: 'x', label: 'Links' }, { key: 'y', label: 'Oben' }, { key: 'width', label: 'Breite' }, { key: 'height', label: 'Höhe' }] as const
function setRect(key: 'gameplay' | 'facecam' | 'chat', field: keyof CropRect, event: Event) {
  if (!draft.value) return
  const r = draft.value[key], v = Number((event.target as HTMLInputElement).value) / 100
  if (!Number.isFinite(v)) return
  const max = field === 'x' ? 1-r.width : field === 'y' ? 1-r.height : field === 'width' ? 1-r.x : 1-r.y
  r[field] = Math.max(field === 'width' || field === 'height' ? .02 : 0, Math.min(max, v))
}
function draw() {
  const p = draft.value, img = image.value, ctx = canvas.value?.getContext('2d')
  if (!p || !img?.naturalWidth || !ctx) return
  ctx.fillStyle = '#000'; ctx.fillRect(0, 0, 720, 1280)
  const masked = document.createElement('canvas'); masked.width = img.naturalWidth; masked.height = img.naturalHeight
  const m = masked.getContext('2d')!; m.drawImage(img, 0, 0)
  if (p.chat_mode !== 'keep') { m.fillStyle = '#000'; m.fillRect(p.chat.x*masked.width, p.chat.y*masked.height, p.chat.width*masked.width, p.chat.height*masked.height) }
  const drawRegion = (source: CanvasImageSource, r: CropRect, y: number, h: number, fill: boolean) => {
    const sw = r.width*img.naturalWidth, sh = r.height*img.naturalHeight
    const scale = fill ? Math.max(720/sw,h/sh) : Math.min(720/sw,h/sh)
    const w = sw*scale, height = sh*scale
    ctx.save(); ctx.beginPath(); ctx.rect(0,y,720,h); ctx.clip()
    ctx.drawImage(source,r.x*img.naturalWidth,r.y*img.naturalHeight,sw,sh,(720-w)/2,y+(h-height)/2,w,height); ctx.restore()
  }
  const camH = p.facecam_enabled ? Math.floor(1280*p.facecam_percent/100/2)*2 : 0
  const chatH = p.chat_mode === 'panel' ? 256 : 0, gameH = 1280-camH-chatH
  let y = 0
  if (camH && p.facecam_position === 'top') { drawRegion(img,p.facecam,y,camH,false); y += camH }
  drawRegion(masked,p.gameplay,y,gameH,p.framing === 'fill'); y += gameH
  if (camH && p.facecam_position === 'bottom') { drawRegion(img,p.facecam,y,camH,false); y += camH }
  if (chatH) drawRegion(img,p.chat,y,chatH,false)
}
watch(draft, () => nextTick(draw), { deep: true })
async function save() {
  busy.value = true; failure.value = ''
  try { const value = await api<EditingSettings>(`/streamers/${props.streamer.id}/editing`, { method: 'PUT', body: draft.value }); saved.value = JSON.stringify(value); draft.value = value; toast('Hochkant-Profil gespeichert.') }
  catch (e) { failure.value = errorMessage(e) }
  finally { busy.value = false }
}
function reset() { draft.value = JSON.parse(saved.value); failure.value = '' }
function beforeUnload(event: BeforeUnloadEvent) { if (dirty.value) { event.preventDefault(); event.returnValue = '' } }
onMounted(() => window.addEventListener('beforeunload', beforeUnload))
onBeforeUnmount(() => window.removeEventListener('beforeunload', beforeUnload))
</script>
<template>
  <ConnectionError v-if="error" @retry="refresh" />
  <div v-else-if="draft" class="grid items-start gap-6 pb-24 xl:grid-cols-[minmax(0,1fr)_300px]">
    <div class="space-y-6">
      <section class="panel p-6"><h2 class="flex items-center gap-2 text-sm font-semibold"><Film :size="18" />Layout für @{{ streamer.login }}</h2><p class="hint mb-5">Fertige Clips über „Zur Bearbeitung“ in die Schnittliste schicken. Jeder Auftrag speichert dieses Layout als eigene Kopie; spätere Änderungen gelten für neue Aufträge.</p><label class="flex items-start gap-3 text-sm"><input v-model="draft.auto_queue" type="checkbox" class="checkbox mt-1" /><span>Neue fertige Clips automatisch zur Bearbeitung schicken<p class="hint">Zusätzlicher Hochkant-Export · 720 × 1280 · 30 fps. Das Original bleibt erhalten.</p></span></label></section>
      <section class="panel p-6"><h3 class="mb-4 text-sm font-semibold">Quellbild & Ausschnitte</h3><div v-if="data?.preview_url" class="relative overflow-hidden rounded-lg bg-black"><img ref="image" :src="data.preview_url" alt="Letztes Clip-Vorschaubild dieses Streamers" class="block w-full" @load="draw" /><div v-for="r in regions" :key="r.key" class="pointer-events-none absolute border-2" :style="{ borderColor: r.color, left: `${draft[r.key].x*100}%`, top: `${draft[r.key].y*100}%`, width: `${draft[r.key].width*100}%`, height: `${draft[r.key].height*100}%` }"><span class="bg-black/80 px-1 text-[9px]" :style="{ color: r.color }">{{ r.label }}</span></div></div><p v-else class="rounded-lg bg-studio-950 p-8 text-sm text-studio-400">Sobald ein Clip dieses Streamers fertig ist, erscheint sein Vorschaubild hier. Erstelle bei Bedarf einen manuellen Clip.</p><p class="hint">Koordinaten in Prozent des gesamten Quellvideos. Die farbigen Rahmen zeigen, welcher Bereich ausgeschnitten wird.</p></section>
      <section v-for="r in regions" :key="r.key" class="panel p-6"><h3 class="mb-4 text-sm font-semibold" :style="{ color: r.color }">{{ r.label }}</h3>
        <div v-if="r.key === 'gameplay'" class="mb-5"><label for="framing" class="label">Hauptbild einpassen</label><select id="framing" v-model="draft.framing" class="input"><option value="fit">Vollständig zeigen · schwarze Ränder</option><option value="fill">Fläche füllen · mittig beschneiden</option></select><p class="hint">Wähle hier einen Spielausschnitt ohne die ursprüngliche Facecam, wenn sie separat angezeigt wird.</p></div>
        <div v-if="r.key === 'facecam'" class="mb-5 space-y-4"><label class="flex items-center gap-3 text-sm"><input v-model="draft.facecam_enabled" type="checkbox" class="checkbox" />Facecam separat anzeigen</label><div class="grid gap-4 sm:grid-cols-2"><div><label for="cam-position" class="label">Im Hochkant-Video</label><select id="cam-position" v-model="draft.facecam_position" class="input"><option value="top">Über dem Spiel</option><option value="bottom">Unter dem Spiel</option></select></div><div><label for="cam-height" class="label">Höhenanteil · {{ draft.facecam_percent }}%</label><input id="cam-height" v-model.number="draft.facecam_percent" type="range" min="15" max="40" class="mt-3 w-full accent-accent" /></div></div></div>
        <div v-if="r.key === 'chat'" class="mb-5"><label for="chat-mode" class="label">Chat-Behandlung</label><select id="chat-mode" v-model="draft.chat_mode" class="input"><option value="keep">Im Quellbild belassen</option><option value="hide">Chat-Bereich schwärzen</option><option value="panel">Separates Panel unten (20%)</option></select><p class="hint">Betrifft eingeblendeten Chat im Video. Das Panel übernimmt die sichtbaren Bildpixel, keine rekonstruierten Twitch-Nachrichten.</p></div>
        <div class="grid grid-cols-2 gap-3 sm:grid-cols-4"><div v-for="field in fields" :key="field.key"><label :for="`${r.key}-${field.key}`" class="label">{{ field.label }} %</label><input :id="`${r.key}-${field.key}`" :value="Number((draft[r.key][field.key]*100).toFixed(2))" type="number" min="0" max="100" step="0.5" class="input" @input="setRect(r.key,field.key,$event)" /></div></div>
      </section>
    </div>
    <aside class="panel p-5 xl:sticky xl:top-24"><h3 class="mb-4 text-sm font-semibold">Hochkant-Vorschau</h3><canvas ref="canvas" width="720" height="1280" class="aspect-[9/16] w-full rounded-lg bg-black" /><p class="hint">Statische Layout-Vorschau aus dem letzten Clip. Bei geänderter Stream-Aufteilung das Profil anpassen.</p><NuxtLink to="/edits" class="btn btn-secondary mt-4 w-full">Schnittliste öffnen</NuxtLink></aside>
    <Teleport to="body"><div v-if="dirty" class="fixed inset-x-4 bottom-5 z-40 lg:left-[272px] lg:right-6" role="region" aria-label="Ungespeichertes Hochkant-Layout"><div class="mx-auto max-w-3xl rounded-xl border border-accent/30 bg-studio-850/95 p-4 shadow-2xl backdrop-blur"><div class="flex flex-wrap items-center justify-between gap-3"><span class="text-sm">Layout für @{{ streamer.login }} geändert</span><div class="flex gap-2"><button class="btn btn-ghost text-xs" :disabled="busy" @click="reset"><RotateCcw :size="14" />Verwerfen</button><button class="btn btn-primary text-xs" :disabled="busy" @click="save"><Save :size="14" />Speichern</button></div></div><p v-if="failure" role="alert" class="mt-3 text-xs text-red-300">{{ failure }}</p></div></div></Teleport>
  </div>
  <p v-else class="panel p-8 text-sm text-studio-400">Layout wird geladen …</p>
</template>
