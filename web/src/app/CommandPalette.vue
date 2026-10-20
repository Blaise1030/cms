<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useEventListener } from "@vueuse/core";
import { ListboxFilter } from "reka-ui";
import { Check, GitBranch, Search } from "@lucide/vue";
import { toast } from "vue-sonner";
import { CommandDialog, CommandGroup, CommandItem, CommandList } from "@/components/ui/command";
import { errorMessage } from "@orso/utils";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import { useChangesStore } from "@/modules/changes/store";
import { filterGroups, type PaletteGroup, type PaletteItem } from "./palette/filter";
import { usePaletteGroups } from "./palette/usePaletteGroups";

const open = ref(false);
const query = ref("");
const page = ref<"root" | "changes">("root");
const showNewChange = ref(false);
const changes = useChangesStore();

const { projectId, groups, load } = usePaletteGroups({
  showChanges() {
    page.value = "changes";
    query.value = "";
    if (projectId.value) changes.list(projectId.value).catch((e) => toast.error(errorMessage(e)));
  },
  showNewChange() {
    showNewChange.value = true;
  },
});

const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

// Capture phase: the code editor and terminal would otherwise swallow the shortcut.
useEventListener(window, "keydown", (e: KeyboardEvent) => {
  if (e.key.toLowerCase() !== "k" || e.shiftKey || e.altKey || !(isMac ? e.metaKey : e.ctrlKey)) return;
  e.preventDefault();
  e.stopPropagation();
  open.value = !open.value;
}, { capture: true });

watch(open, (isOpen) => {
  if (isOpen) load();
  else {
    query.value = "";
    page.value = "root";
  }
});

const changeGroups = computed<PaletteGroup[]>(() => {
  const id = projectId.value;
  if (!id) return [];
  return [{
    heading: changes.listing ? "Changes (loading…)" : "Changes",
    items: changes.items.map((c) => ({
      id: `change:${c.branch}`,
      label: c.title,
      hint: c.branch,
      icon: GitBranch,
      checked: c.branch === changes.active?.branch,
      run: async () => {
        if (c.branch === changes.active?.branch) return;
        try {
          await changes.switchTo(id, c.branch);
        } catch (e) {
          toast.error(errorMessage(e));
        }
      },
    })),
  }];
});

const visible = computed(() => filterGroups(page.value === "changes" ? changeGroups.value : groups.value, query.value));

function pick(item: PaletteItem) {
  if (!item.stay) open.value = false;
  void item.run();
}

function onInputKeydown(e: KeyboardEvent) {
  if (e.key === "Backspace" && !query.value && page.value !== "root") page.value = "root";
}
</script>

<template>
  <CommandDialog v-model:open="open" title="Command palette" description="Search commands, content and files">
    <div class="flex h-9 items-center gap-2 border-b px-3">
      <Search class="size-4 shrink-0 opacity-50" />
      <ListboxFilter
        v-model="query"
        auto-focus
        data-testid="command-palette-input"
        :placeholder="page === 'changes' ? 'Switch to change…' : 'Type a command or search…'"
        class="placeholder:text-muted-foreground flex h-10 w-full bg-transparent py-3 text-sm outline-hidden"
        @keydown="onInputKeydown"
      />
    </div>
    <CommandList data-testid="command-palette" class="max-h-[min(480px,60vh)]">
      <p v-if="!visible.length" class="text-muted-foreground py-6 text-center text-sm">
        {{ page === "changes" && changes.listing ? "Loading…" : "No results found." }}
      </p>
      <CommandGroup v-for="group in visible" :key="`${page}:${group.heading}`" :heading="group.heading">
        <CommandItem
          v-for="item in group.items"
          :key="item.id"
          :value="item.id"
          :data-testid="`command-${item.id}`"
          @select="pick(item)"
        >
          <component :is="item.icon" v-if="item.icon" />
          <span class="truncate">{{ item.label }}</span>
          <span v-if="item.hint" class="text-muted-foreground truncate text-xs">{{ item.hint }}</span>
          <Check v-if="item.checked" class="ml-auto" />
        </CommandItem>
      </CommandGroup>
    </CommandList>
  </CommandDialog>

  <NewChangeDialog
    v-if="projectId"
    v-model:open="showNewChange"
    :project-id="projectId"
    via-agent
    description="The agent starts it from the latest version of the site."
  />
</template>
