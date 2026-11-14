<script lang="ts" setup>
import type { ToasterProps } from "vue-sonner"
import { CircleCheckIcon, InfoIcon, OctagonXIcon, TriangleAlertIcon, XIcon } from "@lucide/vue"
import { Toaster as Sonner } from "vue-sonner"
import { Spinner } from "@/components/ui/spinner"
import { cn } from "@/lib/utils"
import "vue-sonner/style.css"

const props = defineProps<ToasterProps>()
</script>

<template>
  <Sonner
    :class="cn('toaster group', props.class)"
    :style="{
      '--normal-bg': 'var(--popover)',
      '--normal-text': 'var(--popover-foreground)',
      '--normal-border': 'var(--border)',
      '--border-radius': 'var(--radius)',
    }"
    v-bind="props"
  >
    <template #success-icon>
      <CircleCheckIcon class="size-4 fill-emerald-500 text-white dark:fill-emerald-400 dark:text-emerald-950" />
    </template>
    <template #info-icon>
      <InfoIcon class="size-4" />
    </template>
    <template #warning-icon>
      <TriangleAlertIcon class="size-4" />
    </template>
    <template #error-icon>
      <OctagonXIcon class="size-4" />
    </template>
    <template #loading-icon>
      <div>
        <Spinner class="size-4" />
      </div>
    </template>
    <template #close-icon>
      <XIcon class="size-4" />
    </template>
  </Sonner>
</template>

<style>
/* Top-center toasts: keep the fixed-width pill and center the icon + title inside it. */
[data-sonner-toaster][data-x-position='center'] [data-sonner-toast][data-styled='true'] {
  justify-content: center;
}

/* Sonner's default shadow is black @ 10% — invisible on dark previews. Lift the toast clearly. */
[data-sonner-toaster][data-sonner-theme='dark'] [data-sonner-toast][data-styled='true'] {
  background: var(--normal-bg);
  box-shadow:
    0 0 0 1px color-mix(in oklch, var(--foreground) 12%, transparent),
    0 8px 24px -4px rgb(0 0 0 / 0.55),
    0 2px 8px rgb(0 0 0 / 0.4);
}

[data-sonner-toaster][data-sonner-theme='light'] [data-sonner-toast][data-styled='true'] {
  background: var(--normal-bg);
  box-shadow:
    0 0 0 1px color-mix(in oklch, var(--foreground) 8%, transparent),
    0 8px 24px -4px rgb(0 0 0 / 0.18),
    0 2px 8px rgb(0 0 0 / 0.08);
}
</style>
