<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { FILE_TREE_DENSITY_PRESETS, FileTree, type GitStatus } from "@pierre/trees";

const props = defineProps<{ files: { path: string; status: GitStatus }[] }>();
const emit = defineEmits<{ select: [path: string] }>();

const ROW = FILE_TREE_DENSITY_PRESETS.compact.itemHeight;
const MAX_ROWS = 10;

const host = ref<HTMLElement | null>(null);
let tree: FileTree | null = null;

// The tree is virtualized, so it needs a fixed height: fit every row (files plus the folders
// above them) up to a cap, then scroll.
const height = computed(() => {
  const rows = new Set<string>();
  for (const { path } of props.files) {
    const parts = path.split("/");
    for (let i = 1; i <= parts.length; i++) rows.add(parts.slice(0, i).join("/"));
  }
  return Math.min(rows.size, MAX_ROWS) * ROW + 8;
});

function mount() {
  tree?.cleanUp();
  if (!host.value) return;
  const paths = props.files.map((f) => f.path);
  tree = new FileTree({
    paths,
    gitStatus: props.files,
    density: "compact",
    initialExpansion: "open",
    flattenEmptyDirectories: true,
    onSelectionChange: (selected) => {
      const path = selected[0];
      if (path && paths.includes(path)) emit("select", path);
    },
  });
  tree.render({ containerWrapper: host.value });
}

onMounted(mount);
watch(() => props.files, mount);
onBeforeUnmount(() => {
  tree?.cleanUp();
  tree = null;
});
</script>

<template>
  <div
    ref="host"
    class="changed-files-tree overflow-hidden rounded-md border"
    :style="{ height: `${height}px` }"
    data-testid="changed-files-tree"
  />
</template>

<style scoped>
.changed-files-tree {
  --trees-bg-override: transparent;
  --trees-fg-override: var(--foreground);
  --trees-fg-muted-override: var(--muted-foreground);
  --trees-selected-bg-override: var(--accent);
  --trees-border-color-override: var(--border);
  --trees-font-family-override: var(--font-mono, ui-monospace, monospace);
}
</style>
