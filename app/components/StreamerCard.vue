<script setup lang="ts">
import { Scissors, Pause, Play, SlidersHorizontal, Volume2, ExternalLink, LoaderCircle } from '@lucide/vue'
import type { Streamer } from '~/types'
const props = defineProps<{ streamer: Streamer }>()
const emit = defineEmits<{ edit: [streamer: Streamer]; changed: [] }>()
const { toast } = useToast()
const busy = ref('')
const level = computed(() => props.streamer.audio_db === null ? 0 : Math.max(0, (props.streamer.audio_db + 60) / 60 * 100))
async function action(kind: 'toggle' | 'clip') {
  busy.value = kind
  try {
    if (kind === 'toggle') await api(`/streamers/${props.streamer.id}`, { method: 'PATCH', body: { enabled: !props.streamer.enabled } })
    else {
      await api(`/streamers/${props.streamer.id}/clip`, { method: 'POST' })
      toast('Moment gesichert. Nach dem Nachlauf wird dein Clip erstellt.')
    }
    emit('changed')
  } catch (e) { toast(errorMessage(e), true) }
  finally { busy.value = '' }
}
</script>
<template>
  <article class="panel overflow-hidden transition hover:border-studio-400/40">
    <div class="flex items-center gap-3 p-5"><div class="flex size-10 shrink-0 items-center justify-center rounded-xl border border-lime-300/15 bg-lime-300/8 text-sm font-semibold text-lime-300">{{ streamer.login.slice(0, 2).toUpperCase() }}</div><div class="min-w-0 flex-1"><a :href="`https://twitch.tv/${streamer.login}`" target="_blank" rel="noopener noreferrer" class="group flex items-center gap-1.5 text-sm font-semibold">{{ streamer.login }}<ExternalLink :size="11" class="text-studio-400 opacity-0 group-hover:opacity-100" /></a><p class="mt-1 text-[10px] text-studio-400">{{ streamer.quality.split(',')[0] === 'best' ? 'Beste Qualität' : streamer.quality.split(',')[0] }} <span class="mx-1.5">·</span> {{ streamer.auto_clip ? 'Auto-Clip aktiv' : 'Manueller Modus' }}</p></div><button class="icon-btn" :aria-label="`${streamer.login} einstellen`" @click="emit('edit', streamer)"><SlidersHorizontal :size="16" /></button></div>
    <div class="mx-5 rounded-xl border divider bg-studio-950/70 p-4"><div class="mb-4 flex items-center justify-between"><span class="eyebrow flex items-center gap-1.5"><Volume2 :size="12" />Audio-Signal</span><span class="font-mono text-[10px] text-studio-400">{{ streamer.status === 'recording' && streamer.audio_db !== null ? `${streamer.audio_db.toFixed(1)} dB` : '— dB' }}</span></div><div class="flex h-7 items-end gap-[3px]" aria-label="Aktueller Audiopegel"><span v-for="n in 35" :key="n" class="meter-bar flex-1 rounded-[2px]" :class="streamer.status === 'recording' && n <= level / 100 * 35 ? (n > 30 ? 'bg-amber-300' : 'bg-lime-300/80') : 'bg-studio-700/50'" :style="{ height: `${35 + n * 1.85}%` }" /></div></div>
    <div class="flex items-center justify-between px-5 py-4"><StatusBadge :status="streamer.status" /><span class="text-[10px] text-studio-400">Puffer <span class="ml-1 font-mono text-zinc-300">{{ duration(streamer.buffer_seconds) }}</span></span></div>
    <p v-if="streamer.last_error" class="mx-5 mb-3 line-clamp-2 text-[10px] leading-relaxed text-amber-200/70" :title="streamer.last_error">{{ streamer.last_error }}</p>
    <div class="flex gap-2 border-t divider bg-studio-950/25 px-5 py-3"><button class="btn btn-secondary flex-1 py-2 text-xs" :disabled="streamer.status !== 'recording' || !!busy" @click="action('clip')"><LoaderCircle v-if="busy === 'clip'" :size="14" class="animate-spin" /><Scissors v-else :size="14" />Moment clippen</button><button class="btn btn-ghost px-2.5 py-2" :disabled="!!busy" :aria-label="streamer.enabled ? `${streamer.login} pausieren` : `${streamer.login} aktivieren`" @click="action('toggle')"><LoaderCircle v-if="busy === 'toggle'" :size="15" class="animate-spin" /><Pause v-else-if="streamer.enabled" :size="15" /><Play v-else :size="15" /></button></div>
  </article>
</template>
