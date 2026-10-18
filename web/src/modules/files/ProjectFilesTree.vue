<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { FileTree, type FileTreeDirectoryHandle } from "@pierre/trees";
import { savedExpanded, trackExpanded } from "@/lib/treeExpansion";
import { TREE_SCROLL_FADE_CSS } from "@/lib/treeScrollFade";

const props = defineProps<{
  paths: string[];
  selectedPath?: string | null;
  /** Where the open folders are remembered, e.g. per project. */
  storageKey: string;
}>();
const emit = defineEmits<{ select: [path: string] }>();

const host = ref<HTMLElement | null>(null);
const measured = ref(0);
let tree: FileTree | null = null;
let untrack: (() => void) | null = null;
let ro: ResizeObserver | null = null;
let pathSet = new Set<string>();
/** Set while the selection is changed to match `selectedPath`, so it is not reported back. */
let syncing = false;

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
  pathSet = new Set(props.paths);
  const expanded = savedExpanded(props.storageKey);
  tree = new FileTree({
    paths: props.paths,
    density: "compact",
    icons: "minimal",
    search: true,
    searchBlurBehavior: "retain",
    fileTreeSearchMode: "hide-non-matches",
    initialSearchQuery: search,
    unsafeCSS: `[data-file-tree-search-input] { box-sizing: border-box; min-width: 0; width: 100%; height: 30px; line-height: 30px; background: var(--card); }${TREE_SCROLL_FADE_CSS}`,
    ...(expanded ? { initialExpansion: "closed" as const, initialExpandedPaths: expanded } : { initialExpansion: 1 }),
    flattenEmptyDirectories: true,
    initialSelectedPaths: props.selectedPath ? [props.selectedPath] : [],
    onSelectionChange: (selected) => {
      const path = selected[0];
      if (!syncing && path && path !== props.selectedPath && pathSet.has(path)) emit("select", path);
    },
  });
  tree.render({ containerWrapper: host.value });
  untrack = trackExpanded(tree, props.storageKey);
}

function showSelected() {
  if (!tree) return;
  syncing = true;
  try {
    for (const p of tree.getSelectedPaths()) if (p !== props.selectedPath) tree.getItem(p)?.deselect();
    const parts = props.selectedPath?.split("/") ?? [];
    for (let i = 1; i < parts.length; i++) {
      const dir = tree.getItem(`${parts.slice(0, i).join("/")}/`);
      if (dir?.isDirectory() && !(dir as FileTreeDirectoryHandle).isExpanded()) (dir as FileTreeDirectoryHandle).expand();
    }
    const item = props.selectedPath ? tree.getItem(props.selectedPath) : null;
    if (item && !item.isSelected()) item.select();
  } finally {
    syncing = false;
  }
}

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

watch(() => props.paths, (paths) => {
  if (!tree) return;
  pathSet = new Set(paths);
  tree.resetPaths(paths, { initialExpandedPaths: savedExpanded(props.storageKey) ?? [] });
  showSelected();
}, { deep: true });
watch(() => props.selectedPath, showSelected);
watch(() => props.storageKey, mount);

onBeforeUnmount(() => {
  ro?.disconnect();
  ro = null;
  unmount();
});
</script>

<template>
  <div
    ref="host"
    class="project-files-tree h-full w-full overflow-hidden"
    :style="{ height: `${Math.max(measured, 1)}px` }"
    data-testid="project-files-tree"
    role="presentation"
    aria-label="Project files"
  />
</template>

<style scoped>
.project-files-tree {
  --trees-bg-override: transparent;
  --trees-fg-override: var(--foreground);
  --trees-fg-muted-override: var(--muted-foreground);
  --trees-selected-bg-override: var(--accent);
  --trees-border-color-override: var(--border);
  --trees-font-family-override: var(--font-sans, ui-sans-serif, system-ui, sans-serif);
  --trees-padding-inline-override: 8px;
}
</style>
