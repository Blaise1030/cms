<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import { FileCode } from "@lucide/vue";
import { useFilesStore } from "./store";

// CodeMirror and its language packs are large; keep them out of the main bundle.
const FileCodeMirror = defineAsyncComponent(() => import("./FileCodeMirror.vue"));

const store = useFilesStore();

const readyFile = computed(() => {
  const file = store.file;
  if (!file || file.binary || file.content === null) return null;
  return file;
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="file-viewer">
    <div
      v-if="!store.selectedPath"
      class="text-muted-foreground flex flex-1 flex-col items-center justify-center gap-3 text-sm"
    >
      <FileCode class="size-8 opacity-50" aria-hidden="true" />
      <p>Select a file to view it.</p>
    </div>
    <template v-else>
      <div class="text-muted-foreground flex h-8 shrink-0 items-center px-3 font-mono text-xs">
        {{ store.selectedPath }}
      </div>
      <p v-if="store.fileError" class="text-destructive p-3 text-sm" role="alert">{{ store.fileError }}</p>
      <p v-else-if="store.fileLoading && !store.file" class="text-muted-foreground p-3 text-sm">Loading…</p>
      <p
        v-else-if="store.file && (store.file.binary || store.file.content === null)"
        class="text-muted-foreground p-3 text-sm"
        data-testid="file-unavailable"
      >
        {{ store.file.binary ? "Binary file" : "File too large" }} ({{ store.file.size }} bytes) — no preview.
      </p>
      <div v-else-if="readyFile" class="min-h-0 flex-1 overflow-hidden">
        <FileCodeMirror :code="readyFile.content!" :path="readyFile.path" />
      </div>
    </template>
  </div>
</template>
