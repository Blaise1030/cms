<script setup lang="ts">
import type { DropdownMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";

export type LocaleItem = { label: string; lang: string; flag: string; href: string; current: boolean };

/** The header language picker: each item links to this page in that language, flag first and a tick after the current one. */
const props = defineProps<{ items: LocaleItem[]; label: string }>();
const current = props.items.find((item) => item.current) ?? props.items[0];
const items: DropdownMenuItem[] = props.items.map((item) => ({
  label: item.label,
  to: item.href,
  lang: item.lang,
  hreflang: item.lang,
  icon: item.flag,
  current: item.current,
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
    <template #item-trailing="{ item }">
      <UIcon v-if="item.current" name="i-hugeicons-tick-02" class="size-4 shrink-0 text-ink" aria-hidden="true" />
    </template>
  </UDropdownMenu>
</template>
