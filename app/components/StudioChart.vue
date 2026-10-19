<script setup lang="ts">
import type { ApexOptions } from 'apexcharts'
import type ApexCharts from 'apexcharts'

// Preline's area-chart card, adapted to Vue lifecycle and locally bundled ApexCharts.
const props = withDefaults(defineProps<{
  series: { name: string; data: { x: number | string; y: number | null }[] }[]
  colors?: string[]
  height?: number
  type?: 'area' | 'bar'
  datetime?: boolean
  unit?: string
  min?: number
  max?: number
  mean?: number | null
  label: string
}>(), { colors: () => ['#6b9fff', '#38d6bd'], height: 200, type: 'area', datetime: true, unit: '' })
const element = ref<HTMLElement>()
const failed = ref(false)
let chart: ApexCharts | undefined
let disposed = false
const options = (): ApexOptions => ({
  chart: { type: props.type, height: props.height, background: 'transparent', foreColor: '#96a1b7', fontFamily: 'inherit', toolbar: { show: false }, zoom: { enabled: false }, animations: { enabled: false }, parentHeightOffset: 0, stacked: props.type === 'bar' },
  series: props.series,
  colors: props.colors,
  theme: { mode: 'dark' },
  legend: { show: false },
  dataLabels: { enabled: false },
  stroke: { curve: 'straight', width: props.type === 'bar' ? 0 : [2, 1.5], dashArray: props.type === 'bar' ? 0 : [0, 4] },
  fill: props.type === 'bar' ? { opacity: .9 } : { type: 'gradient', gradient: { type: 'vertical', shadeIntensity: 0, opacityFrom: .25, opacityTo: .01, stops: [0, 100] } },
  plotOptions: { bar: { borderRadius: 4, columnWidth: '34%' } },
  grid: { borderColor: '#3c3c3c', strokeDashArray: 4, padding: { left: 4, right: 12, bottom: 8 } },
  xaxis: { type: props.datetime ? 'datetime' : 'category', axisBorder: { show: false }, axisTicks: { show: false }, tickAmount: 4, labels: { datetimeUTC: false, style: { fontSize: '10px' }, hideOverlappingLabels: true }, tooltip: { enabled: false } },
  yaxis: { min: props.min, max: props.max, tickAmount: 3, forceNiceScale: true, labels: { minWidth: 28, style: { fontSize: '10px' }, formatter: value => Number.isFinite(value) ? `${Math.round(value)}` : '—' } },
  tooltip: { theme: 'dark', shared: true, intersect: false, x: { format: 'HH:mm:ss' }, y: { formatter: value => `${signalNumber(value)} ${props.unit}` } },
  annotations: { yaxis: props.mean == null ? [] : [{ y: props.mean, borderColor: '#b8a1ff', strokeDashArray: 5, label: { text: `Ø ${signalNumber(props.mean)}`, borderColor: 'transparent', style: { background: '#302a49', color: '#d2c4ff', fontSize: '10px' }, position: 'right', offsetX: -8 } }] },
  noData: { text: 'Noch keine Messwerte', style: { color: '#96a1b7', fontSize: '12px' } },
})
onMounted(async () => {
  try {
    const { default: ApexCharts } = await import('apexcharts')
    if (disposed || !element.value) return
    chart = new ApexCharts(element.value, options())
    await chart.render()
  } catch { if (!disposed) failed.value = true }
})
watch(() => [props.series, props.mean, props.min, props.max], () => { if (chart && !disposed) void chart.updateOptions(options(), false, false) })
onBeforeUnmount(() => { disposed = true; chart?.destroy(); chart = undefined })
</script>
<template>
  <div role="img" :aria-label="label" :style="{ minHeight: `${height}px` }">
    <p v-if="failed" class="flex h-40 items-center justify-center text-xs text-studio-400">Diagramm konnte nicht geladen werden.</p>
    <div v-else ref="element" />
  </div>
</template>
