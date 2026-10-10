<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { FileTree } from "@pierre/trees";

const props = defineProps<{
  paths: string[];
  selectedPath?: string | null;
}>();
const emit = defineEmits<{ select: [path: string] }>();

const host = ref<HTMLElement | null>(null);
const measured = ref(0);
let tree: FileTree | null = null;
let ro: ResizeObserver | null = null;
let pathSet = new Set<string>();

function mount() {
  tree?.cleanUp();
  tree = null;
  if (!host.value || measured.value <= 0) return;
  pathSet = new Set(props.paths);
  tree = new FileTree({
    paths: props.paths,
    density: "compact",
    initialExpansion: 1,
    flattenEmptyDirectories: true,
    initialSelectedPaths: props.selectedPath ? [props.selectedPath] : [],
    onSelectionChange: (selected) => {
      const path = selected[0];
      if (path && pathSet.has(path)) emit("select", path);
    },
  });
  tree.render({ containerWrapper: host.value });
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

watch(
  () => [props.paths, props.selectedPath] as const,
  () => mount(),
  { deep: true },
);

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
  --trees-font-family-override: var(--font-mono, ui-monospace, monospace);
  --trees-padding-inline-override: 8px;
}
</style>
