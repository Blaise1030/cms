<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { cn } from "@/lib/utils"

const props = defineProps<{
  class?: HTMLAttributes["class"]
}>()

/** Diagonal wave: a cell's delay grows with its row + column. */
function cellDelay(n: number) {
  const i = n - 1
  return `${((i % 3) + Math.floor(i / 3)) * 0.1}s`
}
</script>

<template>
  <span
    role="status"
    aria-label="Loading"
    :class="cn('inline-grid size-3.5 shrink-0 grid-cols-3 grid-rows-3 gap-[8%]', props.class)"
  >
    <span
      v-for="n in 9"
      :key="n"
      class="grid-loader-cell rounded-[2px] bg-current"
      :style="{ animationDelay: cellDelay(n) }"
    />
  </span>
</template>

<style scoped>
.grid-loader-cell {
  animation: grid-loader-pulse 1.2s ease-in-out infinite;
}
@keyframes grid-loader-pulse {
  0%,
  100% {
    opacity: 0.2;
    transform: scale(0.6);
  }
  50% {
    opacity: 1;
    transform: scale(1);
  }
}
@media (prefers-reduced-motion: reduce) {
  .grid-loader-cell {
    animation-duration: 3s;
  }
}
</style>
