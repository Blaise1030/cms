<script setup lang="ts">
import UPageCard from "@nuxt/ui/components/PageCard.vue";

const emit = defineEmits<{ create: [] }>();
</script>

<template>
  <section class="flex flex-col gap-3" data-testid="template-gallery">
    <h2 class="text-highlighted text-lg font-medium">Create from a template</h2>
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <!-- A real <button> root keeps the card keyboard-reachable; UPageCard only links with `to`. -->
      <UPageCard
        as="button"
        type="button"
        variant="outline"
        title="Astro marketing"
        description="Astro landing page with blog and help content"
        class="btn-depth-soft hover:bg-elevated/50 focus-visible:outline-primary text-left transition-colors focus-visible:outline-2"
        :ui="{
          container: 'gap-0 p-0 sm:p-0 lg:flex',
          wrapper: 'px-3 py-2.5 border-t border-default',
          title: 'text-sm font-medium',
          description: 'text-xs text-muted',
        }"
        data-testid="template-astro-marketing"
        @click="emit('create')"
      >
        <!-- Layout sketch of the template, not real content. -->
        <span class="bg-muted order-first flex aspect-[16/10] flex-col gap-1.5 rounded-t-lg p-3" aria-hidden="true">
          <span class="bg-accented h-2 w-1/3 rounded-full" />
          <span class="bg-accented mt-1 h-3 w-3/4 rounded" />
          <span class="bg-elevated h-2 w-full rounded" />
          <span class="bg-elevated min-h-0 flex-1 rounded" />
        </span>
      </UPageCard>
    </div>
  </section>
</template>
