<script setup lang="ts">
import type { NavigationMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UCollapsible from "@nuxt/ui/components/Collapsible.vue";
import UNavigationMenu from "@nuxt/ui/components/NavigationMenu.vue";

/**
 * The Help navigation: every section, subsection and article. Desktop shows it as a sidebar
 * whose sections collapse (the one holding this page starts open); mobile folds it into one
 * "Browse help" collapsible. Built from plain data so it can hydrate as an island. Collapsed
 * groups stay in the HTML (hidden) so every article link is crawlable.
 */
const props = defineProps<{
  items: NavigationMenuItem[];
  /** Translated "Browse help" button and the sidebar's accessible name. */
  labels: { browse: string; nav: string };
}>();

const navUi = {
  list: "gap-0.5",
  childList: "ms-3 border-s-0 ps-0",
  childItem: "ps-0",
  link: "rounded-md px-2.5 py-1.5 text-small font-normal text-muted before:rounded-md hover:text-ink data-active:font-medium data-active:text-ink",
  childLink: "rounded-md px-2.5 py-1.5 text-small text-muted hover:text-ink data-active:font-medium data-active:text-ink",
  linkTrailingIcon: "size-4 text-muted",
};
</script>

<template>
  <UCollapsible class="lg:hidden" :unmount-on-hide="false">
    <UButton
      color="neutral"
      variant="ghost"
      block
:label="props.labels.browse"
      trailing-icon="i-hugeicons-arrow-down-01"
      class="group justify-between px-0 text-small font-medium text-ink hover:bg-transparent"
      :ui="{ trailingIcon: 'size-4 text-muted transition-transform group-data-[state=open]:rotate-180' }"
    />
    <template #content>
      <UNavigationMenu :items="props.items" orientation="vertical" color="neutral" :unmount-on-hide="false" class="mt-3" :ui="navUi" />
    </template>
  </UCollapsible>

  <UNavigationMenu
    :items="props.items"
    orientation="vertical"
    color="neutral"
    :unmount-on-hide="false"
    :aria-label="props.labels.nav"
    class="hidden lg:flex"
    :ui="navUi"
  />
</template>
