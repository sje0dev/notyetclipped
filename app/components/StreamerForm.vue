<script setup lang="ts">
import { Radio, LoaderCircle, Plus, Save } from '@lucide/vue'
import type { Streamer } from '~/types'
const props = defineProps<{ open: boolean; streamer?: Streamer | null }>()
const emit = defineEmits<{ close: []; saved: [] }>()
const { toast } = useToast()
const busy = ref(false)
const error = ref('')
const form = reactive({ login: '', enabled: true, quality: '720p,best', priority: 0, auto_clip: true })
watch(() => props.open, (open) => {
  if (!open) return
  error.value = ''
  Object.assign(form, props.streamer ? {
    login: props.streamer.login, enabled: props.streamer.enabled, quality: props.streamer.quality,
    priority: props.streamer.priority, auto_clip: props.streamer.auto_clip,
  } : { login: '', enabled: true, quality: '720p,best', priority: 0, auto_clip: true })
}, { immediate: true })
async function save() {
  busy.value = true
  error.value = ''
  try {
    const { login, ...settings } = form
    await api(props.streamer ? `/streamers/${props.streamer.id}` : '/streamers', {
      method: props.streamer ? 'PATCH' : 'POST', body: props.streamer ? settings : { login, ...settings },
    })
    toast(props.streamer ? 'Streamer-Einstellungen gespeichert.' : `@${login.replace(/.*twitch\.tv\//, '')} ist auf deiner Watchlist.`)
    emit('saved')
    emit('close')
  } catch (e) { error.value = errorMessage(e) }
  finally { busy.value = false }
}
</script>
<template>
  <AppModal :open="open" :title="streamer ? 'Streamer einstellen' : 'Streamer hinzufügen'" @close="!busy && emit('close')">
    <form class="space-y-5" @submit.prevent="save">
      <div class="flex items-start gap-3 rounded-lg border divider bg-studio-850 p-4"><Radio class="mt-0.5 shrink-0 text-studio-400" :size="20" /><p class="text-xs leading-relaxed text-studio-400">Sobald der Kanal live ist, startet die Aufnahme. Neue Kanäle verwenden „Audio + Chat“: ein Audioanstieg und mehrere aktive Chatter müssen gemeinsam auftreten.</p></div>
      <div><label for="streamer-login" class="label">Twitch-Kanal</label><input id="streamer-login" v-model="form.login" class="input" placeholder="Kanalname oder twitch.tv/…" required :disabled="!!streamer" autocomplete="off" autofocus /></div>
      <div class="grid grid-cols-2 gap-4">
        <div><label for="streamer-quality" class="label">Aufnahmequalität</label><select id="streamer-quality" v-model="form.quality" class="input"><option value="best">Beste verfügbar</option><option value="1080p60,1080p,best">1080p</option><option value="720p60,720p,best">720p / 60 FPS</option><option value="720p,best">720p</option><option value="480p,worst">480p</option></select></div>
        <div><label for="streamer-priority" class="label">Priorität · 0–100</label><input id="streamer-priority" v-model.number="form.priority" class="input" type="number" min="0" max="100" required /></div>
      </div>
      <p class="hint">Trigger, Clip-Länge und Limits konfigurierst du im <strong class="text-zinc-300">Automationen-Builder</strong> für diesen Kanal.</p>
      <label class="flex cursor-pointer items-center justify-between rounded-lg border divider p-3.5"><span class="text-sm">Automatisch Clips erstellen</span><input v-model="form.auto_clip" type="checkbox" class="size-4 rounded border-studio-700 bg-studio-950 text-lime-300 focus:ring-lime-300" /></label>
      <p v-if="error" role="alert" class="rounded-lg bg-red-400/10 p-3 text-sm text-red-300">{{ error }}</p>
      <div class="flex justify-end gap-2 border-t divider pt-5"><button type="button" class="btn btn-ghost" :disabled="busy" @click="emit('close')">Abbrechen</button><button class="btn btn-primary" :disabled="busy"><LoaderCircle v-if="busy" class="animate-spin" :size="16" /><Save v-else-if="streamer" :size="16" /><Plus v-else :size="16" />{{ streamer ? 'Speichern' : 'Zur Watchlist' }}</button></div>
    </form>
  </AppModal>
</template>
