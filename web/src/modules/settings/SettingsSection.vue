<script setup lang="ts">
// One labelled block inside a settings page: a quiet icon, the label, and
// optional actions (such as Recheck) on the right.
import type { Component } from "vue";

defineProps<{
  label: string;
  icon?: Component;
}>();
</script>

<template>
  <section class="flex flex-col gap-2.5">
    <div class="flex min-h-6 items-center justify-between gap-2 px-1">
      <h2 class="text-muted-foreground flex items-center gap-1.5 text-[11px] font-medium">
        <component :is="icon" v-if="icon" class="size-3.5 shrink-0" aria-hidden="true" />
        {{ label }}
      </h2>
      <slot name="actions" />
    </div>
    <slot />
  </section>
</template>
