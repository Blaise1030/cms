<script setup lang="ts">
import { computed } from "vue";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { Toaster } from "@/components/ui/sonner";
import type { ConnectionStatus } from "@/core/types";

export type WorkspaceTab = "preview" | "files" | "history";

const props = defineProps<{
  status?: ConnectionStatus;
  projectName?: string | null;
}>();

const hasProject = computed(() => !!props.projectName);
</script>

<template>
  <div class="dark bg-background text-foreground flex h-svh flex-col overflow-hidden">
    <header class="bg-background flex h-11 shrink-0 items-center gap-2 border-b px-3">
      <div class="flex min-w-0 flex-1 items-center gap-1">
        <span class="truncate px-1 text-sm font-medium">workbench</span>
        <slot v-if="hasProject" name="menubar" />
      </div>
      <div class="flex items-center gap-2 pr-1">
        <span
          v-if="status"
          class="text-muted-foreground flex items-center gap-1.5 text-xs"
          :data-status="status"
        >
          <span
            class="size-1.5 rounded-full"
            :class="status === 'open' ? 'bg-emerald-500' : 'bg-amber-500'"
          />
          {{ status === "open" ? "connected" : status }}
        </span>
      </div>
    </header>
    <ResizablePanelGroup
      direction="horizontal"
      auto-save-id="workbench-shell"
      class="min-h-0 flex-1"
    >
      <ResizablePanel id="workspace" :default-size="65" :min-size="30">
        <div class="h-full min-h-0 overflow-hidden">
          <slot name="workspace" />
        </div>
      </ResizablePanel>
      <ResizableHandle class="hover:bg-border data-[resize-handle-state=drag]:bg-border bg-transparent transition-colors" />
      <ResizablePanel
        id="chat"
        :default-size="35"
        :min-size="22"
        collapsible
        :collapsed-size="0"
      >
        <div class="bg-background h-full min-h-0 overflow-hidden">
          <slot name="chat" />
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
    <Toaster />
  </div>
</template>
