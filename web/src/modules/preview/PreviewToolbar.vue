<script setup lang="ts">
import { computed, watch } from "vue";
import { Monitor, RefreshCw, Smartphone, Tablet } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import type { DevserverState } from "@/core/types";
import { usePreviewStore, type Device } from "./store";

const props = defineProps<{ projectId: string; devserver?: DevserverState }>();

const store = usePreviewStore();

watch(
  () => [props.projectId, props.devserver] as const,
  ([id, state]) => {
    if (state) store.seed(id, state);
  },
  { immediate: true },
);

const state = computed<DevserverState>(
  () => store.status[props.projectId] ?? props.devserver ?? "starting",
);

const DOT: Record<DevserverState, string> = {
  starting: "bg-amber-500 animate-pulse",
  ready: "bg-emerald-500",
  crashed: "bg-red-500",
};
const LABEL: Record<DevserverState, string> = {
  starting: "Dev server starting",
  ready: "Dev server ready",
  crashed: "Dev server crashed",
};

const devices: { id: Device; label: string; icon: typeof Monitor }[] = [
  { id: "desktop", label: "Desktop", icon: Monitor },
  { id: "tablet", label: "Tablet", icon: Tablet },
  { id: "mobile", label: "Mobile", icon: Smartphone },
];
</script>

<template>
  <div class="flex items-center gap-0.5" data-testid="preview-toolbar">
    <TooltipProvider>
      <Tooltip v-for="d in devices" :key="d.id">
        <TooltipTrigger as-child>
          <Button
            variant="ghost"
            size="icon-sm"
            :aria-label="d.label"
            :aria-pressed="store.device === d.id"
            :class="store.device === d.id ? 'bg-accent text-accent-foreground' : ''"
            @click="store.setDevice(d.id)"
          >
            <component :is="d.icon" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{{ d.label }}</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger as-child>
          <span
            class="mx-1 size-2 rounded-full"
            :class="DOT[state]"
            :data-state="state"
            role="status"
            :aria-label="LABEL[state]"
            data-testid="status-dot"
          />
        </TooltipTrigger>
        <TooltipContent>{{ LABEL[state] }}</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger as-child>
          <Button variant="ghost" size="icon-sm" aria-label="Reload preview" @click="store.reload()">
            <RefreshCw />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Reload</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  </div>
</template>
