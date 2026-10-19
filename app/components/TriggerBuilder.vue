<script setup lang="ts">
import { Plus, Trash2, Copy, Save, Play, LoaderCircle, AudioLines, MessageSquare, Workflow, ChevronDown, SlidersHorizontal, Clock3, Info, RotateCcw, Sparkles } from '@lucide/vue'
import type { Streamer, TriggerCondition, TriggerSignals, TriggerTest } from '~/types'
const props = defineProps<{ streamer: Streamer; streamers: Streamer[] }>()
const emit = defineEmits<{ saved: []; dirty: [value: boolean] }>()
const { toast } = useToast()
const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value))
const fingerprint = (value: unknown) => JSON.stringify(value, (_, child) => child && typeof child === 'object' && !Array.isArray(child) ? Object.fromEntries(Object.entries(child).sort(([a], [b]) => a.localeCompare(b))) : child)
const draft = reactive({ trigger_profile: clone(props.streamer.trigger_profile), auto_clip: props.streamer.auto_clip, pre_roll: props.streamer.pre_roll, post_roll: props.streamer.post_roll })
const saved = ref(fingerprint(draft))
const dirty = computed(() => saved.value !== fingerprint(draft))
const busy = ref('')
const error = ref('')
const copyOpen = ref(false)
const tab = ref<'rules' | 'semantic' | 'live'>('rules')
const targets = ref<string[]>([])
const test = ref<TriggerTest | null>(null)
const testedDraft = ref('')
const feed = useSignalFeed()
const realtime = useRealtimeState()
const { data: history, error: signalsError, refresh: refreshSignals } = await useFetch<TriggerSignals>(`/api/streamers/${props.streamer.id}/signals`, { key: `signals-${props.streamer.id}`, server: false })
watch(history, value => { if (value) feed.value[props.streamer.id] = mergeSignals(feed.value[props.streamer.id], value) }, { immediate: true })
const signals = computed(() => feed.value[props.streamer.id])
usePolling(refreshSignals, 30000, false)
watch(() => realtime.value.revision, () => refreshSignals())
watch(() => props.streamer.recording_id, () => refreshSignals())
const now = ref(Date.now())
let clock: ReturnType<typeof setInterval> | undefined
onMounted(() => { clock = setInterval(() => { now.value = Date.now() }, 1000) })
onBeforeUnmount(() => clearInterval(clock))
const signal = computed(() => signals.value?.recording?.trigger_state.signal)
const age = computed(() => signals.value?.recording?.trigger_state.updated_at ? Math.max(0, Math.floor((now.value - Date.parse(signals.value.recording.trigger_state.updated_at)) / 1000)) : null)
const fresh = computed(() => props.streamer.status === 'recording' && !signals.value?.recording?.ended_at && !!signal.value && age.value != null && age.value < 12)
const frames = computed(() => signals.value?.frames || [])
const summaries = computed(() => summarizeSignals(frames.value))
const measuredSeconds = computed(() => Math.round(frames.value.length / 4))
const range = ref(300)
const chartFrames = computed(() => {
  const cutoff = (frames.value.at(-1)?.time_ms || 0) - range.value * 1000
  // One point per second for charts; reference statistics retain every 250 ms measurement.
  return frames.value.filter(frame => frame.time_ms > cutoff).filter((_, i) => i % 4 === 0)
})
const chartTime = (time: number) => Date.parse(signals.value?.recording?.started_at || '') + time
const audioSeries = computed(() => [
  { name: 'Audiopegel', data: chartFrames.value.map(frame => ({ x: chartTime(frame.time_ms), y: frame.audio_level })) },
  { name: 'Trigger-Basispegel', data: chartFrames.value.map(frame => ({ x: chartTime(frame.time_ms), y: frame.baseline_db })) },
])
const chatSeries = computed(() => [
  { name: 'Nachrichten / 10s', data: chartFrames.value.map(frame => ({ x: chartTime(frame.time_ms), y: frame.chat_messages })) },
  { name: 'Chatter / 10s', data: chartFrames.value.map(frame => ({ x: chartTime(frame.time_ms), y: frame.chat_users })) },
])
const usesChat = computed(() => draft.trigger_profile.rules.some(r => r.enabled && r.conditions.some(c => c.metric.startsWith('chat_'))))
const otherStreamers = computed(() => props.streamers.filter(s => s.id !== props.streamer.id))
const liveLabel = computed(() => {
  if (!fresh.value) return 'Warte auf aktuelle Signale'
  if (!draft.auto_clip) return 'Beobachten · Auto-Clips aus'
  if ((signal.value?.calibrated_seconds || 0) < draft.trigger_profile.warmup_seconds) return 'Kanal wird kalibriert'
  if (usesChat.value && signal.value?.chat_users == null) return 'Chat noch nicht verfügbar'
  if ((signals.value?.hourly_count || 0) >= draft.trigger_profile.hourly_limit) return 'Stundenlimit erreicht'
  if (signals.value?.last_trigger_at && now.value - Date.parse(signals.value.last_trigger_at) < draft.trigger_profile.cooldown_seconds * 1000) return 'Clip-Abstand aktiv'
  return 'Bereit · warte auf passende Signale'
})
watch(dirty, value => emit('dirty', value), { immediate: true })
watch(() => props.streamer, streamer => {
  if (dirty.value || busy.value) return
  Object.assign(draft, { trigger_profile: clone(streamer.trigger_profile), auto_clip: streamer.auto_clip, pre_roll: streamer.pre_roll, post_roll: streamer.post_roll })
  saved.value = fingerprint(draft)
})
function beforeUnload(event: BeforeUnloadEvent) { if (dirty.value) { event.preventDefault(); event.returnValue = '' } }
onMounted(() => window.addEventListener('beforeunload', beforeUnload))
onBeforeUnmount(() => window.removeEventListener('beforeunload', beforeUnload))
function preset(kind: 'combined' | 'audio' | 'chat') { draft.trigger_profile = { ...triggerPreset(kind), semantic: draft.trigger_profile.semantic }; error.value = '' }
function reset() {
  Object.assign(draft, { trigger_profile: clone(props.streamer.trigger_profile), auto_clip: props.streamer.auto_clip, pre_roll: props.streamer.pre_roll, post_roll: props.streamer.post_roll })
  saved.value = fingerprint(draft); error.value = ''
}
function addRule() { draft.trigger_profile.rules.push({ name: `Regel ${draft.trigger_profile.rules.length + 1}`, enabled: true, hold_seconds: 2, conditions: [{ metric: 'audio_rise', op: 'gte', value: 12 }] }) }
function valueFor(condition: TriggerCondition) { return fresh.value ? signal.value?.[condition.metric] : null }
async function save() {
  busy.value = 'save'; error.value = ''
  const body = clone(draft)
  try { await api(`/streamers/${props.streamer.id}`, { method: 'PATCH', body }); saved.value = fingerprint(body); emit('saved'); toast('Automation gespeichert. Gilt ab dem nächsten Videosegment.'); return true }
  catch (e) { error.value = errorMessage(e); return false }
  finally { busy.value = '' }
}
async function runTest() {
  busy.value = 'test'; error.value = ''
  const profile = clone(draft.trigger_profile)
  try { test.value = await api<TriggerTest>(`/streamers/${props.streamer.id}/triggers/test`, { method: 'POST', body: profile }); testedDraft.value = fingerprint(profile) }
  catch (e) { error.value = errorMessage(e) }
  finally { busy.value = '' }
}
async function copy() {
  if (dirty.value && !await save()) { copyOpen.value = false; return }
  busy.value = 'copy'; error.value = ''
  try { await api(`/streamers/${props.streamer.id}/triggers/copy`, { method: 'POST', body: { target_ids: targets.value } }); toast(`Automation auf ${targets.value.length} Streamer kopiert.`); copyOpen.value = false; targets.value = []; emit('saved') }
  catch (e) { error.value = errorMessage(e); copyOpen.value = false }
  finally { busy.value = '' }
}
</script>
<template>
  <form id="automation-form" class="pb-24" @submit.prevent="save">
    <div class="panel mb-6 flex flex-wrap items-center justify-between gap-4 px-5 py-4">
      <div class="flex items-center gap-3"><span class="flex size-10 items-center justify-center rounded-xl bg-lavender/10 text-lavender"><Workflow :size="19" /></span><div><h2 class="text-sm font-semibold">@{{ streamer.login }}</h2><p class="mt-1 text-xs text-studio-400">{{ draft.trigger_profile.rules.filter(r => r.enabled).length }} Signalregeln<span v-if="draft.trigger_profile.semantic.enabled"> · Story/Video aktiv</span> · max. {{ draft.trigger_profile.hourly_limit }} Clips/h</p></div></div>
      <div class="flex flex-wrap items-center gap-2"><label class="mr-3 flex cursor-pointer items-center gap-2 text-xs"><input v-model="draft.auto_clip" type="checkbox" class="checkbox" />Auto-Clips</label><button type="button" class="btn btn-secondary" :disabled="!!busy || !otherStreamers.length" @click="copyOpen = true"><Copy :size="14" />Kopieren</button></div>
    </div>
    <nav class="tab-bar mb-6" aria-label="Automationsbereiche"><button v-for="item in [{ id: 'rules' as const, label: 'Audio & Chat', icon: AudioLines }, { id: 'semantic' as const, label: 'Story & Video', icon: Sparkles }, { id: 'live' as const, label: 'Live-Analyse', icon: SlidersHorizontal }]" :key="item.id" type="button" class="tab" :class="{ 'tab-active': tab === item.id }" :aria-pressed="tab === item.id" @click="tab = item.id"><component :is="item.icon" :size="16" />{{ item.label }}<span v-if="item.id === 'semantic'" class="rounded bg-lavender/10 px-1.5 py-0.5 text-[10px] text-lavender">KI</span></button></nav>
    <fieldset v-show="tab === 'semantic'" :disabled="!!busy"><SemanticEditor v-model="draft.trigger_profile.semantic" :streamer-id="streamer.id" /></fieldset>
    <div v-if="tab === 'live'">
    <div class="mb-3 flex flex-wrap items-center justify-between gap-3"><div class="flex items-center gap-2"><h3 class="text-sm font-semibold">Dein Kanal in Zahlen</h3><span class="text-[10px] text-studio-400">{{ fresh ? `vor ${age}s` : frames.length ? 'Letzte Aufnahme' : 'Warte auf Messwerte' }}</span></div><select v-model.number="range" aria-label="Zeitraum der Diagramme" class="input w-auto py-1.5 text-xs"><option :value="60">Letzte Minute</option><option :value="300">Letzte 5 Minuten</option></select></div>
    <div class="mb-6 grid gap-4 lg:grid-cols-2">
      <section class="panel min-w-0 overflow-hidden"><div class="flex items-start justify-between px-5 pt-5"><div><h4 class="flex items-center gap-2 text-xs text-studio-400"><AudioLines :size="14" class="text-accent" />Audiopegel</h4><p class="mt-2 text-2xl font-semibold tabular-nums">{{ signalNumber(fresh ? signal?.audio_level : null) }} <span class="text-xs font-normal text-studio-400">dBFS</span></p></div><div class="text-right"><p class="text-[10px] text-studio-400">Ø letzte 5 Min.</p><p class="mt-1 text-sm font-medium tabular-nums text-lavender">{{ signalNumber(summaries.audio_level?.mean) }} dBFS</p></div></div><StudioChart :series="audioSeries" :height="180" :colors="['#6b9fff', '#b8a1ff']" :min="-96" :max="0" unit="dBFS" label="Audiopegel und Trigger-Basispegel im Zeitverlauf" /><div class="flex flex-wrap gap-4 border-t divider px-5 py-3 text-[10px] text-studio-400"><span class="chart-legend before:bg-accent">Audiopegel</span><span class="chart-legend before:bg-lavender">Trigger-Basispegel · aktive Audiopegel</span></div></section>
      <section class="panel min-w-0 overflow-hidden"><div class="flex items-start justify-between px-5 pt-5"><div><h4 class="flex items-center gap-2 text-xs text-studio-400"><MessageSquare :size="14" class="text-mint" />Chat-Aktivität</h4><p class="mt-2 text-2xl font-semibold tabular-nums">{{ signalNumber(fresh ? signal?.chat_messages : null) }} <span class="text-xs font-normal text-studio-400">Nachrichten / 10s</span></p></div><div class="text-right"><p class="text-[10px] text-studio-400">Ø letzte 5 Min.</p><p class="mt-1 text-sm font-medium tabular-nums text-lavender">{{ signalNumber(summaries.chat_messages?.mean) }} / 10s</p></div></div><StudioChart :series="chatSeries" :height="180" :colors="['#38d6bd', '#6b9fff']" :min="0" :mean="summaries.chat_messages?.mean" label="Chatnachrichten und verschiedene Chatter im Zeitverlauf" /><div class="flex flex-wrap gap-4 border-t divider px-5 py-3 text-[10px] text-studio-400"><span class="chart-legend before:bg-mint">Nachrichten</span><span class="chart-legend before:bg-accent">Verschiedene Chatter</span><span class="chart-legend before:bg-lavender">Ø Nachrichten</span></div></section>
    </div>
    </div>
    <p v-if="error" role="alert" class="mb-5 rounded-lg border border-red-400/20 bg-red-400/5 p-4 text-sm text-red-300">{{ error }}</p>
    <div v-show="tab === 'rules'" class="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_300px]">
      <fieldset class="min-w-0 space-y-5" :disabled="!!busy">
        <div class="panel p-5"><div class="mb-3 flex items-center gap-2 text-xs font-medium"><Sparkles :size="15" class="text-lavender" />Mit einer Vorlage starten</div><div class="flex flex-wrap gap-2"><button type="button" class="btn btn-secondary text-xs" @click="preset('combined')"><AudioLines :size="14" class="text-accent" />Audio + Chat</button><button type="button" class="btn btn-secondary text-xs" @click="preset('audio')"><AudioLines :size="14" class="text-lavender" />Nur Audio · streng</button><button type="button" class="btn btn-secondary text-xs" @click="preset('chat')"><MessageSquare :size="14" class="text-mint" />Chat-Reaktion</button></div><p class="hint">Wähle eine Vorlage und passe ihre Schwellen an die gemessenen Kanalwerte an.</p></div>
        <div class="flex items-center justify-between px-1"><h3 class="text-sm font-medium">Wenn diese Bedingungen zutreffen …</h3><span class="text-xs text-studio-400">{{ draft.trigger_profile.rules.length }} / 8 Regeln</span></div>
        <template v-for="(rule, ri) in draft.trigger_profile.rules" :key="ri">
          <div v-if="ri" class="flex items-center gap-3 px-5"><span class="h-px flex-1 bg-studio-700" /><span class="text-[10px] font-semibold tracking-wider text-studio-400">ODER</span><span class="h-px flex-1 bg-studio-700" /></div>
          <section class="panel overflow-hidden" :class="{ 'opacity-50': !rule.enabled }">
            <div class="flex items-center gap-3 border-b divider bg-accent/[.035] px-4 py-3"><span class="flex size-6 shrink-0 items-center justify-center rounded-md bg-accent/15 text-xs text-accent">{{ ri + 1 }}</span><input v-model="rule.name" :aria-label="`Name für Regel ${ri + 1}`" class="min-w-0 flex-1 rounded border-0 bg-transparent p-1 text-sm font-medium focus:ring-1 focus:ring-accent" maxlength="60" required /><label class="flex items-center gap-2 text-xs text-studio-400"><input v-model="rule.enabled" type="checkbox" class="checkbox" :aria-label="`Regel ${ri + 1} aktiv`" />Aktiv</label><button type="button" class="icon-btn" :disabled="draft.trigger_profile.rules.length === 1" :aria-label="`Regel ${ri + 1} entfernen`" @click="draft.trigger_profile.rules.splice(ri, 1)"><Trash2 :size="14" /></button></div>
            <div class="space-y-3 p-4">
              <div v-for="(condition, ci) in rule.conditions" :key="ci">
                <div v-if="ci" class="mb-2 ml-2 border-l divider pl-3 text-[10px] font-semibold text-studio-400">UND</div>
                <TriggerConditionEditor v-model="rule.conditions[ci]!" :id="`${ri + 1}.${ci + 1}`" :current="valueFor(condition)" :summary="summaries[condition.metric]" :removable="rule.conditions.length > 1" @remove="rule.conditions.splice(ci, 1)" />
              </div>
              <button type="button" class="btn btn-ghost px-1 py-1.5 text-xs" :disabled="rule.conditions.length >= 6" @click="rule.conditions.push({ metric: 'chat_users', op: 'gte', value: 3 })"><Plus :size="14" />UND-Bedingung</button>
            </div>
            <div class="flex flex-wrap items-center gap-2 border-t divider px-4 py-3 text-xs text-studio-400"><Clock3 :size="14" /><label :for="`hold-${ri}`">Alle Bedingungen für mindestens</label><input :id="`hold-${ri}`" v-model.number="rule.hold_seconds" class="input w-20 py-1.5 text-xs" type="number" min="0.5" max="10" step="0.25" required /><span>Sekunden</span></div>
          </section>
        </template>
        <button type="button" class="btn btn-secondary w-full border-dashed" :disabled="draft.trigger_profile.rules.length >= 8" @click="addRule"><Plus :size="15" />ODER-Regel hinzufügen</button>
        <div class="flex justify-center text-studio-400"><ChevronDown :size="18" /></div>
        <section class="panel"><div class="border-b divider px-5 py-4"><h3 class="text-sm font-medium">… dann einen Clip erstellen</h3><p class="hint">Mindestdauer filtert kurze Spitzen. Limits gelten für alle Regeln dieses Kanals zusammen.</p></div><div class="grid gap-4 p-5 sm:grid-cols-2">
          <div><label for="trigger-pre" class="label">Vorlauf · Sekunden</label><input id="trigger-pre" v-model.number="draft.pre_roll" type="number" class="input" min="5" max="90" required /></div>
          <div><label for="trigger-post" class="label">Nachlauf · Sekunden</label><input id="trigger-post" v-model.number="draft.post_roll" type="number" class="input" min="5" max="60" required /></div>
          <div><label for="trigger-cooldown" class="label">Clip-Abstand · Sekunden</label><input id="trigger-cooldown" v-model.number="draft.trigger_profile.cooldown_seconds" type="number" class="input" min="30" max="1800" required /></div>
          <div><label for="trigger-limit" class="label">Max. Clips pro Stunde</label><input id="trigger-limit" v-model.number="draft.trigger_profile.hourly_limit" type="number" class="input" min="1" max="60" required /></div>
          <div><label for="trigger-warmup" class="label">Kalibrierung · Sekunden</label><input id="trigger-warmup" v-model.number="draft.trigger_profile.warmup_seconds" type="number" class="input" min="15" max="180" required /></div>
          <p class="hint self-center">Vorlauf + Nachlauf: maximal 120s. Ein durchgehend erfüllter Trigger wird erst nach einer Unterbrechung erneut scharf.</p>
        </div></section>
      </fieldset>
      <aside class="space-y-5 xl:sticky xl:top-24">
        <section class="panel p-5"><h3 class="flex items-center gap-2 text-sm font-medium"><SlidersHorizontal :size="15" class="text-lavender" />Schwellen richtig wählen</h3><p class="hint">Orientiere dich am Alltag deines Kanals. Eine Schwelle über dem typischen Bereich reagiert nur auf stärkere Ausschläge.</p><div class="mt-4 space-y-3 text-xs"><p class="chart-legend before:bg-lavender">Violett: Durchschnitt & typischer Bereich</p><p class="chart-legend before:bg-amber-300">Gelb: deine eingestellte Schwelle</p><p class="chart-legend before:bg-accent">Blau: aktueller Messwert</p></div><p class="mt-4 border-t divider pt-4 text-[11px] leading-relaxed text-studio-400">{{ measuredSeconds }}s Messdaten von maximal 5 Minuten. Der typische Bereich umfasst die mittleren 80 % der Werte. Fehlende Chatwerte werden ausgelassen; echte Nullen zählen mit.</p><p v-if="!fresh && frames.length" class="mt-3 text-[11px] text-amber-200">Historische Referenzwerte · momentan keine frischen Messungen.</p><p v-if="signalsError" class="mt-3 text-xs text-red-300">Messhistorie konnte nicht geladen werden.</p></section>
        <section class="panel p-5"><p class="mb-4 flex items-start gap-2 text-xs leading-relaxed"><span class="mt-1 size-1.5 shrink-0 rounded-full" :class="fresh ? 'bg-mint' : 'bg-studio-400'" />{{ liveLabel }}</p><div class="flex items-baseline justify-between"><span class="text-xs text-studio-400">Auto-Clips · letzte Stunde</span><span class="text-lg font-semibold tabular-nums">{{ signals?.hourly_count ?? 0 }} <span class="text-xs font-normal text-studio-400">/ {{ draft.trigger_profile.hourly_limit }}</span></span></div><div class="mt-3 h-1.5 overflow-hidden rounded-full bg-studio-800"><div class="h-full rounded-full bg-lavender transition-all" :style="{ width: `${Math.min(100, (signals?.hourly_count || 0) / draft.trigger_profile.hourly_limit * 100)}%` }" /></div><p v-if="dirty" class="hint">Vorschau deines Entwurfs. Live gelten die gespeicherten Regeln.</p></section>
         <section class="panel p-5"><h3 class="text-sm font-medium">Audio & Chat testen</h3><p class="hint mb-4">Regeln auf bis zu 15 Minuten gemessene Signale anwenden. Ohne Clip-Erstellung und ohne KI-Auswertung.</p><button type="button" class="btn btn-secondary w-full" :disabled="!!busy" @click="runTest"><LoaderCircle v-if="busy === 'test'" class="animate-spin" :size="14" /><Play v-else :size="14" />Probelauf starten</button>
          <div v-if="test" class="mt-4 space-y-3" aria-live="polite"><p v-if="testedDraft !== fingerprint(draft.trigger_profile)" class="text-xs text-amber-200">Entwurf geändert – bitte erneut testen.</p><div class="flex items-baseline gap-2"><span class="text-2xl font-semibold">{{ test.hits.length }}</span><span class="text-xs text-studio-400">Treffer in {{ Math.round(test.seconds) }}s Daten</span></div><p v-if="!test.sample_count" class="hint">Noch keine Signale. Starte eine Aufnahme und warte auf die ersten Segmente.</p><p v-if="usesChat && !test.chat_available" class="hint">In diesem Zeitraum fehlen Chatdaten. Chat-Regeln können daher nicht auslösen.</p><div class="max-h-64 space-y-2 overflow-y-auto"><details v-for="(hit, i) in test.hits" :key="i" class="rounded-lg border divider p-3"><summary class="cursor-pointer text-xs">{{ duration(hit.time_ms / 1000) }} · {{ hit.rule }}</summary><p class="hint">{{ hit.reason }}</p></details></div><p class="text-[10px] leading-relaxed text-studio-400">Zeitpunkte ab Aufnahmestart. Der Probelauf startet mit leerem Clip-Limit; frühere Live-Clips werden nicht mitgezählt.</p></div>
        </section>
        <p class="flex gap-2 px-1 text-[11px] leading-relaxed text-studio-400"><Info :size="14" class="mt-0.5 shrink-0" />Audio enthält Stimme, Spiel und Musik. Die Regeln messen Reaktionen, verstehen aber keinen Spielinhalt. Die Chat-Bedingungen helfen, reine Soundeffekte auszufiltern.</p>
      </aside>
    </div>
  </form>
  <Teleport to="body"><Transition name="savebar"><div v-if="dirty" class="fixed inset-x-4 bottom-5 z-40 lg:left-[248px] lg:right-6" role="region" aria-label="Ungespeicherte Änderungen"><div class="mx-auto max-w-3xl rounded-2xl border border-accent/30 bg-studio-850/95 p-3 shadow-[0_12px_60px_#0009] backdrop-blur-xl sm:px-5"><div class="flex flex-wrap items-center justify-between gap-3"><div class="flex items-center gap-3"><span class="size-2 rounded-full bg-amber-300 shadow-[0_0_12px_#fcd34d55]" /><div><p class="text-sm font-medium">Ungespeicherte Änderungen</p><p class="mt-0.5 text-[11px] text-studio-400">Automation für @{{ streamer.login }}</p></div></div><div class="flex items-center gap-2"><button type="button" class="btn btn-ghost text-xs" :disabled="!!busy" @click="reset"><RotateCcw :size="14" />Verwerfen</button><button type="submit" form="automation-form" class="btn btn-primary text-xs" :disabled="!!busy"><LoaderCircle v-if="busy === 'save'" :size="14" class="animate-spin" /><Save v-else :size="14" />Änderungen speichern</button></div></div><p v-if="error" role="alert" class="mt-3 text-xs text-red-300">{{ error }}</p></div></div></Transition></Teleport>
  <AppModal :open="copyOpen" title="Automation kopieren" @close="!busy && (copyOpen = false)"><p class="text-sm leading-relaxed text-studio-400">Regeln, Limits und Clip-Länge von <strong class="text-zinc-200">@{{ streamer.login }}</strong> auf andere Kanäle übernehmen. Deren bisherige Regeln werden ersetzt; ihr Auto-Clip-Schalter bleibt bestehen.</p><p v-if="dirty" class="hint mt-3">Dein Entwurf wird zuerst für @{{ streamer.login }} gespeichert.</p><div class="my-5 max-h-72 space-y-2 overflow-y-auto"><label v-for="target in otherStreamers" :key="target.id" class="flex cursor-pointer items-center gap-3 rounded-lg border divider p-3 text-sm"><input v-model="targets" type="checkbox" :value="target.id" class="checkbox" :disabled="!!busy" />{{ target.login }}<span class="ml-auto text-xs text-studio-400">{{ target.trigger_profile.rules.length }} Regeln</span></label></div><div class="flex justify-end gap-2"><button class="btn btn-ghost" :disabled="!!busy" @click="copyOpen = false">Abbrechen</button><button class="btn btn-primary" :disabled="!!busy || !targets.length" @click="copy"><Copy :size="14" />{{ dirty ? 'Speichern & kopieren' : 'Kopieren' }} ({{ targets.length }})</button></div></AppModal>
</template>
