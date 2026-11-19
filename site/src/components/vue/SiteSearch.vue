<script setup lang="ts">
import { ref, useTemplateRef, watch } from "vue";
import { navigate } from "astro:transitions/client";
import UInputMenu from "@nuxt/ui/components/InputMenu.vue";
import { defineShortcuts } from "@nuxt/ui/composables/defineShortcuts";

/**
 * Search over one section's Pagefind index (built by integrations/pagefind.js after
 * `astro build`) in Nuxt UI's combobox: type to see matching pages, pick one to open it.
 * ⌘K / Ctrl+K focuses the field. Indexes only exist in a build, so under `astro dev` the
 * field says search is unavailable.
 */
const props = defineProps<{
  /** Base-aware URL of the index folder, e.g. `/pagefind-blog/`. */
  bundlePath: string;
  /** Site base URL, so result links include it. */
  baseUrl: string;
  placeholder: string;
}>();

type Result = { label: string; description: string; url: string };
type Pagefind = {
  options: (opts: { baseUrl: string }) => Promise<void>;
  debouncedSearch: (term: string, opts?: object, ms?: number) => Promise<{ results: { data: () => Promise<PageData> }[] } | null>;
};
type PageData = { url: string; excerpt: string; meta: { title?: string } };

const MAX_RESULTS = 8;
const term = ref("");
const items = ref<Result[]>([]);
const loading = ref(false);
const unavailable = ref(false);
let pagefind: Promise<Pagefind> | undefined;

/** Loads the section's Pagefind module once, on first search. */
function load() {
  pagefind ??= import(/* @vite-ignore */ `${props.bundlePath}pagefind.js`).then(async (mod: Pagefind) => {
    await mod.options({ baseUrl: props.baseUrl });
    return mod;
  });
  return pagefind;
}

/** Pagefind excerpts mark matches with <mark>; the menu shows plain text. */
const plain = (html: string) => new DOMParser().parseFromString(html, "text/html").body.textContent ?? "";

watch(term, async (value) => {
  const query = value.trim();
  if (!query) { items.value = []; loading.value = false; return; }
  loading.value = true;
  try {
    const search = await (await load()).debouncedSearch(query, {}, 200);
    if (search === null) return; // superseded by a newer keystroke
    const pages = await Promise.all(search.results.slice(0, MAX_RESULTS).map((result) => result.data()));
    if (query !== term.value.trim()) return;
    items.value = pages.map((page) => ({ label: page.meta.title ?? page.url, description: plain(page.excerpt), url: page.url }));
  } catch {
    unavailable.value = true;
    items.value = [];
  } finally {
    if (query === term.value.trim()) loading.value = false;
  }
});

function open(result: Result | undefined) {
  if (result) navigate(result.url);
}

const menu = useTemplateRef<{ inputRef: HTMLInputElement | null | undefined }>("menu");
defineShortcuts({ meta_k: { usingInput: true, handler: () => menu.value?.inputRef?.focus() } });
</script>

<template>
  <UInputMenu
    ref="menu"
    v-model:search-term="term"
    :items="items"
    :model-value="undefined"
    :placeholder="props.placeholder"
    :aria-label="props.placeholder"
    :loading="loading"
    ignore-filter
    icon="i-hugeicons-search-01"
    trailing-icon=""
    size="lg"
    class="w-full"
    :content="{ align: 'start' }"
    :ui="{
      base: 'bg-surface-raised text-small',
      content: 'min-w-(--reka-combobox-trigger-width) sm:w-lg',
      item: 'items-start py-2',
      itemLabel: 'text-small font-medium text-ink whitespace-normal',
      itemDescription: 'line-clamp-2 text-label text-muted whitespace-normal',
      empty: 'text-small text-muted',
    }"
    @update:model-value="open"
  >
    <template #empty>
      <span v-if="unavailable">Search works on the published site.</span>
      <span v-else-if="term.trim() && !loading">No results for “{{ term.trim() }}”.</span>
      <span v-else>Type to search.</span>
    </template>
  </UInputMenu>
</template>
