<script setup lang="ts">
import { computed, watch } from "vue";
import { useRouter } from "vue-router";
import { codePath } from "@/router";
import type { SplitterItem } from "@nuxt/ui";
import UIcon from "@nuxt/ui/components/Icon.vue";
import USplitter from "@nuxt/ui/components/Splitter.vue";
import ProjectFilesTree from "./ProjectFilesTree.vue";
import FileViewer from "./FileViewer.vue";
import { useFilesStore } from "./store";

const props = defineProps<{ projectId: string; path?: string }>();
const store = useFilesStore();
const router = useRouter();

// Panel ids and the auto-save id match the old resizable layout, so saved widths carry over.
const panels: SplitterItem[] = [
  { id: "files-tree", slot: "tree", defaultSize: 20, minSize: 12, maxSize: 45 },
  { id: "files-viewer", slot: "viewer", defaultSize: 80, minSize: 40 },
];

// With a site folder set, the tree is rooted there: it shows paths relative to the folder,
// while the store, the URL and file reads keep repo-relative paths.
const prefix = computed(() => (store.root ? `${store.root}/` : ""));
const treePaths = computed(() =>
  prefix.value ? store.paths.map((p) => p.slice(prefix.value.length)) : store.paths,
);
const treeSelected = computed(() => {
  const path = store.selectedPath;
  if (!path || !prefix.value) return path;
  return path.startsWith(prefix.value) ? path.slice(prefix.value.length) : null;
});
const treeStorageKey = computed(() =>
  store.root ? `files:${props.projectId}:${store.root}` : `files:${props.projectId}`,
);

watch(
  () => props.projectId,
  (id) => void store.open(id),
  { immediate: true },
);
// The URL names the open file; without one, the file left open earlier is put back in the URL.
watch(
  () => props.path,
  (path) => {
    if (path) {
      if (path !== store.selectedPath) void store.select(path);
    } else if (store.selectedPath) {
      void router.replace(codePath(props.projectId, store.selectedPath));
    }
  },
  { immediate: true },
);
</script>

<template>
  <USplitter
    :items="panels"
    auto-save-id="files-sidebar"
    class="h-full min-h-0"
    :ui="{ handle: 'flex items-center justify-center' }"
    data-testid="files-panel"
  >
    <template #tree>
      <aside class="flex h-full w-full flex-col overflow-hidden">
        <p v-if="store.treeError" class="text-error p-3 text-xs" role="alert">
          {{ store.treeError }}
        </p>
        <p
          v-else-if="store.treeLoading && store.paths.length === 0"
          class="text-muted flex items-center gap-2 p-3 text-xs"
        >
          <UIcon name="i-hugeicons-loading-03" class="size-3.5 animate-spin" /> Loading files…
        </p>
        <ProjectFilesTree
          v-else
          :paths="treePaths"
          :selected-path="treeSelected"
          :storage-key="treeStorageKey"
          @select="router.push(codePath(projectId, prefix + $event))"
        />
      </aside>
    </template>
    <template #resize-handle>
      <div
        class="bg-accented group-hover:bg-inverted/70 group-data-[resize-handle-state=drag]:bg-inverted h-8 w-1 rounded-full transition-colors"
        aria-hidden="true"
      />
    </template>
    <template #viewer>
      <div class="h-full w-full min-w-0">
        <FileViewer />
      </div>
    </template>
  </USplitter>
</template>
