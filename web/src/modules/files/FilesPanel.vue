<script setup lang="ts">
import { watch } from "vue";
import { useRouter } from "vue-router";
import { codePath } from "@/router";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import ProjectFilesTree from "./ProjectFilesTree.vue";
import FileViewer from "./FileViewer.vue";
import { useFilesStore } from "./store";

const props = defineProps<{ projectId: string; path?: string }>();
const store = useFilesStore();
const router = useRouter();

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
  <ResizablePanelGroup direction="horizontal" auto-save-id="files-sidebar" class="h-full min-h-0" data-testid="files-panel">
    <ResizablePanel id="files-tree" :default-size="20" :min-size="12" :max-size="45">
      <aside class="flex h-full flex-col overflow-hidden">
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
          :storage-key="`files:${projectId}`"
          @select="router.push(codePath(projectId, $event))"
        />
      </aside>
    </ResizablePanel>
    <ResizableHandle with-handle class="bg-transparent" />
    <ResizablePanel id="files-viewer" :default-size="80" :min-size="40">
      <div class="h-full min-w-0">
        <FileViewer />
      </div>
    </ResizablePanel>
  </ResizablePanelGroup>
</template>
