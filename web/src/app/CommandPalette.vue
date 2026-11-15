<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useEventListener } from "@vueuse/core";
import { hugeicon } from "@/lib/hugeicon";
import { toast } from "@/lib/toast";
import type { CommandPaletteGroup, CommandPaletteItem, InputProps } from "@nuxt/ui";
import UCommandPalette from "@nuxt/ui/components/CommandPalette.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import { errorMessage } from "@orso/utils";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import { useChangesStore } from "@/modules/changes/store";
import { openAssistantSetup } from "@/modules/assistant/dialog";
import { filterGroups, type PaletteGroup, type PaletteItem } from "./palette/filter";
import { usePaletteGroups } from "./palette/usePaletteGroups";

const GitBranch = hugeicon("i-hugeicons-git-branch");

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
  showAssistant() {
    openAssistantSetup();
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

/** Filtering stays ours (`filterGroups`), so Nuxt UI's fuzzy search is switched off per group. */
const paletteGroups = computed<CommandPaletteGroup[]>(() =>
  visible.value.map((group) => ({
    id: `${page.value}:${group.heading}`,
    label: group.heading,
    ignoreFilter: true,
    items: group.items.map<CommandPaletteItem>((item) => ({
      id: item.id,
      label: item.label,
      suffix: item.hint,
      icon: item.icon,
      checked: item.checked,
      "data-testid": `command-${item.id}`,
      onSelect: () => pick(item),
    })),
  })),
);

// `input` is bound onto UInput, which passes non-prop attrs (the testid, the keydown listener)
// through to the <input> itself.
const inputProps = {
  "data-testid": "command-palette-input",
  onKeydown: onInputKeydown,
} as unknown as InputProps;
</script>

<template>
  <UModal
    v-model:open="open"
    title="Command palette"
    description="Search commands, content and files"
    :ui="{ content: 'top-[15vh] translate-y-0 overflow-hidden sm:max-w-2xl' }"
  >
    <template #content>
      <UCommandPalette
        v-model:search-term="query"
        data-testid="command-palette"
        :groups="paletteGroups"
        :fuse="{ resultLimit: Infinity }"
        preserve-group-order
        :input="inputProps"
        :placeholder="page === 'changes' ? 'Switch to change…' : 'Type a command or search…'"
        :ui="{ content: 'max-h-[min(480px,60vh)]', itemLabelSuffix: 'text-xs' }"
      >
        <template #empty>
          <p class="text-muted py-6 text-center text-sm">
            {{ page === "changes" && changes.listing ? "Loading…" : "No results found." }}
          </p>
        </template>
        <template #item-trailing="{ item }">
          <UIcon v-if="item.checked" name="i-hugeicons-tick-02" class="size-4" />
        </template>
      </UCommandPalette>
    </template>
  </UModal>

  <NewChangeDialog v-if="projectId" v-model:open="showNewChange" :project-id="projectId" />
</template>
