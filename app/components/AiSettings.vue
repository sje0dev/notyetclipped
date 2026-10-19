<script setup lang="ts">
import { ExternalLink, LogOut, Save, LoaderCircle, RefreshCw, Check, BrainCircuit } from '@lucide/vue'
import type { AiSettings, AiStatus } from '~/types'
const { data, error, refresh } = await useFetch<AiStatus>('/api/ai', { key: 'ai', server: false })
usePolling(refresh, 15000, false)
const form = reactive<AiSettings>({ enabled: false, model: '', hourly_limit: 20, daily_limit: 100, cpu_threads: 2, frame_count: 24, paced: true })
const saved = ref('')
const dirty = computed(() => JSON.stringify(form) !== saved.value)
watch(() => data.value?.settings, value => { if (value && (!saved.value || !dirty.value)) { Object.assign(form, value); saved.value = JSON.stringify(form) } }, { immediate: true })
const models = ref<{ id: string; name: string }[]>([])
const busy = ref('')
const failure = ref('')
const { toast } = useToast()
const account = computed(() => data.value?.auth.accounts.find(a => a.id === data.value?.auth.active))
async function loadModels() {
  try { models.value = await api('/ai/models') }
  catch (e) { failure.value = errorMessage(e) }
}
watch(() => [data.value?.auth.active, data.value?.auth.plan_allowed], () => { if (data.value?.auth.plan_allowed) void loadModels() }, { immediate: true })
async function login(accountId?: string) {
  busy.value = 'login'; failure.value = ''
  try { const result = await api<{ url: string }>('/ai/login', { method: 'POST', body: { account_id: accountId || null } }); window.location.assign(result.url) }
  catch (e) { failure.value = errorMessage(e); busy.value = '' }
}
async function logout() {
  busy.value = 'logout'; failure.value = ''
  try { const result = await api<{ revoked: boolean }>('/ai/logout', { method: 'POST' }); form.enabled = false; saved.value = ''; await refresh(); models.value = []; toast(result.revoked ? 'ChatGPT getrennt. KI-Analyse pausiert.' : 'Lokal getrennt. Bitte die App zusätzlich in den ChatGPT-Einstellungen trennen.', !result.revoked) }
  catch (e) { failure.value = errorMessage(e) }
  finally { busy.value = '' }
}
async function save() {
  busy.value = 'save'; failure.value = ''
  try { await api('/ai/settings', { method: 'PUT', body: form }); saved.value = JSON.stringify(form); await refresh(); toast('KI-Einstellungen gespeichert.') }
  catch (e) { failure.value = errorMessage(e) }
  finally { busy.value = '' }
}
</script>
<template>
  <ConnectionError v-if="error" @retry="refresh" />
  <div v-else class="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
    <div class="space-y-6">
      <section class="panel">
        <div class="section-heading"><BrainCircuit :size="20" class="text-accent" /><div><h2>ChatGPT verbinden</h2><p>Geschichten und Videomomente mit deinem Abo auswerten.</p></div><span v-if="data?.auth.connected" class="badge ml-auto border-mint/20 bg-mint/10 text-mint"><Check :size="12" />Verbunden</span></div>
        <div class="space-y-5 p-6">
          <p class="text-sm leading-relaxed text-studio-400">Whisper transkribiert Sprache lokal. ChatGPT erhält das Transkript und bis zu {{ form.frame_count }} Videobilder pro Auswertung. Alle Text-Trigger eines Kanals werden gemeinsam in einer Anfrage geprüft.</p>
          <div v-if="account" class="flex flex-wrap items-center justify-between gap-3 rounded-lg border divider p-4"><div><p class="text-sm">{{ account.email }}</p><p class="mt-1 text-xs text-studio-400">{{ data?.auth.plan_allowed ? 'Abo-Nutzung freigegeben' : 'Abo-Nutzung noch nicht freigegeben' }}</p></div><button v-if="data?.auth.connected" type="button" class="btn btn-secondary" :disabled="!!busy" @click="logout"><LogOut :size="14" />Trennen</button></div>
          <div class="flex flex-wrap items-center gap-3">
            <button v-if="!data?.auth.connected || !data.auth.plan_allowed" type="button" class="btn border border-white/20 bg-white px-5 text-black hover:bg-zinc-200" :disabled="!!busy" @click="login(account?.id)"><LoaderCircle v-if="busy === 'login'" :size="16" class="animate-spin" />Continue with ChatGPT</button>
            <details v-if="data?.auth.accounts.length" class="text-xs text-studio-400"><summary class="cursor-pointer py-2">Anderes Konto verwenden</summary><div class="mt-2 flex flex-wrap gap-2"><button v-for="a in data.auth.accounts" :key="a.id" class="btn btn-secondary text-xs" :disabled="!!busy" @click="login(a.id)">{{ a.email }} · {{ a.id.slice(-6) }}</button><button class="btn btn-secondary text-xs" :disabled="!!busy" @click="login()">Neues Konto</button></div></details>
            <a href="https://chatgpt.com/settings/usage" target="_blank" rel="noopener noreferrer" class="btn btn-ghost text-xs">Abo-Limits verwalten<ExternalLink :size="13" /></a>
          </div>
          <p class="hint">Beim ersten Login legst du in ChatGPT fest, welchen Anteil deines Kontingents NotYetClipped nutzen darf. Die Verfügbarkeit hängt von deinem Konto und Workspace ab.</p>
        </div>
      </section>
      <form class="panel" @submit.prevent="save">
        <div class="section-heading"><div><h2>Analyse & Anfrage-Limits</h2><p>Gemeinsam für alle Streamer. Eine Auswertung läuft zur selben Zeit.</p></div></div>
        <fieldset class="space-y-6 p-6" :disabled="!!busy">
          <label class="flex items-center gap-3 text-sm"><input v-model="form.enabled" type="checkbox" class="checkbox" :disabled="!data?.auth.plan_allowed" />Inhaltliche Analyse aktivieren</label>
          <div><div class="mb-2 flex items-center justify-between"><label for="ai-model" class="label mb-0">Modell aus deinem ChatGPT-Konto</label><button type="button" class="icon-btn" :disabled="!data?.auth.plan_allowed" aria-label="Modelle aktualisieren" @click="loadModels"><RefreshCw :size="14" /></button></div><select id="ai-model" v-model="form.model" class="input" :required="form.enabled"><option value="">Modell wählen …</option><option v-if="form.model && !models.some(m => m.id === form.model)" :value="form.model">{{ form.model }}</option><option v-for="m in models" :key="m.id" :value="m.id">{{ m.name }}</option></select><p class="hint">Wähle ein Modell mit Bildverständnis. Ein größeres Modell verbraucht in der Regel mehr Kontingent.</p></div>
          <div class="grid gap-5 sm:grid-cols-2"><div><label for="ai-hour" class="label">Max. Anfragen · letzte Stunde</label><input id="ai-hour" v-model.number="form.hourly_limit" type="number" min="1" max="120" required class="input" /></div><div><label for="ai-day" class="label">Max. Anfragen · letzte 24 Stunden</label><input id="ai-day" v-model.number="form.daily_limit" type="number" min="1" max="2000" required class="input" /></div></div>
          <label class="flex items-start gap-3 text-sm"><input v-model="form.paced" type="checkbox" class="checkbox mt-1" /><span>Anfragen über die Stunde verteilen<p class="hint">Verhindert, dass das Stundenkontingent sofort aufgebraucht wird. Kanäle wechseln sich nach letzter Auswertung ab; volle Clip-Budgets werden vor der Transkription übersprungen.</p></span></label>
          <div class="grid gap-5 sm:grid-cols-2"><div><label for="ai-threads" class="label">Whisper CPU-Threads</label><select id="ai-threads" v-model.number="form.cpu_threads" class="input"><option :value="1">1 · besonders sparsam</option><option :value="2">2 · ausgewogen</option><option :value="4">4 · schneller, mehr Last</option></select></div><div><label for="ai-frames" class="label">Bilder pro Auswertung</label><select id="ai-frames" v-model.number="form.frame_count" class="input"><option :value="12">Bis zu 12 · weniger Kontingent</option><option :value="24">Bis zu 24 · dichterer Bildkontext</option></select></div></div>
          <p class="hint">Es läuft nur eine lokale KI-Vorbereitung gleichzeitig. Video-Keyframes und begrenzte Decoder-Threads sparen CPU; Aufnahmen speichern weiterhin die gewählte Streamqualität.</p>
          <p v-if="data?.paused_reason" role="alert" class="rounded-lg border border-amber-300/20 bg-amber-300/5 p-4 text-sm text-amber-200">Analyse pausiert: {{ data.paused_reason }}<span class="mt-2 block text-xs">Prüfe Konto, Limits oder Modell. Speichern hebt die Pause auf.</span></p>
          <div class="flex flex-wrap items-center justify-between gap-4 border-t divider pt-5"><p class="text-xs text-studio-400">{{ data?.transcription_ready ? 'Whisper-Modell lokal verfügbar' : 'Whisper-Modell noch nicht verfügbar' }}</p><button class="btn btn-primary" :disabled="!!busy || (!dirty && !data?.paused_reason)"><LoaderCircle v-if="busy === 'save'" :size="15" class="animate-spin" /><Save v-else :size="15" />Speichern</button></div>
        </fieldset>
      </form>
      <p v-if="failure" role="alert" class="rounded-lg border border-red-400/20 bg-red-400/5 p-4 text-sm text-red-300">{{ failure }}</p>
    </div>
    <aside class="space-y-5">
      <section v-if="data?.scheduler" class="panel p-5"><h2 class="text-sm font-semibold">Multi-Streamer-Verarbeitung</h2><p class="hint">{{ data.scheduler.active_streamers }} aktive KI-Kanäle</p><p v-if="data.scheduler.budget_interval_seconds" class="mt-3 text-sm">Budget: Ø mindestens {{ Math.ceil(data.scheduler.budget_interval_seconds / 60) }} Min. je Kanal</p><p class="hint">Bei gleichmäßiger Verteilung deines Stundenlimits. Intervall, Rechenzeit und Tageslimit können den Abstand verlängern. Ein Rückblick von 2–3 Minuten deckt größere Abstände nicht vollständig ab.</p><div class="mt-4 border-t divider pt-3 text-xs text-studio-400">Ø Vorbereitung: {{ data.scheduler.preparation_seconds == null ? '—' : `${Math.round(data.scheduler.preparation_seconds)}s` }} · KI: {{ data.scheduler.inference_seconds == null ? '—' : `${Math.round(data.scheduler.inference_seconds)}s` }}</div></section>
      <section class="panel p-5"><h2 class="mb-5 text-sm font-semibold">Anfragen dieses Studios</h2><div v-for="period in [{ label: 'Letzte Stunde', count: data?.requests_hour || 0, limit: data?.settings.hourly_limit || 20 }, { label: 'Letzte 24 Stunden', count: data?.requests_day || 0, limit: data?.settings.daily_limit || 100 }]" :key="period.label" class="mb-5"><div class="mb-2 flex justify-between text-xs"><span class="text-studio-400">{{ period.label }}</span><span>{{ period.count }} / {{ period.limit }}</span></div><div class="h-1.5 overflow-hidden rounded-full bg-studio-800"><div class="h-full bg-accent" :style="{ width: `${Math.min(100, period.count / period.limit * 100)}%` }" /></div></div><p class="hint">Das sind lokale Anfragezähler, keine Anzeige deines verbleibenden Wochenkontingents.</p></section>
      <section class="panel p-5"><h2 class="text-sm font-semibold">Welche Momente?</h2><p class="hint">Aktiviere Story- und Video-Trigger für die gewünschten Streamer und beschreibe dort deinen Clip-Geschmack.</p><NuxtLink to="/streamers" class="btn btn-secondary mt-4 w-full">Streamer auswählen</NuxtLink></section>
    </aside>
  </div>
</template>
