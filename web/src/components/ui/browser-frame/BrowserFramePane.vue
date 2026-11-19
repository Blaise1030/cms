<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useElementSize } from "@vueuse/core";
import { frameFit } from "@/modules/preview/frameFit";
import type { Device } from "@/modules/preview/store";

/**
 * One side of a BrowserFrame: the page is laid out at the device width (desktop: 1440 px) and,
 * when this pane is narrower, scaled down to fill exactly its width, refitted as the pane
 * resizes, the same way the preview page fits its frame.
 */
const props = defineProps<{ src: string | null; title: string; device: Device; reloadNonce: number; testid: string }>();
const emit = defineEmits<{ fit: [scale: number]; load: [frame: HTMLIFrameElement] }>();
defineSlots<{ default(): unknown }>();

const viewport = ref<HTMLElement | null>(null);
const size = useElementSize(viewport);
const fit = computed(() => frameFit(props.device, size.width.value, size.height.value));
const zoomed = computed(() => fit.value.scale < 1);
watch(
  () => fit.value.scale,
  (scale) => emit("fit", scale),
  { immediate: true },
);
</script>

<template>
  <div ref="viewport" class="bg-background flex min-h-0 flex-1 justify-center overflow-auto">
    <slot v-if="!src" />
    <!-- Sized to the scaled frame, so the pane centres and clips what is actually shown. -->
    <div
      v-else
      class="relative h-full shrink-0 overflow-hidden"
      :data-testid="`${testid}-canvas`"
      :style="zoomed ? { width: `${fit.width! * fit.scale}px` } : { width: fit.width ? `${fit.width}px` : '100%' }"
    >
      <!-- Another site: its scripts run in its own origin, never orso's. -->
      <iframe
        :key="`${src}:${reloadNonce}`"
        :src="src"
        :title="title"
        class="h-full origin-top-left border-0 bg-white"
        :style="{
          width: fit.width ? `${fit.width}px` : '100%',
          ...(zoomed ? { height: `${fit.height}px`, transform: `scale(${fit.scale})` } : {}),
        }"
        sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
        referrerpolicy="no-referrer"
        :data-testid="testid"
        @load="emit('load', $event.target as HTMLIFrameElement)"
      />
      <!-- A device-sized frame has no visible edge against the pane; outline it. -->
      <div
        v-if="device !== 'desktop'"
        class="border-default pointer-events-none absolute inset-0 border"
        aria-hidden="true"
      />
    </div>
  </div>
</template>
