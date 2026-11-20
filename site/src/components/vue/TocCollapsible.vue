<script setup lang="ts">
import UButton from "@nuxt/ui/components/Button.vue";
import UCollapsible from "@nuxt/ui/components/Collapsible.vue";

/** The collapsible "On this page" list above a post or article on smaller screens. */
defineProps<{ headings: { depth: number; slug: string; text: string }[]; title: string }>();
</script>

<template>
  <!-- The page script marks the link being read; keep the links in the DOM while collapsed. -->
  <UCollapsible :unmount-on-hide="false" class="mt-10 border-y border-line" data-toc>
    <UButton
      color="neutral"
      variant="ghost"
      block
:label="title"
      trailing-icon="i-hugeicons-arrow-down-01"
      class="group justify-between rounded-none px-0 py-3 text-small font-medium text-ink hover:bg-transparent"
      :ui="{ trailingIcon: 'size-4 text-muted transition-transform group-data-[state=open]:rotate-180' }"
    />
    <template #content>
      <ul class="pb-2 text-small">
        <li v-for="heading in headings" :key="heading.slug">
          <a
            class="block py-1.5 text-muted transition-colors duration-quick ease-gentle hover:text-ink aria-[current=true]:text-ink"
            :class="heading.depth === 3 ? 'pl-8' : 'pl-4'"
            :href="`#${heading.slug}`"
          >{{ heading.text }}</a>
        </li>
      </ul>
    </template>
  </UCollapsible>
</template>
