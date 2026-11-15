<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { DropdownMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import type { ContentCollection, ContentEntrySummary } from "@/core/types";
import { filterEntries, sortEntries, type EntryAction } from "./entryList";
import { humanize } from "./labels";

const props = defineProps<{
  collection: ContentCollection;
  entries: readonly ContentEntrySummary[];
  loading: boolean;
  /** Why create is unavailable, or null when it works. */
  createHint: string | null;
  actions: (id: string) => EntryAction[];
}>();
const emit = defineEmits<{ open: [id: string]; create: [] }>();

/** The header count follows the rows on screen, so a filter reads as "1 entry". */
const query = ref("");
const visible = computed(() => sortEntries(filterEntries(props.entries, query.value)));
const rows = computed(() => visible.value.map(entry => ({ entry, menu: menuItems(props.actions(entry.id)) })));
const KIND_ICON = { markdown: "i-hugeicons-file-01", mdx: "i-hugeicons-file-01", json: "i-hugeicons-file-script" } as const;

/**
 * Menu rows for UDropdownMenu. Action icons are components (owned by `entryList.ts`), so they are
 * drawn in the `item-leading` slot; a disabled action's hint shows as the item description.
 */
function menuItems(actions: EntryAction[]): DropdownMenuItem[] {
  return actions.map((action): DropdownMenuItem => action === "separator"
    ? { type: "separator" }
    : {
        label: action.label,
        actionIcon: action.icon,
        color: action.destructive ? "error" : undefined,
        disabled: action.disabled,
        description: action.disabled && action.hint ? action.hint : undefined,
        "data-testid": `content-entry-${action.id}`,
        onSelect: () => action.run(),
      });
}

// A filter belongs to the collection it was typed in; another collection starts fresh.
watch(() => props.collection.name, () => { query.value = ""; });
</script>

<template>
  <section data-testid="content-entry-list" class="flex h-full min-h-0 flex-col">
    <header class="flex shrink-0 flex-col gap-2 px-4 pt-4 pb-3">
      <div class="flex items-baseline gap-2">
        <h2 class="text-highlighted truncate text-sm font-medium">{{ humanize(collection.name) }}</h2>
        <span class="text-muted text-xs">{{ visible.length }} {{ visible.length === 1 ? "entry" : "entries" }}</span>
      </div>
      <div class="flex items-center gap-2">
        <UInput
          v-model="query"
          icon="i-hugeicons-search-01"
          size="sm"
          class="w-full max-w-64"
          data-testid="content-filter"
          placeholder="Filter entries"
          aria-label="Filter entries"
        />
        <UButton
          type="button"
          size="sm"
          class="ml-auto shrink-0"
          icon="i-hugeicons-plus-sign"
          label="New entry"
          data-testid="content-new-entry"
          :disabled="!!createHint"
          :title="createHint ?? undefined"
          @click="emit('create')"
        />
      </div>
    </header>
    <p v-if="loading" class="text-muted px-4 text-sm">Loading content…</p>
    <div v-else-if="!visible.length" class="text-muted px-4 py-10 text-center text-sm" data-testid="content-entry-empty">
      {{ query.trim() ? "No matches" : "No entries yet" }}
    </div>
    <ul v-else class="min-h-0 flex-1 overflow-auto px-2 pb-4">
      <li v-for="r in rows" :key="r.entry.id" class="flex items-center gap-1 pr-1">
        <UButton
          type="button"
          color="neutral"
          variant="ghost"
          :icon="KIND_ICON[r.entry.kind]"
          :label="humanize(r.entry.id)"
          class="text-default min-w-0 flex-1 justify-start"
          :ui="{ leadingIcon: 'text-muted', label: 'truncate' }"
          data-testid="content-entry"
          :data-entry-id="r.entry.id"
          @click="emit('open', r.entry.id)"
        />
        <UDropdownMenu v-if="r.menu.length" :items="r.menu" :content="{ align: 'end' }" :ui="{ content: 'w-48' }">
          <UButton
            type="button"
            color="neutral"
            variant="ghost"
            size="sm"
            square
            icon="i-hugeicons-more-horizontal"
            class="text-muted shrink-0"
            data-testid="content-entry-menu"
            :aria-label="`Actions for ${r.entry.id}`"
          />
          <template #item-leading="{ item }">
            <component :is="item.actionIcon" v-if="item.actionIcon" class="size-4 shrink-0" />
          </template>
        </UDropdownMenu>
      </li>
    </ul>
  </section>
</template>
