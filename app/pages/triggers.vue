<script setup lang="ts">
const route = useRoute()
await navigateTo(typeof route.query.streamer === 'string' ? `/streamers/${encodeURIComponent(route.query.streamer)}` : '/streamers', { replace: true })
</script>
<template><div /></template>
