<script setup lang="ts">
import { Move, ImageOff } from '@lucide/vue'
import type { CropRect, EditingSettings } from '~/types'
import type { CropHandle, CropRegion } from '~/utils/crop'
const model = defineModel<EditingSettings>({ required: true })
const selected = defineModel<CropRegion>('selected', { required: true })
const props = defineProps<{ src: string | null; disabled?: boolean }>()
const emit = defineEmits<{ loaded: [image: HTMLImageElement | null] }>()
const frame = ref<HTMLElement>()
const loading = ref(true)
const failed = ref(false)
const dragging = ref(false)
const handles = [
  { id: 'nw', label: 'oben links', x: 0, y: 0, cursor: 'nwse-resize' },
  { id: 'n', label: 'oben', x: 50, y: 0, cursor: 'ns-resize' },
  { id: 'ne', label: 'oben rechts', x: 100, y: 0, cursor: 'nesw-resize' },
  { id: 'e', label: 'rechts', x: 100, y: 50, cursor: 'ew-resize' },
  { id: 'se', label: 'unten rechts', x: 100, y: 100, cursor: 'nwse-resize' },
  { id: 's', label: 'unten', x: 50, y: 100, cursor: 'ns-resize' },
  { id: 'sw', label: 'unten links', x: 0, y: 100, cursor: 'nesw-resize' },
  { id: 'w', label: 'links', x: 0, y: 50, cursor: 'ew-resize' },
] as const
let gesture: { pointer: number; region: CropRegion; handle: CropHandle; rect: CropRect; x: number; y: number; width: number; height: number; target: HTMLElement } | undefined

function finish(cancel = false) {
  const previous = gesture
  gesture = undefined; dragging.value = false
  if (!previous) return
  if (cancel) model.value[previous.region] = previous.rect
  if (previous.target.hasPointerCapture(previous.pointer)) previous.target.releasePointerCapture(previous.pointer)
}
function start(event: PointerEvent, region: CropRegion, handle: CropHandle) {
  if (props.disabled || gesture || event.button !== 0 || !event.isPrimary) return
  const bounds = frame.value?.getBoundingClientRect()
  if (!bounds?.width || !bounds.height) return
  event.preventDefault()
  const target = event.currentTarget as HTMLElement
  target.focus({ preventScroll: true })
  target.setPointerCapture(event.pointerId)
  selected.value = region
  gesture = { pointer: event.pointerId, region, handle, rect: { ...model.value[region] }, x: event.clientX, y: event.clientY, width: bounds.width, height: bounds.height, target }
  dragging.value = true
}
function move(event: PointerEvent) {
  if (!gesture || event.pointerId !== gesture.pointer) return
  const g = gesture
  model.value[g.region] = transformCrop(g.rect, g.handle, (event.clientX - g.x) / g.width, (event.clientY - g.y) / g.height)
}
function end(event: PointerEvent, cancel = false) {
  if (!gesture || event.pointerId !== gesture.pointer) return
  if (!cancel) move(event)
  finish(cancel)
}
function keyboard(event: KeyboardEvent, region: CropRegion, handle: CropHandle) {
  if (props.disabled || gesture || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return
  event.preventDefault()
  const dx = event.key === 'ArrowLeft' ? -.005 : event.key === 'ArrowRight' ? .005 : 0
  const dy = event.key === 'ArrowUp' ? -.005 : event.key === 'ArrowDown' ? .005 : 0
  model.value[region] = transformCrop(model.value[region], handle === 'move' && event.shiftKey ? 'se' : handle, dx, dy)
}
function loaded(event: Event) { loading.value = false; failed.value = false; emit('loaded', event.target as HTMLImageElement) }
function imageError() { loading.value = false; failed.value = true; emit('loaded', null) }
watch(() => props.src, () => { finish(); loading.value = true; failed.value = false; emit('loaded', null) })
watch(() => props.disabled, value => { if (value) finish() })
onBeforeUnmount(() => finish())
</script>
<template>
  <div>
    <div class="mb-4 flex flex-wrap gap-2" role="group" aria-label="Ausschnitt auswählen">
      <button v-for="region in cropRegions" :key="region.key" type="button" class="btn btn-secondary text-xs" :disabled="disabled" :aria-pressed="selected === region.key" :style="selected === region.key ? { borderColor: region.color, color: region.color } : {}" @click="selected = region.key"><span class="size-2 rounded-full" :style="{ backgroundColor: region.color }" />{{ region.label }}</button>
    </div>
    <p id="crop-help" class="mb-5 text-xs leading-relaxed text-studio-400"><Move :size="13" class="mr-1 inline" />Box ziehen zum Verschieben. An den acht Griffen die Größe ändern. Mit Pfeiltasten fein verschieben, mit Umschalt + Pfeiltasten vergrößern oder verkleinern. Escape bricht das Ziehen ab.</p>
    <div ref="frame" class="relative select-none rounded-lg bg-black" :class="{ 'aspect-video': !src || loading || failed }" @pointermove="move" @pointerup="end($event)" @pointercancel="end($event, true)" @lostpointercapture="finish()" @keydown.esc.stop.prevent="finish(true)">
      <img v-if="src && !failed" :key="src" :src="src" alt="Letztes Clip-Vorschaubild – Ausschnitte für Spiel, Facecam und Chat" draggable="false" class="pointer-events-none block w-full rounded-lg" :class="{ invisible: loading }" @load="loaded" @error="imageError" />
      <div v-if="!src || failed || loading" class="pointer-events-none absolute inset-0 flex items-center justify-center rounded-lg bg-studio-950 px-8 text-center text-xs text-studio-400"><span><ImageOff :size="24" class="mx-auto mb-3" />{{ src && loading && !failed ? 'Vorschaubild wird geladen …' : failed ? 'Vorschaubild nicht erreichbar. Bitte aktualisieren.' : 'Noch kein Clip-Vorschaubild. Erstelle einen manuellen Clip und aktualisiere danach die Vorschau.' }}</span></div>
      <div v-for="region in cropRegions" :key="region.key" class="pointer-events-none absolute border-2" :class="selected === region.key ? '' : 'z-10 border-dashed opacity-60'" :style="{ borderColor: region.color, left: `${model[region.key].x * 100}%`, top: `${model[region.key].y * 100}%`, width: `${model[region.key].width * 100}%`, height: `${model[region.key].height * 100}%` }">
        <button v-if="selected === region.key" type="button" :disabled="disabled" class="pointer-events-auto absolute inset-0 z-20 touch-none bg-transparent text-left outline-none focus-visible:ring-2 focus-visible:ring-white" :class="dragging ? 'cursor-grabbing' : 'cursor-grab'" :aria-label="`${region.label} verschieben`" aria-describedby="crop-help" @pointerdown="start($event, region.key, 'move')" @keydown="keyboard($event, region.key, 'move')"><span class="pointer-events-none absolute left-1 top-1 max-w-[calc(100%-8px)] truncate rounded bg-black/80 px-1.5 py-1 text-[10px]" :style="{ color: region.color }">{{ region.label }}</span></button>
        <template v-if="selected === region.key"><button v-for="handle in handles" :key="handle.id" type="button" :disabled="disabled" class="pointer-events-auto absolute z-40 flex size-7 -translate-x-1/2 -translate-y-1/2 touch-none items-center justify-center rounded focus-visible:outline-2 focus-visible:outline-white" :style="{ left: `${handle.x}%`, top: `${handle.y}%`, cursor: handle.cursor }" :aria-label="`${region.label}: Größe ${handle.label} ändern`" aria-describedby="crop-help" @pointerdown.stop="start($event, region.key, handle.id)" @keydown="keyboard($event, region.key, handle.id)"><span class="pointer-events-none size-3 rounded-sm border-2 border-white shadow-md" :style="{ backgroundColor: region.color }" /></button></template>
      </div>
      <template v-for="region in cropRegions" :key="`label-${region.key}`"><button v-if="selected !== region.key" type="button" :disabled="disabled" class="absolute z-30 truncate rounded bg-black/85 px-2 py-1 text-[10px]" :style="{ left: `${model[region.key].x * 100}%`, top: `${model[region.key].y * 100}%`, maxWidth: `${Math.min(.45, 1 - model[region.key].x) * 100}%`, color: region.color }" :aria-label="`${region.label} auswählen`" @click="selected = region.key">{{ region.label }}</button></template>
    </div>
    <p v-if="selected === 'facecam' && !model.facecam_enabled" class="hint mt-4">Facecam ist noch ausgeblendet. Aktiviere sie unten, um den Ausschnitt auch in der Hochkant-Vorschau zu sehen.</p>
    <p v-else-if="selected === 'chat' && model.chat_mode === 'keep'" class="hint mt-4">Chat bleibt aktuell im Quellbild. Wähle unten „schwärzen“ oder „separates Panel“, um diesen Ausschnitt zu verwenden.</p>
  </div>
</template>
