<script setup lang="ts">
import { Check, X } from '@lucide/vue'
import type { MetricSummary, TriggerCondition } from '~/types'
const condition = defineModel<TriggerCondition>({ required: true })
const props = defineProps<{ id: string; current: number | null | undefined; summary: MetricSummary | null; removable: boolean }>()
const emit = defineEmits<{ remove: [] }>()
const meta = computed(() => triggerMetrics[condition.value.metric])
const matching = computed(() => props.current != null && (condition.value.op === 'gte' ? props.current >= condition.value.value : props.current <= condition.value.value))
const domain = computed(() => {
  const values = [condition.value.value, props.current, props.summary?.mean, props.summary?.low, props.summary?.high].filter((v): v is number => v != null && Number.isFinite(v))
  if (!values.length) return [0, 1] as const
  const low = Math.min(...values), high = Math.max(...values)
  const pad = Math.max(1, (high - low) * .15)
  return [low - pad, high + pad] as const
})
function position(value: number) { return Number.isFinite(value) ? (value - domain.value[0]) / (domain.value[1] - domain.value[0]) * 100 : 0 }
</script>
<template>
  <div class="rounded-xl border p-3.5 transition-colors" :class="matching ? 'border-mint/30 bg-mint/[.035]' : 'border-studio-700/60 bg-studio-950/40'">
    <div class="grid grid-cols-[70px_minmax(0,1fr)_28px] items-center gap-2 sm:grid-cols-[minmax(0,1fr)_70px_90px_28px]">
      <select v-model="condition.metric" class="input py-2 text-xs max-sm:col-span-2" :aria-label="`Signal ${id}`" @change="condition.value = meta.value"><option v-for="(item, key) in triggerMetrics" :key="key" :value="key">{{ item.label }}</option></select>
      <select v-model="condition.op" class="input py-2 text-xs max-sm:col-start-1 max-sm:row-start-2" :aria-label="`Vergleich ${id}`"><option value="gte">≥</option><option value="lte">≤</option></select>
      <div class="relative max-sm:col-span-2 max-sm:row-start-2"><input v-model.number="condition.value" type="number" class="input py-2 text-xs" :aria-label="`Schwelle ${id}`" :min="meta.min" :max="meta.max" :step="meta.step" required /></div>
      <button type="button" class="icon-btn size-7 max-sm:col-start-3 max-sm:row-start-1" :aria-label="`Bedingung ${id} entfernen`" :disabled="!removable" @click="emit('remove')"><X :size="14" /></button>
    </div>
    <div class="mt-3 grid grid-cols-3 gap-2 text-[11px]">
      <div><span class="text-studio-400">Jetzt</span><p class="mt-1 flex items-center gap-1 font-medium tabular-nums" :class="matching ? 'text-mint' : 'text-zinc-200'"><Check v-if="matching" :size="12" />{{ signalNumber(current) }} <span class="text-[10px] font-normal text-studio-400">{{ meta.unit }}</span></p></div>
      <div><span class="text-studio-400">Ø letzte 5 Min.</span><p class="mt-1 font-medium tabular-nums text-lavender">{{ signalNumber(summary?.mean) }} <span class="text-[10px] font-normal text-studio-400">{{ meta.unit }}</span></p></div>
      <div><span class="text-studio-400" title="10. bis 90. Perzentil: Hier liegen die mittleren 80 % der gemessenen Werte.">Typischer Bereich</span><p class="mt-1 font-medium tabular-nums">{{ signalNumber(summary?.low) }} <span class="text-studio-400">–</span> {{ signalNumber(summary?.high) }}</p></div>
    </div>
    <div v-if="summary" class="relative my-3 h-1.5 rounded-full bg-studio-800" aria-hidden="true">
      <span class="absolute inset-y-0 rounded-full bg-lavender/25" :style="{ left: `${position(summary.low)}%`, width: `${Math.max(1, position(summary.high) - position(summary.low))}%` }" />
      <span class="absolute -top-0.5 h-2.5 w-0.5 rounded bg-lavender" :style="{ left: `${position(summary.mean)}%` }" title="Durchschnitt" />
      <span class="absolute -top-1 h-3.5 w-0.5 rounded bg-amber-300" :style="{ left: `${position(condition.value)}%` }" title="Deine Schwelle" />
      <span v-if="current != null" class="absolute -top-0.5 size-2.5 -translate-x-1/2 rounded-full border-2 border-studio-900 bg-accent" :style="{ left: `${position(current)}%` }" />
    </div>
    <p class="mt-2 text-[10px] leading-relaxed text-studio-400">{{ meta.hint }}<span v-if="!summary"> Referenzwerte erscheinen mit den ersten Messungen.</span></p>
  </div>
</template>
