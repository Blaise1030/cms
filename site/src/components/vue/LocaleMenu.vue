<script setup lang="ts">
import type { DropdownMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";

export type LocaleItem = { label: string; lang: string; href: string; current: boolean };

/** The header language picker: each item links to this page in that language. */
const props = defineProps<{ items: LocaleItem[]; label: string }>();
const current = props.items.find((item) => item.current) ?? props.items[0];
const items: DropdownMenuItem[] = props.items.map((item) => ({
  label: item.label,
  to: item.href,
  lang: item.lang,
  hreflang: item.lang,
  icon: item.current ? "i-hugeicons-tick-02" : undefined,
  class: item.current ? "font-medium text-ink" : "text-muted",
}));
</script>

<template>
  <UDropdownMenu :items="items" :content="{ align: 'end' }" :ui="{ content: 'w-48' }">
    <UButton
      color="neutral"
      variant="ghost"
      size="lg"
      icon="i-hugeicons-translate"
      :aria-label="`${props.label}: ${current.label}`"
      class="text-muted"
    />
  </UDropdownMenu>
</template>
