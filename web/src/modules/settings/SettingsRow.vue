<script setup lang="ts">
// One row inside a settings card: a quiet icon, a title and description, optional extra content
// under them, and controls on the right. Every settings page builds its cards from these rows so
// spacing and type stay the same across sections.
import UAvatar from "@nuxt/ui/components/Avatar.vue";

withDefaults(
  defineProps<{
    /** Iconify name, e.g. `i-hugeicons-information-circle`. */
    icon?: string;
    title?: string;
    description?: string;
    /** Top-align the icon and controls when the row has content under the description. */
    align?: "center" | "start";
  }>(),
  { align: "center" },
);
</script>

<template>
  <div class="flex gap-3 px-4 py-3" :class="align === 'start' ? 'items-start' : 'items-center'">
    <UAvatar
      v-if="icon"
      :icon="icon"
      color="neutral"
      size="md"
      :ui="{ root: 'rounded-md bg-elevated ring ring-default', icon: 'text-muted size-4' }"
    />
    <div class="flex min-w-0 flex-1 flex-col gap-1">
      <p class="text-highlighted flex items-center gap-1.5 text-sm font-medium">
        <slot name="title">{{ title }}</slot>
      </p>
      <p v-if="description || $slots.description" class="text-muted text-sm text-pretty">
        <slot name="description">{{ description }}</slot>
      </p>
      <slot />
    </div>
    <div v-if="$slots.actions" class="flex shrink-0 items-center gap-1.5">
      <slot name="actions" />
    </div>
  </div>
</template>
