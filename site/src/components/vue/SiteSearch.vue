<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { navigate } from "astro:transitions/client";
import type { CommandPaletteGroup, CommandPaletteItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UCommandPalette from "@nuxt/ui/components/CommandPalette.vue";
import UKbd from "@nuxt/ui/components/Kbd.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import { defineShortcuts } from "@nuxt/ui/composables/defineShortcuts";

/**
 * Search over one section's Pagefind index (built by integrations/pagefind.js after
 * `astro build`): a search button (or ⌘K / Ctrl+K) opens Nuxt UI's command palette in a modal;
 * type to see matching pages, pick one to open it. Indexes only exist in a build, so under
 * `astro dev` the palette says search is unavailable.
 */
const props = defineProps<{
  /** URL of the index folder, e.g. `/pagefind-blog/`. */
  bundlePath: string;
  placeholder: string;
  /** Translated palette copy, from `search` in src/i18n/messages (`{term}` is filled in here). */
  labels: { hint: string; unavailable: string; noResults: string; prompt: string };
}>();

type Pagefind = {
  debouncedSearch: (term: string, opts?: object, ms?: number) => Promise<{ results: { data: () => Promise<PageData> }[] } | null>;
};
type PageData = { url: string; excerpt: string; meta: { title?: string } };

const MAX_RESULTS = 8;
const open = ref(false);
const term = ref("");
const items = ref<CommandPaletteItem[]>([]);
const loading = ref(false);
const unavailable = ref(false);
let pagefind: Promise<Pagefind> | undefined;

/** Loads the section's Pagefind module once, on first search. */
function load() {
  pagefind ??= import(/* @vite-ignore */ `${props.bundlePath}pagefind.js`) as Promise<Pagefind>;
  return pagefind;
}

/** Pagefind excerpts mark matches with <mark>; the palette shows plain text. */
const plain = (html: string) => new DOMParser().parseFromString(html, "text/html").body.textContent ?? "";

function go(url: string) {
  open.value = false;
  navigate(url);
}

watch(term, async (value) => {
  const query = value.trim();
  if (!query) { items.value = []; loading.value = false; return; }
  loading.value = true;
  try {
    const search = await (await load()).debouncedSearch(query, {}, 200);
    if (search === null) return; // superseded by a newer keystroke
    const pages = await Promise.all(search.results.slice(0, MAX_RESULTS).map((result) => result.data()));
    if (query !== term.value.trim()) return;
    items.value = pages.map((page) => ({
      label: page.meta.title ?? page.url,
      description: plain(page.excerpt),
      onSelect: () => go(page.url),
    }));
  } catch {
    unavailable.value = true;
    items.value = [];
  } finally {
    if (query === term.value.trim()) loading.value = false;
  }
});

// Start each visit to the palette from an empty query.
watch(open, (isOpen) => { if (!isOpen) term.value = ""; });

const groups = computed<CommandPaletteGroup[]>(() => [{ id: "pages", items: items.value, ignoreFilter: true }]);

defineShortcuts({ meta_k: { usingInput: true, handler: () => { open.value = !open.value; } } });
</script>

<template>
  <UButton
    color="neutral"
    variant="outline"
    size="lg"
    icon="i-hugeicons-search-01"
    :aria-label="props.placeholder"
    aria-keyshortcuts="Meta+K Control+K"
    class="w-full bg-surface-raised text-small font-normal text-muted"
    @click="open = true"
  >
    <span class="truncate">{{ props.placeholder }}</span>
    <template #trailing>
      <span class="ms-auto hidden items-center gap-0.5 sm:flex">
        <UKbd value="meta" />
        <UKbd value="K" />
      </span>
    </template>
  </UButton>

  <UModal
    v-model:open="open"
    :title="props.placeholder"
    :description="props.labels.hint"
    :ui="{ overlay: 'z-50 bg-night/30 backdrop-blur-md', content: 'z-50 sm:max-w-xl' }"
  >
    <template #content>
      <UCommandPalette
        v-model:search-term="term"
        :groups="groups"
        :placeholder="props.placeholder"
        :loading="loading"
        icon="i-hugeicons-search-01"
        close
        class="h-96"
        :ui="{
          input: '[&>input]:text-small',
          item: 'items-start py-2',
          itemLabel: 'text-small font-medium text-ink whitespace-normal',
          itemDescription: 'line-clamp-2 text-label text-muted whitespace-normal',
          empty: 'text-small text-muted',
        }"
        @update:open="open = $event"
      >
        <template #empty>
          <span v-if="unavailable">{{ props.labels.unavailable }}</span>
          <span v-else-if="term.trim() && !loading">{{ props.labels.noResults.replace("{term}", term.trim()) }}</span>
          <span v-else>{{ props.labels.prompt }}</span>
        </template>
      </UCommandPalette>
    </template>
  </UModal>
</template>
