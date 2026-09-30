<script setup lang="ts">
import { computed, watch } from "vue";
import { ScrollArea } from "@/components/ui/scroll-area";
import FileTreeNode from "./FileTreeNode.vue";
import FileViewer from "./FileViewer.vue";
import { useFilesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useFilesStore();

watch(
  () => props.projectId,
  (id) => void store.open(id),
  { immediate: true },
);

const root = computed(() => store.dirs[""] ?? []);
</script>

<template>
  <div class="flex h-full min-h-0" data-testid="files-panel">
    <ScrollArea class="w-56 shrink-0 border-r">
      <p v-if="store.treeError" class="text-destructive p-3 text-xs" role="alert">{{ store.treeError }}</p>
      <ul class="py-2" role="tree" aria-label="Project files">
        <FileTreeNode v-for="entry in root" :key="entry.path" :entry="entry" :depth="0" />
      </ul>
    </ScrollArea>
    <div class="min-w-0 flex-1">
      <FileViewer />
    </div>
  </div>
</template>
