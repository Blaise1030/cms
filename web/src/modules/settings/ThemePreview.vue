<script setup lang="ts">
// A tiny window mockup used by the Appearance choices. The neutral palette is
// deliberate: each mockup previews one appearance even while the app itself is
// in the other theme.
defineProps<{ mode: "light" | "dark" | "system" }>();
</script>

<template>
  <span
    class="relative block aspect-[4/3] w-20 overflow-hidden rounded-md border border-zinc-300/80 shadow-sm"
    aria-hidden="true"
  >
    <!-- The pane itself. -->
    <span
      class="absolute inset-0 transition-colors"
      :class="mode === 'dark' ? 'bg-zinc-800' : 'bg-zinc-100'"
    />

    <!-- Window chrome and the open page. -->
    <span
      class="absolute inset-y-0 left-0 w-1/4 border-r transition-colors"
      :class="mode === 'dark' ? 'border-zinc-700 bg-zinc-700/70' : 'border-zinc-300 bg-zinc-200/80'"
    />
    <span
      class="absolute top-2.5 right-2.5 left-[30%] h-1.5 rounded-full transition-colors"
      :class="mode === 'dark' ? 'bg-zinc-600' : 'bg-zinc-300'"
    />
    <span
      class="absolute top-5 right-5 left-[30%] h-1 rounded-full transition-colors"
      :class="mode === 'dark' ? 'bg-zinc-700' : 'bg-zinc-300/80'"
    />

    <!-- Auto: a dark page taking over the right-hand side. -->
    <template v-if="mode === 'system'">
      <span class="absolute inset-y-0 right-0 w-1/2 border-l border-zinc-600 bg-zinc-800" />
      <span class="absolute top-2.5 right-2 h-1.5 w-[35%] rounded-full bg-zinc-600" />
      <span class="absolute top-5 right-3.5 h-1 w-[22%] rounded-full bg-zinc-700" />
    </template>
  </span>
</template>
