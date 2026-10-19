<script setup lang="ts">
import { Radio, LoaderCircle, Plus, Save } from '@lucide/vue'
import type { Streamer } from '~/types'
const props = defineProps<{ open: boolean; streamer?: Streamer | null }>()
const emit = defineEmits<{ close: []; saved: [] }>()
const { toast } = useToast()
const busy = ref(false)
const error = ref('')
const form = reactive({ login: '', enabled: true, quality: '720p,best', priority: 0, pre_roll: 30, post_roll: 20, threshold_db: 9, auto_clip: true })
watch(() => props.open, (open) => {
  if (!open) return
  error.value = ''
  Object.assign(form, props.streamer ? {
    login: props.streamer.login, enabled: props.streamer.enabled, quality: props.streamer.quality,
    priority: props.streamer.priority, pre_roll: props.streamer.pre_roll, post_roll: props.streamer.post_roll,
    threshold_db: props.streamer.threshold_db, auto_clip: props.streamer.auto_clip,
  } : { login: '', enabled: true, quality: '720p,best', priority: 0, pre_roll: 30, post_roll: 20, threshold_db: 9, auto_clip: true })
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
      <div class="flex items-start gap-3 rounded-xl bg-lime-300/5 p-4"><Radio class="mt-0.5 shrink-0 text-lime-300" :size="20" /><p class="text-xs leading-relaxed text-studio-400">Sobald der Kanal live ist, startet dein Worker den Videopuffer und sucht nach Audio-Highlights.</p></div>
      <div><label for="streamer-login" class="label">Twitch-Kanal</label><input id="streamer-login" v-model="form.login" class="input" placeholder="Kanalname oder twitch.tv/…" required :disabled="!!streamer" autocomplete="off" autofocus /></div>
      <div class="grid grid-cols-2 gap-4">
        <div><label for="streamer-quality" class="label">Aufnahmequalität</label><select id="streamer-quality" v-model="form.quality" class="input"><option value="best">Beste verfügbar</option><option value="1080p60,1080p,best">1080p</option><option value="720p60,720p,best">720p / 60 FPS</option><option value="720p,best">720p</option><option value="480p,worst">480p</option></select></div>
        <div><label for="streamer-priority" class="label">Priorität · 0–100</label><input id="streamer-priority" v-model.number="form.priority" class="input" type="number" min="0" max="100" required /></div>
      </div>
      <div class="grid grid-cols-2 gap-4">
        <div><label for="pre-roll" class="label">Vorlauf in Sekunden</label><input id="pre-roll" v-model.number="form.pre_roll" class="input" type="number" min="5" max="90" required /></div>
        <div><label for="post-roll" class="label">Nachlauf in Sekunden</label><input id="post-roll" v-model.number="form.post_roll" class="input" type="number" min="5" max="60" required /></div>
      </div>
      <div><label for="threshold" class="label flex justify-between"><span>Audio-Schwelle</span><span class="font-mono text-lime-300">+{{ form.threshold_db }} dB</span></label><input id="threshold" v-model.number="form.threshold_db" type="range" min="3" max="24" step="1" class="h-1.5 w-full accent-lime-300" /><div class="mt-2 flex justify-between text-[10px] text-studio-400"><span>Mehr Treffer</span><span>Stärkere Ausschläge</span></div></div>
      <label class="flex cursor-pointer items-center justify-between rounded-lg border divider p-3.5"><span class="text-sm">Automatisch Clips erstellen</span><input v-model="form.auto_clip" type="checkbox" class="size-4 rounded border-studio-700 bg-studio-950 text-lime-300 focus:ring-lime-300" /></label>
      <p v-if="error" role="alert" class="rounded-lg bg-red-400/10 p-3 text-sm text-red-300">{{ error }}</p>
      <div class="flex justify-end gap-2 border-t divider pt-5"><button type="button" class="btn btn-ghost" :disabled="busy" @click="emit('close')">Abbrechen</button><button class="btn btn-primary" :disabled="busy"><LoaderCircle v-if="busy" class="animate-spin" :size="16" /><Save v-else-if="streamer" :size="16" /><Plus v-else :size="16" />{{ streamer ? 'Speichern' : 'Zur Watchlist' }}</button></div>
    </form>
  </AppModal>
</template>
