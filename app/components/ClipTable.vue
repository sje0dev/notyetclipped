<script setup lang="ts">
import { Play, Bookmark, Clapperboard } from '@lucide/vue'
import type { Clip } from '~/types'
defineProps<{ clips: Clip[]; selected: string[] }>()
const emit = defineEmits<{ open: [id: string]; select: [id: string]; changed: [] }>()
const busy = ref<string | null>(null)
const { toast } = useToast()
async function favorite(clip: Clip) {
  busy.value = clip.id
  try { await api(`/clips/${clip.id}`, { method: 'PATCH', body: { favorite: !clip.favorite } }); emit('changed') }
  catch (e) { toast(errorMessage(e), true) }
  finally { busy.value = null }
}
</script>
<template>
  <div class="overflow-x-auto">
    <table class="data-table">
      <thead><tr><th class="w-10"><span class="sr-only">Auswahl</span></th><th>Moment</th><th>Streamer</th><th>Auslöser</th><th>Dauer</th><th>Status</th><th><span class="sr-only">Favorit</span></th></tr></thead>
      <tbody><tr v-for="clip in clips" :key="clip.id" :class="{ 'bg-accent/5': selected.includes(clip.id) }">
        <td><input type="checkbox" class="checkbox" :checked="selected.includes(clip.id)" :aria-label="`${clip.title} auswählen`" @change="emit('select', clip.id)" /></td>
        <td><button class="group flex min-w-64 max-w-lg items-center gap-4 text-left" @click="emit('open', clip.id)"><span class="relative flex h-14 w-24 shrink-0 items-center justify-center overflow-hidden rounded-md bg-studio-950"><img v-if="clip.thumbnail_url" :src="clip.thumbnail_url" alt="" class="size-full object-cover" loading="lazy" /><Clapperboard v-else :size="20" class="text-studio-700" /><span class="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 transition group-hover:opacity-100 group-focus-visible:opacity-100"><Play :size="17" /></span></span><span class="min-w-0"><span class="line-clamp-1 font-medium">{{ clip.title }}</span><span class="mt-1.5 block text-xs text-studio-400">{{ dateTime(clip.created_at) }}</span></span></button></td>
        <td><span class="flex items-center gap-2.5"><span class="flex size-7 items-center justify-center rounded-full bg-studio-800 text-xs text-studio-400">{{ clip.streamer_login.slice(0, 1).toUpperCase() }}</span>{{ clip.streamer_login }}</span></td>
        <td><span class="badge border-transparent" :class="['story', 'visual'].includes(clip.trigger) ? 'bg-lavender/10 text-lavender' : clip.trigger === 'manual' ? 'bg-studio-800 text-studio-400' : 'bg-accent/10 text-accent'">{{ clipSource(clip.trigger) }}</span></td>
        <td class="tabular-nums text-studio-400">{{ duration(clip.duration) }}</td><td><StatusBadge :status="clip.status" /></td>
        <td><button class="icon-btn" :disabled="busy === clip.id" :aria-label="clip.favorite ? 'Favorit entfernen' : 'Als Favorit speichern'" :aria-pressed="clip.favorite" @click="favorite(clip)"><Bookmark :size="16" :fill="clip.favorite ? 'currentColor' : 'none'" :class="{ 'text-accent': clip.favorite }" /></button></td>
      </tr></tbody>
    </table>
  </div>
</template>
