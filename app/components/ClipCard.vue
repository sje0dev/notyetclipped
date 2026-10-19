<script setup lang="ts">
import { Play, Bookmark, AudioLines, MousePointer2, Film, LoaderCircle, AlertTriangle } from '@lucide/vue'
import type { Clip } from '~/types'
const props = defineProps<{ clip: Clip; selectable?: boolean; selected?: boolean }>()
const emit = defineEmits<{ open: [id: string]; changed: []; select: [id: string] }>()
const { toast } = useToast()
const busy = ref(false)
async function favorite() {
  busy.value = true
  try { await api(`/clips/${props.clip.id}`, { method: 'PATCH', body: { favorite: !props.clip.favorite } }); emit('changed') }
  catch (e) { toast(errorMessage(e), true) }
  finally { busy.value = false }
}
</script>
<template>
  <article class="group panel overflow-hidden transition hover:-translate-y-0.5 hover:border-studio-400/40" :class="{ 'ring-1 ring-lime-300/60': selected }">
    <div class="relative aspect-video overflow-hidden bg-studio-800">
      <button class="absolute inset-0 size-full" :aria-label="`Clip öffnen: ${clip.title}`" @click="emit('open', clip.id)">
        <img v-if="clip.thumbnail_url" :src="clip.thumbnail_url" :alt="clip.title" class="size-full object-cover transition duration-500 group-hover:scale-105" loading="lazy" />
        <div v-else class="flex size-full items-center justify-center bg-studio-850"><AlertTriangle v-if="clip.status === 'failed'" :size="30" class="text-amber-200/40" /><LoaderCircle v-else-if="['cutting', 'uploading', 'collecting'].includes(clip.status)" :size="30" class="animate-spin text-studio-400" /><Film v-else :size="34" class="text-studio-400/35" /></div>
        <div class="absolute inset-0 bg-gradient-to-t from-black/65 via-transparent to-black/5" /><span v-if="clip.status === 'ready'" class="absolute left-1/2 top-1/2 flex size-11 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border border-white/25 bg-black/35 text-white opacity-0 backdrop-blur-sm transition group-hover:opacity-100"><Play :size="18" fill="currentColor" class="ml-0.5" /></span>
      </button>
      <label v-if="selectable" class="absolute left-3 top-3 flex size-6 items-center justify-center rounded bg-black/30"><input type="checkbox" :checked="selected" :aria-label="`${clip.title} auswählen`" class="size-4 rounded border-white/40 bg-black/30 text-lime-300 focus:ring-lime-300" @change="emit('select', clip.id)" /></label>
      <span v-else class="absolute left-3 top-3 rounded bg-black/50 px-2 py-1 text-[9px] font-semibold text-zinc-200 backdrop-blur-sm">{{ clip.streamer_login }}</span>
      <span class="pointer-events-none absolute bottom-3 left-3"><StatusBadge :status="clip.status" /></span><span class="absolute bottom-3 right-3 rounded bg-black/65 px-1.5 py-1 font-mono text-[10px] text-white">{{ duration(clip.duration) }}</span>
    </div>
    <div class="p-4"><button class="block w-full truncate text-left text-sm font-medium text-zinc-200 hover:text-white" @click="emit('open', clip.id)">{{ clip.title }}</button><div class="mt-2 flex items-center gap-1.5 text-xs text-studio-400"><AudioLines v-if="clip.trigger !== 'manual'" :size="12" /><MousePointer2 v-else :size="12" />{{ clipSource(clip.trigger) }}<span class="ml-auto text-[11px]">{{ bytes(clip.size_bytes) }}</span></div><p v-if="clip.trigger !== 'manual'" class="mt-2 line-clamp-2 text-xs leading-relaxed text-studio-400" :title="clip.reason">{{ clip.reason }}</p><div class="mt-4 flex items-center justify-between border-t divider pt-3"><span class="text-[11px] text-studio-400">{{ dateTime(clip.created_at) }}</span><button class="icon-btn size-6" :disabled="busy" :aria-label="clip.favorite ? 'Favorit entfernen' : 'Als Favorit speichern'" :aria-pressed="clip.favorite" @click="favorite"><Bookmark :size="14" :fill="clip.favorite ? 'currentColor' : 'none'" :class="{ 'text-lime-300': clip.favorite }" /></button></div></div>
  </article>
</template>
