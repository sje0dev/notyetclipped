<script setup lang="ts">
import { ArrowLeft, SlidersHorizontal } from '@lucide/vue'
import type { Streamer } from '~/types'
definePageMeta({ key: route => String(route.params.id) })
const route = useRoute()
const { data, error, refresh } = await useFetch<Streamer[]>('/api/streamers', { key: 'streamers', timeout: 8000 })
usePolling(refresh)
const selected = computed(() => data.value?.find(s => s.id === route.params.id))
const tab = ref(route.query.tab === 'layout' ? 'layout' : 'triggers')
const dirty = ref(false)
const formOpen = ref(false)
function change(next: string) {
  if (next === tab.value || (dirty.value && !window.confirm('Ungespeicherte Änderungen verwerfen?'))) return
  dirty.value = false; tab.value = next
}
onBeforeRouteLeave(() => !dirty.value || window.confirm('Ungespeicherte Änderungen verwerfen?'))
onBeforeRouteUpdate(() => !dirty.value || window.confirm('Ungespeicherte Änderungen verwerfen?'))
</script>
<template>
  <div>
    <NuxtLink to="/streamers" class="mb-5 inline-flex items-center gap-2 text-xs text-studio-400"><ArrowLeft :size="14" />Alle Streamer</NuxtLink>
    <ConnectionError v-if="error" @retry="refresh" />
    <template v-else-if="selected">
      <div class="mb-6 flex flex-wrap items-start justify-between gap-4"><div><h1 class="page-title">@{{ selected.login }}</h1><p class="mt-2 text-sm text-studio-400">Trigger und Bearbeitungsprofil für diesen Streamer.</p></div><button class="btn btn-secondary" @click="formOpen = true"><SlidersHorizontal :size="15" />Aufnahme einstellen</button></div>
      <nav class="mb-6 flex gap-6 border-b divider" aria-label="Streamer-Bereiche"><button class="tab" :class="{ 'tab-active': tab === 'triggers' }" @click="change('triggers')">Automationen</button><button class="tab" :class="{ 'tab-active': tab === 'layout' }" @click="change('layout')">Hochkant & Bearbeitung</button></nav>
      <TriggerBuilder v-if="tab === 'triggers'" :streamer="selected" :streamers="data || []" @saved="refresh" @dirty="dirty = $event" />
      <EditingProfile v-else :streamer="selected" @dirty="dirty = $event" />
      <StreamerForm :open="formOpen" :streamer="selected" @close="formOpen = false" @saved="refresh" />
    </template>
    <p v-else class="panel p-8 text-sm text-studio-400">Dieser Streamer ist nicht mehr in deiner Watchlist.</p>
  </div>
</template>
