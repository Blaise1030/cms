<script setup lang="ts">
import { ref } from "vue";
import { PanelLeft, PanelRight } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { Toaster } from "@/components/ui/sonner";
import type { ConnectionStatus } from "@/core/types";

defineProps<{ status?: ConnectionStatus }>();

type PanelApi = { collapse: () => void; expand: () => void; isCollapsed: boolean };
const preview = ref<PanelApi | null>(null);
const chat = ref<PanelApi | null>(null);

function toggle(panel: PanelApi | null) {
  if (!panel) return;
  if (panel.isCollapsed) panel.expand();
  else panel.collapse();
}
</script>

<template>
  <div class="dark bg-background text-foreground flex h-screen flex-col">
    <header class="flex h-10 shrink-0 items-center justify-between border-b px-3">
      <div class="flex items-center gap-2">
        <Button variant="ghost" size="icon-sm" aria-label="Toggle preview" @click="toggle(preview)">
          <PanelLeft />
        </Button>
        <span class="text-sm font-medium">workbench</span>
        <slot name="header" />
      </div>
      <div class="flex items-center gap-2">
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
        <Button variant="ghost" size="icon-sm" aria-label="Toggle chat" @click="toggle(chat)">
          <PanelRight />
        </Button>
      </div>
    </header>

    <ResizablePanelGroup direction="horizontal" auto-save-id="workbench-shell" class="min-h-0 flex-1">
      <ResizablePanel
        id="preview"
        ref="preview"
        :default-size="60"
        :min-size="25"
        collapsible
        :collapsed-size="0"
      >
        <slot name="preview">
          <div class="text-muted-foreground flex h-full items-center justify-center text-sm">
            Preview
          </div>
        </slot>
      </ResizablePanel>
      <ResizableHandle with-handle />
      <ResizablePanel
        id="chat"
        ref="chat"
        :default-size="40"
        :min-size="25"
        collapsible
        :collapsed-size="0"
      >
        <slot name="chat">
          <div class="text-muted-foreground flex h-full items-center justify-center text-sm">
            Chat
          </div>
        </slot>
      </ResizablePanel>
    </ResizablePanelGroup>
    <Toaster />
  </div>
</template>
