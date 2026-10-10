<script setup lang="ts">
import { watch } from "vue";
import ProjectFilesTree from "./ProjectFilesTree.vue";
import FileViewer from "./FileViewer.vue";
import { useFilesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useFilesStore();

watch(
  () => props.projectId,
  (id) => void store.open(id),
  { immediate: true },
);
</script>

<template>
  <div class="flex h-full min-h-0" data-testid="files-panel">
    <aside class="flex w-60 shrink-0 flex-col overflow-hidden">
      <p v-if="store.treeError" class="text-destructive p-3 text-xs" role="alert">
        {{ store.treeError }}
      </p>
      <p v-else-if="store.treeLoading && store.paths.length === 0" class="text-muted-foreground p-3 text-xs">
        Loading files…
      </p>
      <ProjectFilesTree
        v-else
        :paths="store.paths"
        :selected-path="store.selectedPath"
        @select="store.select"
      />
    </aside>
    <div class="min-w-0 flex-1">
      <FileViewer />
    </div>
  </div>
</template>
