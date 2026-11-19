<script setup lang="ts">
import { computed } from "vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import { titleParts } from "./references";

/**
 * A chat title as people read it: a picked element or editor selection at its start shows as a
 * small tag (`div.page`, `a.md`), then the words, all on one truncated line. Raw reference
 * markup never shows.
 */
const props = defineProps<{ title: string }>();
const parts = computed(() => titleParts(props.title));
</script>

<template>
  <span class="flex min-w-0 items-center gap-1.5" :title="title">
    <span
      v-if="parts.elements.length"
      class="border-default bg-default/60 inline-flex max-w-32 shrink-0 items-center gap-1 rounded-sm border px-1 font-mono text-[11px]"
      data-testid="chat-title-tag"
    >
      <UIcon :name="parts.icons[0]!" class="size-3 shrink-0" aria-hidden="true" />
      <span class="truncate">{{ parts.elements[0] }}</span>
    </span>
    <span v-if="parts.text || !parts.elements.length" class="min-w-0 truncate">{{ parts.text || title }}</span>
  </span>
</template>
