<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { FILE_TREE_DENSITY_PRESETS, FileTree, type GitStatus } from "@pierre/trees";
import { TREE_SCROLL_FADE_CSS } from "@/lib/treeScrollFade";

const props = withDefaults(
  defineProps<{ files: { path: string; status: GitStatus }[]; fill?: boolean }>(),
  { fill: false },
);
const emit = defineEmits<{ select: [path: string] }>();

const ROW = FILE_TREE_DENSITY_PRESETS.compact.itemHeight;
const MAX_ROWS = 10;

const host = ref<HTMLElement | null>(null);
const measured = ref(0);
let tree: FileTree | null = null;
let ro: ResizeObserver | null = null;

// The tree is virtualized, so it needs a fixed height: fit every row (files plus the folders
// above them) up to a cap, then scroll — or fill the sidebar when `fill` is set.
const height = computed(() => {
  if (props.fill) return Math.max(measured.value, ROW);
  const rows = new Set<string>();
  for (const { path } of props.files) {
    const parts = path.split("/");
    for (let i = 1; i <= parts.length; i++) rows.add(parts.slice(0, i).join("/"));
  }
  return Math.min(rows.size, MAX_ROWS) * ROW + 8;
});

function mount() {
  tree?.cleanUp();
  if (!host.value || (props.fill && measured.value <= 0)) return;
  const paths = props.files.map((f) => f.path);
  tree = new FileTree({
    paths,
    gitStatus: props.files,
    density: "compact",
    initialExpansion: "open",
    flattenEmptyDirectories: true,
    unsafeCSS: TREE_SCROLL_FADE_CSS,
    onSelectionChange: (selected) => {
      const path = selected[0];
      if (path && paths.includes(path)) emit("select", path);
    },
  });
  tree.render({ containerWrapper: host.value });
}

onMounted(() => {
  if (props.fill && host.value?.parentElement) {
    ro = new ResizeObserver((entries) => {
      const next = Math.floor(entries[0]?.contentRect.height ?? 0);
      if (next === measured.value) return;
      measured.value = next;
      mount();
    });
    ro.observe(host.value.parentElement);
  } else {
    mount();
  }
});
watch(() => props.files, mount);
onBeforeUnmount(() => {
  ro?.disconnect();
  ro = null;
  tree?.cleanUp();
  tree = null;
});
</script>

<template>
  <div
    ref="host"
    class="changed-files-tree overflow-hidden"
    :class="fill ? 'w-full' : 'rounded-md border'"
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
