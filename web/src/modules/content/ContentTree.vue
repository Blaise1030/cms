<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { FileTree, type ContextMenuItem, type ContextMenuOpenContext, type FileTreeDirectoryHandle, type FileTreeIconConfig } from "@pierre/trees";
import type { ContentCollection, ContentEntrySummary } from "@/core/types";
import { savedExpanded, trackExpanded } from "@/lib/treeExpansion";
import { TREE_SCROLL_FADE_CSS } from "@/lib/treeScrollFade";
import { cn } from "@/lib/utils";
import { treePaths, treeTarget, type TreeAction, type TreeTarget } from "./contentTree";

const props = defineProps<{
  collections: readonly ContentCollection[];
  entries: Readonly<Record<string, readonly ContentEntrySummary[]>>;
  /** Tree path of the open entry (`blog/welcome`). */
  selected: string | null;
  /** The "…" menu for a collection or entry. */
  actions: (target: TreeTarget) => TreeAction[];
  /** Where the open folders are remembered, e.g. per project. */
  storageKey: string;
}>();
const emit = defineEmits<{ open: [collection: string, id: string] }>();

const host = ref<HTMLElement | null>(null);
const measured = ref(0);
let tree: FileTree | null = null;
let untrack: (() => void) | null = null;
let ro: ResizeObserver | null = null;
/** Set while the tree's selection is changed to match `selected`, so it is not reported back. */
let syncing = false;
const paths = computed(() => treePaths(props.collections, props.entries));

const KIND_ICONS: Record<ContentEntrySummary["kind"], string> = {
  markdown: "file-tree-builtin-markdown",
  mdx: "file-tree-builtin-markdown",
  json: "file-tree-builtin-json",
};
/** Entry paths have no extension, so icons come from each entry's kind by file name. */
const icons = computed<FileTreeIconConfig>(() => ({
  set: "complete",
  colored: true,
  byFileName: Object.fromEntries(Object.values(props.entries).flat().map(e => [e.id.split("/").at(-1)!, KIND_ICONS[e.kind]])),
}));

const menu = shallowRef<{ actions: TreeAction[]; top: number; left: number; above: boolean; close: ContextMenuOpenContext["close"] } | null>(null);
const menuEl = ref<HTMLElement | null>(null);

function openMenu(item: ContextMenuItem, context: ContextMenuOpenContext): HTMLElement | null {
  const target = treeTarget(item.path, item.kind);
  const actions = target ? props.actions(target) : [];
  if (!actions.length) { queueMicrotask(() => context.close()); return null; }
  const rect = context.anchorRect;
  const above = rect.bottom > window.innerHeight * 0.6;
  menu.value = { actions, top: above ? rect.top - 4 : rect.bottom + 4, left: rect.right, above, close: context.close };
  void nextTick(() => menuEl.value?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus());
  // The visible menu is teleported to the body; the tree only needs something in its slot.
  return document.createElement("span");
}
function run(action: Exclude<TreeAction, "separator">) {
  menu.value?.close({ restoreFocus: false });
  action.run();
}
function onMenuKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") { event.preventDefault(); menu.value?.close(); return; }
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
  event.preventDefault();
  const buttons = [...(menuEl.value?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
  const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
  buttons[(at + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus();
}

function folder(path: string): FileTreeDirectoryHandle | null {
  const item = tree?.getItem(path);
  return item?.isDirectory() ? (item as FileTreeDirectoryHandle) : null;
}

/** Expand the selected entry's folders and make it the only selected row. */
function showSelected() {
  if (!tree) return;
  syncing = true;
  try {
    for (const p of tree.getSelectedPaths()) if (p !== props.selected) tree.getItem(p)?.deselect();
    if (!props.selected) return;
    const parts = props.selected.split("/");
    for (let i = 1; i < parts.length; i++) {
      const dir = folder(`${parts.slice(0, i).join("/")}/`);
      if (dir && !dir.isExpanded()) dir.expand();
    }
    const item = tree.getItem(props.selected);
    if (item && !item.isSelected()) item.select();
  } finally { syncing = false; }
}

function unmount() {
  untrack?.();
  untrack = null;
  tree?.cleanUp();
  tree = null;
}

function mount() {
  const search = tree?.getSearchValue() || null;
  unmount();
  if (!host.value || measured.value <= 0) return;
  tree = new FileTree({
    paths: paths.value,
    density: "default",
    icons: icons.value,
    initialExpansion: "closed",
    initialExpandedPaths: savedExpanded(props.storageKey) ?? [],
    initialSearchQuery: search,
    search: true,
    fileTreeSearchMode: "hide-non-matches",
    searchBlurBehavior: "retain",
    unsafeCSS: `[data-file-tree-search-input] {
      box-sizing: border-box; min-width: 0; width: 100%;
      background: transparent; color: var(--trees-fg); font-weight: var(--trees-font-weight-regular);
      &:hover { background: var(--trees-bg-muted); }
      &:focus-visible, &[data-file-tree-search-input-fake-focus="true"] { outline: none; background: var(--trees-selected-bg); }
    }${TREE_SCROLL_FADE_CSS}`,
    composition: {
      contextMenu: { enabled: true, triggerMode: "both", buttonVisibility: "when-needed", render: openMenu, onClose: () => { menu.value = null; } },
    },
    onSelectionChange: (selected) => {
      const path = selected[0];
      if (syncing || !path || path === props.selected || path.endsWith("/")) return;
      const target = treeTarget(path, "file");
      if (target?.id) emit("open", target.collection, target.id);
    },
  });
  tree.render({ containerWrapper: host.value });
  untrack = trackExpanded(tree, props.storageKey);
  showSelected();
}

watch(paths, (next) => {
  if (!tree) return;
  tree.resetPaths(next, { initialExpandedPaths: savedExpanded(props.storageKey) ?? [] });
  showSelected();
});
watch(() => props.selected, showSelected);
watch(icons, next => tree?.setIcons(next));
watch(() => props.storageKey, mount);

onMounted(() => {
  if (!host.value?.parentElement) return;
  ro = new ResizeObserver((entries) => {
    const next = Math.floor(entries[0]?.contentRect.height ?? 0);
    if (next === measured.value) return;
    measured.value = next;
    mount();
  });
  ro.observe(host.value.parentElement);
});
onBeforeUnmount(() => {
  ro?.disconnect();
  unmount();
});
</script>

<template>
  <div
    ref="host"
    class="content-tree w-full overflow-hidden"
    :style="{ height: `${Math.max(measured, 1)}px` }"
    data-testid="content-tree"
    aria-label="Collections"
  />
  <Teleport to="body">
    <div
      v-if="menu"
      ref="menuEl"
      role="menu"
      data-file-tree-context-menu-root="true"
      data-testid="content-tree-menu"
      class="fixed z-50 min-w-48 -translate-x-full rounded-lg border bg-popover p-1 text-popover-foreground shadow-md"
      :class="{ '-translate-y-full': menu.above }"
      :style="{ top: `${menu.top}px`, left: `${menu.left}px` }"
      @keydown="onMenuKeydown"
    >
      <template v-for="(action, index) in menu.actions" :key="action === 'separator' ? `sep-${index}` : action.id">
        <div v-if="action === 'separator'" class="-mx-1 my-1 h-px bg-border" role="separator" />
        <template v-else>
          <button
            type="button"
            role="menuitem"
            :disabled="action.disabled"
            :data-testid="`content-tree-${action.id}`"
            :class="cn(
              'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent focus-visible:bg-accent disabled:pointer-events-none disabled:opacity-50 [&_svg]:size-4 [&_svg]:shrink-0',
              action.destructive ? 'text-destructive hover:bg-destructive/10 focus-visible:bg-destructive/10' : '[&_svg]:text-muted-foreground',
            )"
            @click="run(action)"
          >
            <component :is="action.icon" v-if="action.icon" />
            <span v-else class="size-4 shrink-0" />
            {{ action.label }}
          </button>
          <p v-if="action.disabled && action.hint" class="px-2 pb-1 pl-8 text-xs text-muted-foreground">{{ action.hint }}</p>
        </template>
      </template>
    </div>
  </Teleport>
</template>

<style scoped>
.content-tree {
  --trees-bg-override: transparent;
  --trees-fg-override: var(--foreground);
  --trees-fg-muted-override: var(--muted-foreground);
  --trees-selected-bg-override: var(--accent);
  --trees-selected-fg-override: var(--accent-foreground);
  --trees-border-color-override: var(--border);
  --trees-font-family-override: var(--font-sans, ui-sans-serif, system-ui, sans-serif);
  --trees-padding-inline-override: 8px;
  --trees-focus-ring-color-override: var(--ring);
  --trees-selected-focused-border-color-override: var(--ring);
}
</style>
