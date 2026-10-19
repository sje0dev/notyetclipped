<script setup lang="ts">
const props = defineProps<{ status: string }>()
const labels: Record<string, string> = {
  recording: 'Live · Aufnahme', connecting: 'Verbinden', waiting: 'Wartet', paused: 'Pausiert', offline: 'Offline',
  ready: 'Bereit', collecting: 'Nachlauf', queued: 'Warteschlange', cutting: 'Schnitt', uploading: 'Upload', retry: 'Neuer Versuch', failed: 'Fehler', deleting: 'Wird gelöscht',
}
const color = computed(() => {
  if (['recording', 'ready'].includes(props.status)) return 'border-lime-300/20 bg-lime-300/8 text-lime-300'
  if (['connecting', 'collecting', 'cutting', 'uploading', 'queued', 'retry'].includes(props.status)) return 'border-amber-300/20 bg-amber-300/8 text-amber-200'
  if (props.status === 'failed') return 'border-red-400/20 bg-red-400/10 text-red-300'
  return 'border-studio-700 bg-studio-800 text-studio-400'
})
</script>
<template>
  <span class="badge" :class="color"><span class="size-1.5 rounded-full bg-current" :class="{ 'animate-pulse': status === 'recording' }" />{{ labels[status] || status }}</span>
</template>
