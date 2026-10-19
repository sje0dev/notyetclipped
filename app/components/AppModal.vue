<script setup lang="ts">
import { X } from '@lucide/vue'
const props = defineProps<{ open: boolean; title: string; wide?: boolean }>()
const emit = defineEmits<{ close: [] }>()
const dialog = ref<HTMLDialogElement>()
watch(() => props.open, async (open) => {
  await nextTick()
  if (open && !dialog.value?.open) dialog.value?.showModal()
  if (!open && dialog.value?.open) dialog.value?.close()
}, { immediate: true })
onMounted(() => { if (props.open) dialog.value?.showModal() })
function backdrop(event: MouseEvent) {
  if (event.target !== dialog.value) return
  const rect = dialog.value.getBoundingClientRect()
  if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) emit('close')
}
</script>

<template>
  <dialog ref="dialog" class="w-[calc(100%-2rem)] rounded-2xl border border-studio-700 bg-studio-900 p-0 text-zinc-100 shadow-2xl" :class="wide ? 'max-w-4xl' : 'max-w-lg'" :aria-label="title" @cancel.prevent="emit('close')" @click="backdrop">
    <div class="flex items-center justify-between border-b divider px-6 py-5">
      <h2 class="text-lg font-semibold tracking-tight">{{ title }}</h2>
      <button class="icon-btn" aria-label="Schließen" @click="emit('close')"><X :size="19" /></button>
    </div>
    <div class="max-h-[75dvh] overflow-y-auto p-6"><slot /></div>
  </dialog>
</template>
