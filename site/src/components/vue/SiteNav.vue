<script setup lang="ts">
import type { NavigationMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import UNavigationMenu from "@nuxt/ui/components/NavigationMenu.vue";

export type NavItem = { label: string; href: string; current?: boolean };

/**
 * The header navigation. `links`: the desktop link row (static, before the header CTAs).
 * `menu`: the tablet/mobile menu button and its full-height sheet (an island, after the CTAs).
 */
const props = defineProps<{ items: NavItem[]; part: "links" | "menu" }>();
const links: NavigationMenuItem[] = props.items.map((item) => ({ label: item.label, to: item.href, active: item.current }));
</script>

<template>
  <UNavigationMenu
    v-if="props.part === 'links'"
    :items="links"
    variant="link"
    color="neutral"
    :highlight="false"
    aria-label="Main"
    class="hidden pe-3 lg:flex"
    :ui="{ list: 'gap-5', link: 'px-0 py-1 text-small font-normal text-muted hover:text-ink data-active:text-ink before:hidden' }"
  />

  <UModal
    v-else
    fullscreen
    title="Menu"
    :close="false"
    :ui="{ overlay: 'z-50 bg-night/60 lg:hidden', content: 'z-50 inset-2 rounded-xl border border-line bg-surface-raised lg:hidden', header: 'sr-only', body: 'my-auto flex flex-col px-6 py-24 sm:px-6' }"
  >
    <UButton color="neutral" variant="outline" size="lg" icon="i-hugeicons-menu-01" class="lg:hidden" aria-label="Open menu" />
    <template #body="{ close }">
      <p class="label tracking-wide uppercase" aria-hidden="true">Menu</p>
      <UNavigationMenu
        :items="links"
        orientation="vertical"
        variant="link"
        color="neutral"
        aria-label="Main"
        class="mt-3"
        :ui="{ link: 'px-0 py-1.5 text-heading font-medium tracking-tight text-ink hover:text-muted data-active:text-muted before:hidden', linkLabel: 'overflow-visible' }"
        @click="close"
      />
      <UButton size="lg" icon="i-hugeicons-cancel-01" label="Close" class="sticky bottom-6 mx-auto mt-auto" @click="close" />
    </template>
  </UModal>
</template>
