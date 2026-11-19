<script setup lang="ts">
import UAccordion from "@nuxt/ui/components/Accordion.vue";

export type FaqItem = { question: string; answer: string };

const props = defineProps<{ items: FaqItem[] }>();
const items = props.items.map((faq) => ({ label: faq.question, content: faq.answer }));
</script>

<template>
  <UAccordion
    :items="items"
    trailing-icon="i-hugeicons-arrow-down-01"
    class="border-t border-line"
    :ui="{
      item: 'border-line',
      trigger: 'py-5 text-body font-normal text-ink',
      content: 'max-w-prose',
      body: 'pb-6 pt-0 text-body text-muted',
    }"
  />
</template>
