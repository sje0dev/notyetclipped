<script setup lang="ts">
import { Unplug, RefreshCw, Terminal } from '@lucide/vue'
defineEmits<{ retry: [] }>()
</script>
<template>
  <div class="panel overflow-hidden">
    <div class="flex flex-col gap-6 p-8 sm:flex-row sm:items-center sm:p-10">
      <div class="flex size-16 shrink-0 items-center justify-center rounded-2xl border border-amber-300/20 bg-amber-300/5 text-amber-200"><Unplug :size="28" /></div>
      <div class="flex-1"><h2 class="text-lg font-semibold">Dein Studio wartet auf die Verbindung.</h2><p class="mt-2 max-w-xl text-sm leading-relaxed text-studio-400">Das Dashboard ist bereit. Starte die Backend-Dienste, um Streamer hinzuzufügen und deine ersten Highlights einzufangen.</p></div>
      <button class="btn btn-secondary shrink-0" @click="$emit('retry')"><RefreshCw :size="15" />Erneut prüfen</button>
    </div>
    <div class="flex items-center gap-3 border-t divider bg-studio-950/50 px-8 py-4 text-xs text-studio-400"><Terminal :size="15" /><code class="text-zinc-300">docker compose up --build -d</code><span class="ml-auto hidden sm:block">PostgreSQL + S3 + API + Worker</span></div>
  </div>
</template>
