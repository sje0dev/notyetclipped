<script setup lang="ts">
import { Scissors, Pause, Play, SlidersHorizontal, Workflow, LoaderCircle, ArrowUpRight } from '@lucide/vue'
import type { Streamer } from '~/types'
const props = defineProps<{ streamer: Streamer }>()
const emit = defineEmits<{ edit: [streamer: Streamer]; changed: [] }>()
const { toast } = useToast()
const busy = ref('')
const level = computed(() => props.streamer.audio_db === null ? 0 : Math.min(100, Math.max(0, (props.streamer.audio_db + 60) / 60 * 100)))
async function action(kind: 'toggle' | 'clip') {
  busy.value = kind
  try {
    if (kind === 'toggle') await api(`/streamers/${props.streamer.id}`, { method: 'PATCH', body: { enabled: !props.streamer.enabled } })
    else { await api(`/streamers/${props.streamer.id}/clip`, { method: 'POST' }); toast('Moment gesichert. Nach dem Nachlauf wird dein Clip erstellt.') }
    emit('changed')
  } catch (e) { toast(errorMessage(e), true) }
  finally { busy.value = '' }
}
</script>
<template>
  <article class="panel overflow-hidden">
    <div class="flex items-center gap-3 p-5"><div class="flex size-10 shrink-0 items-center justify-center rounded-lg border divider bg-studio-800 text-xs font-semibold text-zinc-300">{{ streamer.login.slice(0, 2).toUpperCase() }}</div><div class="min-w-0 flex-1"><a :href="`https://twitch.tv/${streamer.login}`" target="_blank" rel="noopener noreferrer" class="flex items-center gap-1.5 text-sm font-semibold">{{ streamer.login }}<ArrowUpRight :size="12" class="text-studio-400" /></a><p class="mt-1 text-[11px] text-studio-400">{{ streamer.quality.split(',')[0] }} · {{ streamer.auto_clip ? 'Automatik' : 'Manuell' }}</p></div><button class="icon-btn" :aria-label="`${streamer.login} einstellen`" @click="emit('edit', streamer)"><SlidersHorizontal :size="15" /></button></div>
    <div class="space-y-4 px-5 pb-5"><div class="flex items-center justify-between"><StatusBadge :status="streamer.status" /><span class="text-[11px] text-studio-400">{{ duration(streamer.buffer_seconds) }} Puffer</span></div><div><div class="mb-2 flex justify-between text-[10px] text-studio-400"><span>Audiopegel</span><span class="font-mono">{{ streamer.status === 'recording' && streamer.audio_db !== null ? `${streamer.audio_db.toFixed(1)} dBFS` : '—' }}</span></div><div class="h-1 overflow-hidden rounded-full bg-studio-800"><div class="h-full rounded-full bg-accent/60 transition-all" :style="{ width: `${streamer.status === 'recording' ? level : 0}%` }" /></div></div><NuxtLink :to="`/triggers?streamer=${streamer.id}`" class="flex items-center justify-between rounded-lg border divider px-3 py-2.5 text-xs text-studio-400 hover:bg-studio-850"><span class="flex items-center gap-2"><Workflow :size="13" />{{ streamer.trigger_profile.rules.filter(r => r.enabled).length }} Trigger-Regeln</span><ArrowUpRight :size="12" /></NuxtLink><p v-if="streamer.last_error" class="line-clamp-2 text-[10px] leading-relaxed text-amber-200/70" :title="streamer.last_error">{{ streamer.last_error }}</p></div>
    <div class="flex gap-2 border-t divider px-5 py-3"><button class="btn btn-secondary flex-1 text-xs" :disabled="streamer.status !== 'recording' || !!busy" @click="action('clip')"><LoaderCircle v-if="busy === 'clip'" :size="14" class="animate-spin" /><Scissors v-else :size="14" />Moment clippen</button><button class="btn btn-ghost px-2.5" :disabled="!!busy" :aria-label="streamer.enabled ? `${streamer.login} pausieren` : `${streamer.login} aktivieren`" @click="action('toggle')"><LoaderCircle v-if="busy === 'toggle'" :size="15" class="animate-spin" /><Pause v-else-if="streamer.enabled" :size="15" /><Play v-else :size="15" /></button></div>
  </article>
</template>
