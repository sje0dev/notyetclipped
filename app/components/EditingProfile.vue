<script setup lang="ts">
import { Save, RotateCcw, Film, RefreshCw } from '@lucide/vue'
import type { Streamer, EditingSettings, CropRect } from '~/types'
import type { CropRegion } from '~/utils/crop'
const props = defineProps<{ streamer: Streamer }>()
const emit = defineEmits<{ dirty: [value: boolean] }>()
const { toast } = useToast()
const { data, error, status, refresh: refreshData } = await useFetch<{ profile: EditingSettings; preview_url: string | null }>(`/api/streamers/${props.streamer.id}/editing`, { key: `editing-${props.streamer.id}`, server: false })
const refresh = () => refreshData()
const draft = ref<EditingSettings>()
const saved = ref('')
const dirty = computed(() => !!draft.value && JSON.stringify(draft.value) !== saved.value)
watch(data, value => { if (value && !dirty.value) { draft.value = structuredClone(toRaw(value.profile)); saved.value = JSON.stringify(draft.value) } }, { immediate: true })
watch(dirty, value => emit('dirty', value), { immediate: true })
const busy = ref(false)
const failure = ref('')
const image = shallowRef<HTMLImageElement | null>(null)
const canvas = ref<HTMLCanvasElement>()
const selected = ref<CropRegion>('gameplay')
const facecamPositions = [{ value: 'off', label: 'Aus' }, { value: 'top', label: 'Oben' }, { value: 'bottom', label: 'Unten' }] as const
const facecamPlacement = computed({
  get: () => draft.value?.facecam_enabled ? draft.value.facecam_position : 'off',
  set: (value: 'off' | 'top' | 'bottom') => {
    if (!draft.value) return
    draft.value.facecam_enabled = value !== 'off'
    if (value !== 'off') draft.value.facecam_position = value
  },
})
let animation: number | undefined
let masked: HTMLCanvasElement | undefined
function scheduleDraw() { if (import.meta.client && animation === undefined) animation = requestAnimationFrame(() => { animation = undefined; draw() }) }
function imageLoaded(value: HTMLImageElement | null) { image.value = value; void nextTick(scheduleDraw) }
function draw() {
  const p = draft.value, img = image.value, ctx = canvas.value?.getContext('2d')
  if (!ctx) return
  ctx.fillStyle = '#000'; ctx.fillRect(0, 0, 720, 1280)
  if (!p || !img?.naturalWidth) return
  masked ||= document.createElement('canvas')
  if (masked.width !== img.naturalWidth || masked.height !== img.naturalHeight) { masked.width = img.naturalWidth; masked.height = img.naturalHeight }
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
watch(draft, scheduleDraw, { deep: true })
async function save() {
  busy.value = true; failure.value = ''
  try { const value = await api<EditingSettings>(`/streamers/${props.streamer.id}/editing`, { method: 'PUT', body: draft.value }); saved.value = JSON.stringify(value); draft.value = value; toast('Hochkant-Profil gespeichert.') }
  catch (e) { failure.value = errorMessage(e) }
  finally { busy.value = false }
}
function reset() { draft.value = JSON.parse(saved.value); failure.value = '' }
function beforeUnload(event: BeforeUnloadEvent) { if (dirty.value) { event.preventDefault(); event.returnValue = '' } }
onMounted(() => window.addEventListener('beforeunload', beforeUnload))
onBeforeUnmount(() => { window.removeEventListener('beforeunload', beforeUnload); if (animation !== undefined) cancelAnimationFrame(animation) })
</script>
<template>
  <ConnectionError v-if="error" @retry="refresh" />
  <div v-else-if="draft" class="space-y-6 pb-24">
    <section class="panel p-6"><h2 class="flex items-center gap-2 text-sm font-semibold"><Film :size="18" />Layout für @{{ streamer.login }}</h2><p class="hint mb-5">Fertige Clips über „Zur Bearbeitung“ in die Schnittliste schicken. Jeder Auftrag speichert dieses Layout als eigene Kopie; spätere Änderungen gelten für neue Aufträge.</p><label class="flex items-start gap-3 text-sm"><input v-model="draft.auto_queue" :disabled="busy" type="checkbox" class="checkbox mt-1" /><span>Neue fertige Clips automatisch zur Bearbeitung schicken<p class="hint">Zusätzlicher Hochkant-Export · 720 × 1280 · 30 fps. Das Original bleibt erhalten.</p></span></label></section>
    <div class="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_280px]">
      <div class="min-w-0 space-y-6">
        <fieldset class="panel min-w-0 p-6" :disabled="busy">
          <h3 class="text-sm font-semibold">Anordnung im Hochkant-Video</h3>
          <p class="hint mb-5">Hier legst du fest, wo die Ausschnitte im fertigen Video erscheinen.</p>
          <fieldset class="min-w-0">
            <legend class="label">Facecam</legend>
            <div class="grid grid-cols-3 gap-2">
              <label v-for="position in facecamPositions" :key="position.value" class="min-w-0 cursor-pointer">
                <input v-model="facecamPlacement" type="radio" name="facecam-placement" :value="position.value" :aria-label="`Facecam ${position.label.toLowerCase()}`" class="peer sr-only" />
                <span class="flex items-center justify-center gap-2 rounded-lg border border-studio-700 bg-studio-950 px-2 py-3 text-xs text-studio-400 transition-colors hover:bg-studio-850 peer-checked:border-lavender/70 peer-checked:bg-lavender/10 peer-checked:text-zinc-100 peer-focus-visible:ring-2 peer-focus-visible:ring-lavender peer-disabled:opacity-40">
                  <span aria-hidden="true" class="flex h-12 w-7 shrink-0 gap-0.5 overflow-hidden rounded border border-studio-700 bg-black p-0.5" :class="position.value === 'bottom' ? 'flex-col-reverse' : 'flex-col'">
                    <span v-if="position.value !== 'off'" class="h-3 shrink-0 rounded-sm bg-lavender/70" />
                    <span class="flex-1 rounded-sm bg-accent/30" />
                  </span>
                  {{ position.label }}
                </span>
              </label>
            </div>
            <div v-if="draft.facecam_enabled" class="mt-4"><label for="cam-height" class="label">Facecam-Größe</label><input id="cam-height" v-model.number="draft.facecam_percent" type="range" min="15" max="40" class="w-full accent-lavender" /><div class="mt-1 flex justify-between text-[10px] text-studio-400"><span>Kleiner</span><span>Größer</span></div></div>
            <p v-else class="hint">Wähle „Oben“ oder „Unten“, um deinen Facecam-Ausschnitt separat anzuzeigen.</p>
          </fieldset>
          <div class="mt-5 grid gap-4 sm:grid-cols-2">
            <div><label for="framing" class="label">Spiel / Hauptbild</label><select id="framing" v-model="draft.framing" class="input"><option value="fit">Vollständig zeigen · schwarze Ränder</option><option value="fill">Fläche füllen · mittig beschneiden</option></select><p class="hint">Den Spielausschnitt ohne die ursprüngliche Facecam wählst du unten im Quellbild.</p></div>
            <div><label for="chat-mode" class="label">Chat im Video</label><select id="chat-mode" v-model="draft.chat_mode" class="input"><option value="keep">Im Quellbild belassen</option><option value="hide">Chat-Bereich schwärzen</option><option value="panel">Separates Panel unten</option></select><p class="hint">Verwendet den eingeblendeten Chat aus deinem ausgewählten Ausschnitt.</p></div>
          </div>
        </fieldset>
        <section class="panel p-6"><div class="mb-4 flex items-center justify-between gap-3"><h3 class="text-sm font-semibold">Quellbild & Ausschnitte</h3><button class="icon-btn" :disabled="busy || status === 'pending'" aria-label="Vorschaubild aktualisieren" @click="refresh"><RefreshCw :size="15" /></button></div><CropEditor v-model="draft" v-model:selected="selected" :src="data?.preview_url || null" :disabled="busy" @loaded="imageLoaded" /></section>
      </div>
      <aside class="panel p-5 xl:sticky xl:top-24"><h3 class="text-sm font-semibold">Hochkant-Vorschau</h3><p class="mb-4 mt-2 text-xs text-studio-400" aria-live="polite">{{ draft.facecam_enabled ? draft.facecam_position === 'top' ? 'Facecam über dem Spiel' : 'Facecam unter dem Spiel' : 'Ohne separate Facecam' }}{{ draft.chat_mode === 'panel' ? ' · Chat-Panel unten' : '' }}</p><canvas ref="canvas" width="720" height="1280" class="mx-auto aspect-[9/16] w-full max-w-[260px] rounded-lg bg-black" /><p class="hint">Die Vorschau passt sich beim Ziehen und bei Änderungen der Anordnung sofort an. Grundlage ist das letzte Clip-Vorschaubild.</p><NuxtLink to="/edits" class="btn btn-secondary mt-4 w-full">Schnittliste öffnen</NuxtLink></aside>
    </div>
    <Teleport to="body"><div v-if="dirty" class="fixed inset-x-4 bottom-5 z-40 lg:left-[272px] lg:right-6" role="region" aria-label="Ungespeichertes Hochkant-Layout"><div class="mx-auto max-w-3xl rounded-xl border border-accent/30 bg-studio-850/95 p-4 shadow-2xl backdrop-blur"><div class="flex flex-wrap items-center justify-between gap-3"><span class="text-sm">Layout für @{{ streamer.login }} geändert</span><div class="flex gap-2"><button class="btn btn-ghost text-xs" :disabled="busy" @click="reset"><RotateCcw :size="14" />Verwerfen</button><button class="btn btn-primary text-xs" :disabled="busy" @click="save"><Save :size="14" />Speichern</button></div></div><p v-if="failure" role="alert" class="mt-3 text-xs text-red-300">{{ failure }}</p></div></div></Teleport>
  </div>
  <p v-else class="panel p-8 text-sm text-studio-400">Layout wird geladen …</p>
</template>
