<script setup lang="ts">
const emit = defineEmits<{ create: [] }>();
</script>

<template>
  <section class="flex flex-col gap-3" data-testid="template-gallery">
    <h2 class="text-lg font-medium">Create from a template</h2>
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <button
        type="button"
        data-testid="template-astro-marketing"
        class="bg-card hover:bg-accent/50 flex flex-col overflow-hidden rounded-lg border shadow-sm text-left transition-colors"
        @click="emit('create')"
      >
        <span class="bg-muted flex aspect-[16/10] flex-col gap-1.5 p-3" aria-hidden="true">
          <span class="bg-foreground/15 h-2 w-1/3 rounded-full" />
          <span class="bg-foreground/15 mt-1 h-3 w-3/4 rounded" />
          <span class="bg-foreground/10 h-2 w-full rounded" />
          <span class="bg-foreground/10 min-h-0 flex-1 rounded" />
        </span>
        <span class="flex flex-col gap-0.5 border-t px-3 py-2.5">
          <span class="text-sm font-medium">Astro marketing</span>
          <span class="text-muted-foreground text-xs">Astro landing page with blog and help content</span>
        </span>
      </button>
    </div>
  </section>
</template>
