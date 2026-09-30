<script setup lang="ts">
import { ChevronDown, ChevronRight, File, Folder } from "@lucide/vue";
import type { TreeEntry } from "@/core/types";
import { cn } from "@/lib/utils";
import { useFilesStore } from "./store";

defineProps<{ entry: TreeEntry; depth: number }>();
const store = useFilesStore();
</script>

<template>
  <li>
    <button
      type="button"
      :class="
        cn(
          'hover:bg-accent flex w-full items-center gap-1.5 rounded-sm py-1 pr-2 text-left text-xs',
          store.selectedPath === entry.path && entry.kind === 'file' && 'bg-accent',
        )
      "
      :style="{ paddingLeft: `${depth * 12 + 8}px` }"
      :data-testid="`tree-${entry.path}`"
      @click="entry.kind === 'dir' ? store.toggle(entry.path) : store.select(entry.path)"
    >
      <template v-if="entry.kind === 'dir'">
        <ChevronDown v-if="store.expanded[entry.path]" class="size-3.5 shrink-0" />
        <ChevronRight v-else class="size-3.5 shrink-0" />
        <Folder class="text-muted-foreground size-3.5 shrink-0" />
      </template>
      <template v-else>
        <span class="size-3.5 shrink-0" />
        <File class="text-muted-foreground size-3.5 shrink-0" />
      </template>
      <span class="truncate">{{ entry.name }}</span>
    </button>
    <ul v-if="entry.kind === 'dir' && store.expanded[entry.path]">
      <FileTreeNode
        v-for="child in store.dirs[entry.path] ?? []"
        :key="child.path"
        :entry="child"
        :depth="depth + 1"
      />
    </ul>
  </li>
</template>
