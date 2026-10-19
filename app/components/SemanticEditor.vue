<script setup lang="ts">
import { BookOpen, ScanEye, Sparkles, ArrowUpRight, Plus, Trash2 } from '@lucide/vue'
import type { SemanticTrigger, AiStatus, AiRun, ContentRule } from '~/types'
const model = defineModel<SemanticTrigger>({ required: true })
const props = defineProps<{ streamerId: string }>()
const { data, error, refresh } = await useFetch<AiStatus>('/api/ai', { key: 'ai', server: false })
usePolling(refresh, 15000, false)
const { data: runs, refresh: refreshRuns } = await useFetch<AiRun[]>(`/api/streamers/${props.streamerId}/analyses`, { key: `analyses-${props.streamerId}`, server: false })
usePolling(refreshRuns, 15000, false)
const expanded = ref('')
function addRule(preset?: Omit<ContentRule, 'id' | 'enabled'>) {
  if (model.value.rules.length >= 12) return
  const id = crypto.randomUUID()
  model.value.rules.push({ id, enabled: true, ...(preset || { name: 'Eigener Text-Trigger', kind: 'story', description: 'Ein besonderer Moment mit verständlichem Aufbau und einem klaren Abschluss.' }) })
  expanded.value = id
}
function example() { model.value.guidance = 'Spontane Comedy, absurde Monologe, gespielte Empörung, schlagfertige Gespräche und überraschende Enthüllungen mit anderen Streamern. Auch lustige Gesichtsausdrücke oder körperliche Aktionen. Den Satz oder die Situation vor der Pointe mitnehmen. Gewöhnliches Bauen, Farming und reine Lautstärke auslassen.' }
</script>
<template>
  <div class="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
    <section class="panel">
      <div class="section-heading"><Sparkles :size="20" class="text-lavender" /><div><h2>Inhaltliche Momente</h2><p>Beschreibe in deinen Worten, was einen Clip verdient.</p></div><label class="ml-auto flex items-center gap-2 text-sm"><input v-model="model.enabled" type="checkbox" class="checkbox" />Aktiv</label></div>
      <div class="space-y-6 p-6">
        <div class="grid gap-4 sm:grid-cols-2">
          <label class="flex cursor-pointer items-start gap-3 rounded-xl border p-4 transition" :class="model.stories ? 'border-accent/40 bg-accent/5' : 'divider'"><input v-model="model.stories" type="checkbox" class="checkbox mt-1" /><div><BookOpen :size="19" class="mb-3 text-accent" /><span class="text-sm font-medium">Stories & Gespräche</span><p class="hint">Anekdoten, Pointen, Wortwechsel und spontane Comedy. Mit dem Kontext davor.</p></div></label>
          <label class="flex cursor-pointer items-start gap-3 rounded-xl border p-4 transition" :class="model.visual ? 'border-lavender/40 bg-lavender/5' : 'divider'"><input v-model="model.visual" type="checkbox" class="checkbox mt-1" /><div><ScanEye :size="19" class="mb-3 text-lavender" /><span class="text-sm font-medium">Video-Momente</span><p class="hint">Sichtbare Reaktionen, Fails, überraschende Situationen und Enthüllungen.</p></div></label>
        </div>
        <div><div class="mb-2 flex flex-wrap items-center justify-between gap-2"><label for="semantic-guidance" class="label mb-0">Was macht für dich einen guten Clip aus?</label><button type="button" class="text-xs text-accent hover:underline" @click="example">Comedy-Vorlage einsetzen</button></div><textarea id="semantic-guidance" v-model="model.guidance" class="input min-h-32 resize-y leading-relaxed" maxlength="1000" placeholder="Zum Beispiel: lustige Erlebnisse, trockene Konter und unerwartete Reaktionen. Bitte die Vorgeschichte mitnehmen." /><p class="hint">{{ model.guidance.length }} / 1000 Zeichen · Gilt nur für diesen Streamer.</p></div>
        <div class="border-t divider pt-5">
          <div class="mb-2 flex items-center justify-between gap-3"><h3 class="text-sm font-semibold">Deine Text-Trigger</h3><span class="text-xs text-studio-400">{{ model.rules.length }} / 12</span></div>
          <p class="hint mb-4">Jeder aktive Trigger ist eine weitere Möglichkeit für einen Clip. Die KI prüft alle gemeinsam und wählt den stärksten neuen Moment. Du kannst auch die beiden allgemeinen Kategorien oben ausschalten und nur eigene Trigger verwenden.</p>
          <div class="mb-5 flex flex-wrap gap-2"><button v-for="preset in contentPresets" :key="preset.name" type="button" class="btn btn-secondary text-xs" :disabled="model.rules.length >= 12" @click="addRule(preset)"><Plus :size="12" />{{ preset.name }}</button></div>
          <div class="space-y-3">
            <details v-for="(rule, index) in model.rules" :key="rule.id" :open="expanded === rule.id" class="rounded-xl border divider bg-studio-950/40">
              <summary class="flex cursor-pointer items-center gap-3 p-4 text-sm"><input v-model="rule.enabled" type="checkbox" class="checkbox" :aria-label="`${rule.name} aktivieren`" @click.stop /><span class="min-w-0 flex-1 break-words">{{ rule.name }}</span><span class="text-[11px] text-studio-400">{{ rule.kind === 'story' ? 'Sprache' : 'Video' }}</span></summary>
              <div class="space-y-4 border-t divider p-4">
                <div class="grid gap-4 sm:grid-cols-2"><div><label :for="`name-${rule.id}`" class="label">Name</label><input :id="`name-${rule.id}`" v-model="rule.name" maxlength="60" class="input" /></div><div><label :for="`kind-${rule.id}`" class="label">Beweisgrundlage</label><select :id="`kind-${rule.id}`" v-model="rule.kind" class="input"><option value="story">Sprache + Bildkontext</option><option value="visual">Sichtbares Ereignis</option></select></div></div>
                <div><label :for="`description-${rule.id}`" class="label">Wann soll dieser Trigger auslösen?</label><textarea :id="`description-${rule.id}`" v-model="rule.description" maxlength="1000" class="input min-h-32 resize-y leading-relaxed" placeholder="Beschreibe den Moment, den nötigen Kontext und was nicht zählen soll …" /><p class="hint">{{ rule.description.length }} / 1000 Zeichen · Konkrete Beispiele helfen mehr als „alles Lustige“.</p></div>
                <button type="button" class="btn btn-ghost text-xs text-red-300" @click="model.rules.splice(index, 1)"><Trash2 :size="13" />Trigger entfernen</button>
              </div>
            </details>
          </div>
          <button type="button" class="btn btn-secondary mt-4" :disabled="model.rules.length >= 12" @click="addRule()"><Plus :size="14" />Eigenen Text-Trigger hinzufügen</button>
        </div>
        <div><label for="semantic-exclusions" class="label">Was soll ausdrücklich draußen bleiben?</label><textarea id="semantic-exclusions" v-model="model.exclusions" maxlength="1000" class="input min-h-24 resize-y" placeholder="Zum Beispiel: Begrüßungen, Spenden-Dank, wiederholte Insider ohne Erklärung, gewöhnliches Farming …" /><p class="hint">Gilt für alle inhaltlichen Trigger dieses Kanals.</p></div>
        <div class="grid gap-5 sm:grid-cols-2"><div><label for="semantic-interval" class="label">Frühestens erneut analysieren nach</label><select id="semantic-interval" v-model.number="model.interval_seconds" class="input"><option :value="30">30 Sekunden</option><option :value="60">60 Sekunden</option><option :value="90">90 Sekunden</option><option :value="120">2 Minuten</option><option :value="300">5 Minuten</option></select><p class="hint">Das globale Budget und die anderen Kanäle bestimmen den tatsächlichen Abstand mit.</p></div><div><label for="semantic-score" class="label">Mindestbewertung · {{ Math.round(model.min_confidence * 100) }} / 100</label><input id="semantic-score" v-model.number="model.min_confidence" type="range" min="0.5" max="1" step="0.05" class="mt-4 w-full accent-accent" /><p class="hint">80: starker eigenständiger Moment · 90: außergewöhnlich. Subjektiver KI-Score.</p></div></div>
        <div class="grid gap-5 sm:grid-cols-2"><div><label for="semantic-context" class="label">Kontext vor der Entscheidung</label><select id="semantic-context" v-model.number="model.context_seconds" class="input"><option :value="60">1 Minute · kurze Reaktionen</option><option :value="120">2 Minuten · ausgewogen</option><option :value="180">3 Minuten · längere Geschichten</option></select><p class="hint">Mehr Kontext verlängert die lokale Transkription. Der fertige Clip bleibt höchstens 2 Minuten lang.</p></div><div><label for="semantic-language" class="label">Sprache im Stream</label><select id="semantic-language" v-model="model.language" class="input"><option value="auto">Automatisch erkennen</option><option value="de">Deutsch</option><option value="en">Englisch</option></select><p class="hint">Eine feste Sprache hilft bei kurzen oder undeutlichen Sätzen.</p></div></div>
        <label class="flex items-start gap-3 text-sm"><input v-model="model.use_feedback" type="checkbox" class="checkbox mt-1" /><span>Meine Clip-Bewertungen berücksichtigen<p class="hint">Bis zu sechs bewertete Clips dieses Kanals dienen als Geschmackshinweise: Favoriten und Daumen hoch bevorzugen, Daumen runter vermeiden. Kein automatisches Modelltraining.</p></span></label>
        <p class="border-t divider pt-5 text-sm leading-relaxed text-studio-400">Auswahl mit Aufbau, Pointe und Reaktion: möglichst 15–60 Sekunden, bei Bedarf 5–120. Bereits gesicherte Momente werden der KI zum Ausschluss mitgegeben. Die gemeinsamen Clip-Limits gelten weiterhin.</p>
      </div>
    </section>
    <aside class="space-y-5">
      <section class="panel p-5"><h3 class="text-sm font-semibold">Analyse-Verbindung</h3><p class="hint">{{ error ? 'Status konnte nicht geladen werden.' : !data?.auth.connected ? 'ChatGPT ist noch nicht verbunden.' : !data.auth.plan_allowed ? 'Abo-Nutzung noch nicht freigegeben.' : data.paused_reason ? 'Analyse pausiert. Prüfe die Einstellungen.' : !data.settings.enabled ? 'Globale KI-Analyse ist ausgeschaltet.' : 'ChatGPT ist bereit.' }}</p><NuxtLink to="/settings?tab=ai" class="mt-4 flex items-center gap-2 text-xs text-accent">ChatGPT & Limits<ArrowUpRight :size="14" /></NuxtLink><p class="hint mt-5 border-t divider pt-4">Bis zu {{ data?.settings.frame_count || 24 }} Keyframe-Bilder plus lokales Sprachtranskript. Sehr kurze Aktionen können zwischen den Bildern liegen. Video-Trigger allein benötigen keine Transkription.</p></section>
      <section class="panel p-5"><h3 class="mb-4 text-sm font-semibold">Letzte Auswertungen dieses Kanals</h3><p v-if="!runs?.length" class="hint">Noch keine Auswertung für diesen Kanal.</p><div v-for="run in (runs || []).slice(0, 5)" :key="run.id" class="border-t divider py-3 first:border-0"><p class="text-xs" :class="run.status === 'clipped' ? 'text-mint' : run.status === 'failed' ? 'text-amber-200' : 'text-zinc-200'">{{ run.result?.title || ({ preparing: 'Sprache & Bilder vorbereiten', analyzing: 'ChatGPT wertet aus', failed: 'Auswertung fehlgeschlagen' } as Record<string, string>)[run.status] || 'Kein neuer Clip' }}</p><p class="hint break-words">{{ run.message }}</p><p v-if="run.result?.reason" class="mt-2 text-xs leading-relaxed text-studio-300">{{ run.result.reason }}</p><p class="mt-2 text-[11px] text-studio-400">{{ dateTime(run.created_at) }}<span v-if="run.preparation_ms != null"> · lokal {{ Math.round(run.preparation_ms / 1000) }}s</span><span v-if="run.inference_ms != null"> · KI {{ Math.round(run.inference_ms / 1000) }}s</span></p></div></section>
    </aside>
  </div>
</template>
