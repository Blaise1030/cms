<script setup lang="ts">
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import type { ConnectionStatus } from "@/core/types";

withDefaults(defineProps<{ status?: ConnectionStatus; showChat?: boolean }>(), {
  showChat: true,
});
</script>

<template>
  <div class="flex h-full flex-col overflow-hidden">
    <header class="bg-background flex h-11 shrink-0 items-center gap-3 px-3">
      <div class="flex min-w-0 flex-1 items-center gap-3">
        <span class="shrink-0 text-sm font-medium">workbench</span>
        <div class="bg-border h-4 w-px shrink-0" aria-hidden="true" />
        <slot name="menubar" />
      </div>
      <div class="flex shrink-0 items-center gap-3">
        <slot name="actions" />
        <div class="bg-border h-4 w-px shrink-0" aria-hidden="true" />
        <div class="flex items-center gap-1.5">
          <slot name="account" />
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
      </div>
    </header>
    <ResizablePanelGroup
      direction="horizontal"
      auto-save-id="workbench-shell"
      class="min-h-0 flex-1"
    >
      <ResizablePanel
        id="workspace"
        :default-size="showChat ? 65 : 100"
        :min-size="showChat ? 30 : 100"
      >
        <div class="h-full min-h-0 overflow-hidden">
          <slot name="workspace" />
        </div>
      </ResizablePanel>
      <template v-if="showChat">
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
      </template>
    </ResizablePanelGroup>
  </div>
</template>
