<script setup lang="ts">
import { Workflow, Radio } from '@lucide/vue'
import type { Streamer } from '~/types'
const route = useRoute()
const { data, error, refresh } = await useFetch<Streamer[]>('/api/streamers', { key: 'streamers', timeout: 8000 })
usePolling(refresh)
const selectedId = ref(typeof route.query.streamer === 'string' ? route.query.streamer : '')
const dirty = ref(false)
const selected = computed(() => data.value?.find(s => s.id === selectedId.value))
watch(data, list => { if (list?.length && !list.some(s => s.id === selectedId.value)) selectedId.value = list[0]!.id }, { immediate: true })
watch(() => route.query.streamer, id => { if (typeof id === 'string') selectedId.value = id })
function change(event: Event) {
  const input = event.target as HTMLSelectElement
  if (dirty.value && !window.confirm('Ungespeicherte Trigger-Änderungen verwerfen?')) { input.value = selectedId.value; return }
  dirty.value = false
  selectedId.value = input.value
  void navigateTo({ path: '/triggers', query: { streamer: input.value } }, { replace: true })
}
onBeforeRouteLeave(() => !dirty.value || window.confirm('Ungespeicherte Trigger-Änderungen verwerfen?'))
onBeforeRouteUpdate(() => !dirty.value || window.confirm('Ungespeicherte Trigger-Änderungen verwerfen?'))
</script>
<template>
  <div>
    <div class="mb-7 flex flex-wrap items-start justify-between gap-4">
      <div><h1 class="page-title">Automationen</h1><p class="mt-2 text-sm text-studio-400">Bestimme, wann ein Moment zum Clip wird. Für jeden Kanal individuell.</p></div>
      <div v-if="data?.length" class="min-w-56"><label for="trigger-streamer" class="label">Streamer</label><select id="trigger-streamer" :value="selectedId" class="input" @change="change"><option v-for="streamer in data" :key="streamer.id" :value="streamer.id">@{{ streamer.login }}</option></select></div>
    </div>
    <ConnectionError v-if="error" @retry="refresh" />
    <TriggerBuilder v-else-if="selected" :key="selected.id" :streamer="selected" :streamers="data || []" @saved="refresh" @dirty="dirty = $event" />
    <div v-else class="panel flex flex-col items-center gap-4 p-16 text-center"><Workflow :size="30" class="text-studio-400" /><h2 class="font-medium">Deine erste Automation</h2><p class="text-sm text-studio-400">Füge einen Streamer hinzu, um seine Clip-Regeln zu erstellen.</p><NuxtLink to="/streamers" class="btn btn-primary"><Radio :size="16" />Zu den Streamern</NuxtLink></div>
  </div>
</template>
