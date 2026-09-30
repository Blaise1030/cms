<script setup lang="ts">
import { ref, watch } from "vue";
import { ScrollArea } from "@/components/ui/scroll-area";
import { highlightCode } from "./highlight";
import { useFilesStore } from "./store";

const store = useFilesStore();
const html = ref("");
let request = 0;

watch(
  () => store.file,
  async (file) => {
    const mine = ++request;
    html.value = "";
    if (!file || file.binary || file.content === null) return;
    const out = await highlightCode(file.content, file.path);
    if (mine === request) html.value = out;
  },
  { immediate: true },
);
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="file-viewer">
    <p
      v-if="!store.selectedPath"
      class="text-muted-foreground flex flex-1 items-center justify-center text-sm"
    >
      Select a file to view it.
    </p>
    <template v-else>
      <div class="text-muted-foreground flex h-8 shrink-0 items-center border-b px-3 font-mono text-xs">
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
      <ScrollArea v-else class="min-h-0 flex-1">
        <!-- eslint-disable-next-line vue/no-v-html -- shiki output is escaped -->
        <div class="viewer text-xs [&_pre]:!bg-transparent [&_pre]:p-3" data-testid="file-content" v-html="html" />
      </ScrollArea>
    </template>
  </div>
</template>
