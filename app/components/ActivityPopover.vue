<script setup lang="ts">
import { Bell, X } from '@lucide/vue'
const root = ref<HTMLElement>()
const button = ref<HTMLButtonElement>()
const open = ref(false)
const seen = ref(0)
const { data, error, refresh: refreshData } = await useFetch<{ id: number; message: string; created_at: string; streamer_id: string | null }[]>('/api/activities', { key: 'activities', server: false })
const refresh = () => refreshData()
usePolling(refresh, 30000)
const unread = computed(() => data.value?.filter(e => e.id > seen.value).length || 0)
watch(open, async value => {
  if (!value) return
  await refresh()
  seen.value = Math.max(seen.value, ...((data.value || []).map(e => e.id)))
  try { localStorage.setItem('activity-seen', String(seen.value)) } catch { /* Optional local marker. */ }
})
function close() { open.value = false; button.value?.focus() }
function outside(event: PointerEvent) { if (!root.value?.contains(event.target as Node)) open.value = false }
onMounted(() => { try { seen.value = Number(localStorage.getItem('activity-seen')) || 0 } catch { /* Optional local marker. */ }; document.addEventListener('pointerdown', outside) })
onBeforeUnmount(() => document.removeEventListener('pointerdown', outside))
</script>
<template>
  <div ref="root" class="relative" @keydown.esc.stop="close">
    <button ref="button" class="icon-btn relative" aria-label="Aktivitäten anzeigen" :aria-expanded="open" aria-controls="activity-popover" @click="open = !open"><Bell :size="18" /><span v-if="unread" class="absolute right-1 top-1 size-1.5 rounded-full bg-accent" /></button>
    <section v-if="open" id="activity-popover" aria-label="Aktivitäten" class="panel fixed right-3 top-[72px] z-50 w-[min(400px,calc(100vw-24px))] overflow-hidden shadow-2xl sm:absolute sm:right-0 sm:top-11">
      <div class="flex items-center justify-between border-b divider p-4"><h2 class="text-sm font-semibold">Aktivitäten</h2><button class="icon-btn" aria-label="Aktivitäten schließen" @click="close"><X :size="15" /></button></div>
      <div class="max-h-[65dvh] overflow-y-auto p-4"><p v-if="error" class="hint">Aktivitäten konnten nicht geladen werden. <button class="underline" @click="refresh">Erneut laden</button></p><p v-else-if="!data?.length" class="hint">Noch keine Aktivitäten.</p><article v-for="event in data" :key="event.id" class="border-b divider py-3 last:border-0"><p class="text-xs leading-relaxed">{{ event.message }}</p><div class="mt-2 flex justify-between gap-3 text-[10px] text-studio-400"><span>{{ dateTime(event.created_at) }}</span><NuxtLink v-if="event.streamer_id" :to="`/streamers/${event.streamer_id}`" class="text-accent" @click="open = false">Streamer öffnen</NuxtLink></div></article></div>
    </section>
  </div>
</template>
