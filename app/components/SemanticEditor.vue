<script setup lang="ts">
import { BookOpen, ScanEye, Sparkles, ArrowUpRight } from '@lucide/vue'
import type { SemanticTrigger, AiStatus } from '~/types'
const model = defineModel<SemanticTrigger>({ required: true })
const props = defineProps<{ streamerId: string }>()
const { data, error, refresh } = await useFetch<AiStatus>('/api/ai', { key: 'ai', server: false })
usePolling(refresh, 15000, false)
const runs = computed(() => data.value?.runs.filter(r => r.streamer_id === props.streamerId) || [])
function example() { model.value.guidance = 'Spontane Comedy, absurde Monologe, gespielte Empörung, schlagfertige Gespräche und überraschende Enthüllungen mit anderen Streamern. Auch lustige Gesichtsausdrücke oder körperliche Aktionen. Den Satz oder die Situation vor der Pointe mitnehmen. Gewöhnliches Bauen, Farming und reine Lautstärke auslassen.' }
</script>
<template>
  <div class="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
    <section class="panel">
      <div class="section-heading"><Sparkles :size="20" class="text-lavender" /><div><h2>Inhaltliche Momente</h2><p>Geschichten verstehen und sichtbare Ereignisse erkennen.</p></div><label class="ml-auto flex items-center gap-2 text-sm"><input v-model="model.enabled" type="checkbox" class="checkbox" />Aktiv</label></div>
      <div class="space-y-6 p-6">
        <div class="grid gap-4 sm:grid-cols-2">
          <label class="flex cursor-pointer items-start gap-3 rounded-xl border p-4 transition" :class="model.stories ? 'border-accent/40 bg-accent/5' : 'divider'"><input v-model="model.stories" type="checkbox" class="checkbox mt-1" /><div><BookOpen :size="19" class="mb-3 text-accent" /><span class="text-sm font-medium">Stories & Gespräche</span><p class="hint">Anekdoten, Pointen, Wortwechsel und spontane Comedy. Mit dem Kontext davor.</p></div></label>
          <label class="flex cursor-pointer items-start gap-3 rounded-xl border p-4 transition" :class="model.visual ? 'border-lavender/40 bg-lavender/5' : 'divider'"><input v-model="model.visual" type="checkbox" class="checkbox mt-1" /><div><ScanEye :size="19" class="mb-3 text-lavender" /><span class="text-sm font-medium">Video-Momente</span><p class="hint">Sichtbare Reaktionen, Fails, überraschende Situationen und Enthüllungen.</p></div></label>
        </div>
        <div><div class="mb-2 flex flex-wrap items-center justify-between gap-2"><label for="semantic-guidance" class="label mb-0">Was macht für dich einen guten Clip aus?</label><button type="button" class="text-xs text-accent hover:underline" @click="example">Comedy-Vorlage einsetzen</button></div><textarea id="semantic-guidance" v-model="model.guidance" class="input min-h-32 resize-y leading-relaxed" maxlength="1000" placeholder="Zum Beispiel: lustige Erlebnisse, trockene Konter und unerwartete Reaktionen. Bitte die Vorgeschichte mitnehmen." /><p class="hint">{{ model.guidance.length }} / 1000 Zeichen · Gilt nur für diesen Streamer.</p></div>
        <div class="grid gap-5 sm:grid-cols-2"><div><label for="semantic-interval" class="label">Frühestens erneut analysieren nach</label><select id="semantic-interval" v-model.number="model.interval_seconds" class="input"><option :value="30">30 Sekunden</option><option :value="60">60 Sekunden</option><option :value="90">90 Sekunden</option><option :value="120">2 Minuten</option><option :value="300">5 Minuten</option></select><p class="hint">Jeweils bis zu 2 Minuten Rückblick. Kürzere Intervalle überlappen stärker und verbrauchen mehr Kontingent.</p></div><div><label for="semantic-score" class="label">Mindestbewertung · {{ Math.round(model.min_confidence * 100) }} / 100</label><input id="semantic-score" v-model.number="model.min_confidence" type="range" min="0.5" max="1" step="0.05" class="mt-4 w-full accent-accent" /><p class="hint">Subjektive KI-Bewertung, keine garantierte Trefferwahrscheinlichkeit.</p></div></div>
        <p class="border-t divider pt-5 text-sm leading-relaxed text-studio-400">Der Ausschnitt wird anhand des Inhalts gewählt: 5–120 Sekunden mit Anfang und Abschluss. Die gemeinsamen Clip-Limits gelten weiterhin. Längere Geschichten können außerhalb des Analysefensters liegen.</p>
      </div>
    </section>
    <aside class="space-y-5">
      <section class="panel p-5"><h3 class="text-sm font-semibold">Analyse-Verbindung</h3><p class="hint">{{ error ? 'Status konnte nicht geladen werden.' : !data?.auth.connected ? 'ChatGPT ist noch nicht verbunden.' : !data.auth.plan_allowed ? 'Abo-Nutzung noch nicht freigegeben.' : data.paused_reason ? 'Analyse pausiert. Prüfe die Einstellungen.' : !data.settings.enabled ? 'Globale KI-Analyse ist ausgeschaltet.' : 'ChatGPT ist bereit.' }}</p><NuxtLink to="/settings?tab=ai" class="mt-4 flex items-center gap-2 text-xs text-accent">ChatGPT & Limits<ArrowUpRight :size="14" /></NuxtLink><p class="hint mt-5 border-t divider pt-4">Ein Videobild je 10 Sekunden plus lokales Sprachtranskript. Sehr kurze visuelle Aktionen können zwischen den Bildern liegen. Ergebnisse kommen nach der Auswertung, zeitversetzt zum Live-Signal.</p></section>
      <section class="panel p-5"><h3 class="mb-4 text-sm font-semibold">Letzte Auswertungen</h3><p v-if="!runs.length" class="hint">Noch keine Auswertung für diesen Kanal.</p><div v-for="run in runs.slice(0, 5)" :key="run.id" class="border-t divider py-3 first:border-0"><p class="text-xs" :class="run.status === 'clipped' ? 'text-mint' : run.status === 'failed' ? 'text-amber-200' : 'text-zinc-200'">{{ run.result?.title || ({ preparing: 'Sprache & Bilder vorbereiten', analyzing: 'ChatGPT wertet aus', failed: 'Auswertung fehlgeschlagen' } as Record<string, string>)[run.status] || 'Kein neuer Clip' }}</p><p class="hint break-words">{{ run.message || run.result?.reason }}</p><p class="mt-2 text-[11px] text-studio-400">{{ dateTime(run.created_at) }}</p></div></section>
    </aside>
  </div>
</template>
