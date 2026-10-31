<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { FileJson, FileText, MoreHorizontal, Plus, Search } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
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
const rows = computed(() => visible.value.map(entry => ({ entry, menu: props.actions(entry.id) })));
const KIND_ICON = { markdown: FileText, mdx: FileText, json: FileJson } as const;

// A filter belongs to the collection it was typed in; another collection starts fresh.
watch(() => props.collection.name, () => { query.value = ""; });
</script>

<template>
  <section data-testid="content-entry-list" class="flex h-full min-h-0 flex-col">
    <header class="flex shrink-0 flex-col gap-2 px-4 pt-4 pb-3">
      <div class="flex items-baseline gap-2">
        <h2 class="truncate text-sm font-medium">{{ humanize(collection.name) }}</h2>
        <span class="text-xs text-muted-foreground">{{ visible.length }} {{ visible.length === 1 ? "entry" : "entries" }}</span>
      </div>
      <div class="flex items-center gap-2">
        <InputGroup class="h-8 max-w-64">
          <InputGroupAddon align="inline-start">
            <Search class="size-4 opacity-50" aria-hidden="true" />
          </InputGroupAddon>
          <InputGroupInput v-model="query" data-testid="content-filter" placeholder="Filter entries" aria-label="Filter entries" class="h-8" />
        </InputGroup>
        <Button
          type="button"
          size="sm"
          class="ml-auto shrink-0"
          data-testid="content-new-entry"
          :disabled="!!createHint"
          :title="createHint ?? undefined"
          @click="emit('create')"
        >
          <Plus class="size-4" />
          New entry
        </Button>
      </div>
    </header>
    <p v-if="loading" class="px-4 text-sm text-muted-foreground">Loading content…</p>
    <div v-else-if="!visible.length" class="px-4 py-10 text-center text-sm text-muted-foreground" data-testid="content-entry-empty">
      {{ query.trim() ? "No matches" : "No entries yet" }}
    </div>
    <ul v-else class="min-h-0 flex-1 overflow-auto px-2 pb-4">
      <li v-for="r in rows" :key="r.entry.id" class="group flex items-center gap-1 rounded-md pr-1 hover:bg-accent/40">
        <button
          type="button"
          class="flex min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1.5 text-left outline-none focus-visible:bg-accent"
          data-testid="content-entry"
          :data-entry-id="r.entry.id"
          @click="emit('open', r.entry.id)"
        >
          <component :is="KIND_ICON[r.entry.kind]" class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
          <span class="truncate text-sm">{{ humanize(r.entry.id) }}</span>
          <span class="ml-auto shrink-0 text-xs text-muted-foreground">{{ r.entry.kind }}</span>
        </button>
        <DropdownMenu v-if="r.menu.length">
          <DropdownMenuTrigger as-child>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="shrink-0 text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100"
              data-testid="content-entry-menu"
              :aria-label="`Actions for ${r.entry.id}`"
            >
              <MoreHorizontal class="size-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-48">
            <template v-for="(action, index) in r.menu" :key="action === 'separator' ? `sep-${index}` : action.id">
              <DropdownMenuSeparator v-if="action === 'separator'" />
              <template v-else>
                <DropdownMenuItem
                  :disabled="action.disabled"
                  :data-testid="`content-entry-${action.id}`"
                  :class="action.destructive ? 'text-destructive focus:text-destructive' : undefined"
                  @select="action.run()"
                >
                  <component :is="action.icon" v-if="action.icon" class="size-4" />
                  {{ action.label }}
                </DropdownMenuItem>
                <p v-if="action.disabled && action.hint" class="px-2 pb-1 pl-8 text-xs text-muted-foreground">{{ action.hint }}</p>
              </template>
            </template>
          </DropdownMenuContent>
        </DropdownMenu>
      </li>
    </ul>
  </section>
</template>
