<script setup lang="ts">
import { computed } from "vue";
import { ArrowLeft, Code2, Eye, History } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { Toaster } from "@/components/ui/sonner";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import type { ConnectionStatus } from "@/core/types";

export type WorkspaceTab = "preview" | "files" | "history";

const props = defineProps<{
  status?: ConnectionStatus;
  projectName?: string | null;
  tab?: WorkspaceTab;
}>();

const emit = defineEmits<{
  "update:tab": [tab: WorkspaceTab];
  back: [];
}>();

const hasProject = computed(() => !!props.projectName);

const nav: { id: WorkspaceTab; label: string; icon: typeof Eye }[] = [
  { id: "preview", label: "Preview", icon: Eye },
  { id: "files", label: "Code", icon: Code2 },
  { id: "history", label: "History", icon: History },
];
</script>

<template>
  <div class="dark bg-background text-foreground flex h-svh flex-col overflow-hidden">
    <ResizablePanelGroup
      direction="horizontal"
      auto-save-id="workbench-shell"
      class="min-h-0 flex-1"
    >
      <ResizablePanel id="workspace" :default-size="65" :min-size="30">
        <div class="flex h-full min-h-0 flex-col overflow-hidden">
          <header class="bg-background flex h-11 shrink-0 items-center gap-2 px-3">
            <div class="flex min-w-0 flex-1 items-center gap-1">
              <Button
                v-if="hasProject"
                variant="ghost"
                size="icon-sm"
                aria-label="Back to projects"
                @click="emit('back')"
              >
                <ArrowLeft />
              </Button>
              <span class="truncate px-1 text-sm font-medium">workbench</span>
              <span v-if="projectName" class="text-muted-foreground truncate text-sm">
                / {{ projectName }}
              </span>
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
          <div class="min-h-0 flex-1 overflow-hidden">
            <slot name="workspace" />
          </div>
        </div>
      </ResizablePanel>
      <ResizableHandle with-handle />
      <ResizablePanel
        id="chat"
        :default-size="35"
        :min-size="22"
        collapsible
        :collapsed-size="0"
      >
        <div class="bg-background flex h-full min-h-0 flex-col overflow-hidden border-l">
          <header
            v-if="hasProject"
            class="bg-background flex h-11 w-full shrink-0 items-center justify-end gap-1 px-2"
          >
            <TooltipProvider>
              <div class="flex items-center gap-0.5 rounded-md border p-0.5">
                <Tooltip v-for="item in nav" :key="item.id">
                  <TooltipTrigger as-child>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      :aria-label="item.label"
                      :aria-pressed="tab === item.id"
                      :class="tab === item.id ? 'bg-accent text-accent-foreground' : ''"
                      @click="emit('update:tab', item.id)"
                    >
                      <component :is="item.icon" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{{ item.label }}</TooltipContent>
                </Tooltip>
              </div>
            </TooltipProvider>
            <slot name="toolbar" />
          </header>
          <div class="min-h-0 flex-1 overflow-hidden">
            <slot name="chat" />
          </div>
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
    <Toaster />
  </div>
</template>
