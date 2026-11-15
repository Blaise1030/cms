<script setup lang="ts">
// One labelled block inside a settings page: a quiet icon, the label, optional actions (such as
// Recheck) on the right, and the section's card below. Rows in the card are divided.
import UCard from "@nuxt/ui/components/Card.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";

defineProps<{
  label: string;
  /** Iconify name, e.g. `i-hugeicons-information-circle`. */
  icon?: string;
}>();
</script>

<template>
  <section class="flex flex-col gap-2.5">
    <div class="flex min-h-7 items-center justify-between gap-2 px-1">
      <h2 class="text-muted flex items-center gap-1.5 text-xs font-medium">
        <UIcon v-if="icon" :name="icon" class="size-3.5 shrink-0" />
        {{ label }}
      </h2>
      <slot name="actions" />
    </div>
    <UCard variant="outline" :ui="{ root: 'overflow-hidden', body: 'divide-y divide-default p-0 sm:p-0' }">
      <slot />
    </UCard>
  </section>
</template>
